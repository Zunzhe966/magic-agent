mod apps;
// config / mihomo 对 bin 工具（dump_conf）公开
pub mod config;
mod keychain;
pub mod mihomo;
mod ssh;
mod system_proxy;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Manager;

use crate::config::AppConfig;
use crate::mihomo::{MihomoManager, MihomoStatus};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub proxy_running: bool,
    pub proxy_pid: Option<u32>,
    pub proxy_port: u16,
    pub system_proxy: bool,
    pub apps_count: usize,
    pub nodes_count: usize,
    pub ssh: Option<crate::ssh::SshSession>,
}

/// 代理端口与现有 Clash/FlClash 冲突检测结果
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub has_conflict: bool,
    pub messages: Vec<String>,
}

/// 第三方代理应用的特征（进程名关键字 → 显示名）。
/// 用于启动魔法代理时识别需要清理的"其他代理"。
/// 注意：只用足够具体的名字，绝不用宽泛的 "proxy"，避免误杀无关进程。
const FOREIGN_PROXY_APPS: &[(&str, &str)] = &[
    ("flclash", "FlClash"),
    ("clash verge", "Clash Verge"),
    ("clash-verge", "Clash Verge"),
    ("clashverge", "Clash Verge"),
    ("clash for windows", "Clash for Windows"),
    ("clashx", "ClashX"),
    ("clash-nyanpasu", "Clash Nyanpasu"),
    ("clash.meta", "Clash.Meta"),
    ("v2rayx", "V2RayX"),
    ("v2rayu", "V2RayU"),
    ("qv2ray", "Qv2ray"),
    ("shadowsocksx", "ShadowsocksX"),
    ("shadowsocks-ng", "Shadowsocks-NG"),
    ("surge", "Surge"),
    ("quantumult", "Quantumult"),
    ("stash", "Stash"),
    ("loon", "Loon"),
    ("sing-box", "sing-box"),
    ("singbox", "sing-box"),
    ("trojan", "Trojan"),
    ("naiveproxy", "NaiveProxy"),
    ("hysteria", "Hysteria"),
    ("xray", "Xray"),
    ("v2ray", "V2Ray"),
];

/// 第三方代理的子进程名（内核进程，通常父进程被杀了它们还活着）。
/// 这些是已知代理软件的"内核"可执行名，需要一并清理，否则代理仍在生效。
/// 绝不放入宽泛的 "mihomo"——那会误杀本程序自己的内核。
const FOREIGN_PROXY_CORES: &[&str] = &[
    "flclashcore",
    "clash-verge-service",
    "clash-verge-service-ipc",
    "verge-mihomo",
    "clash-meta",
    "clash-meta-core",
    "sing-box",
    "v2ray-core",
    "xray-core",
    "hysteria",
    "naive",
    "trojan-go",
];

/// 扫描并返回正在运行的第三方代理进程 (pid, 描述) 列表。
/// 跳过本程序自己的进程和 runtime 目录下的内核。
fn find_foreign_proxies() -> Vec<(u32, String)> {
    let mut found: Vec<(u32, String)> = Vec::new();
    let runtime = MihomoManager::new().runtime_dir;
    let runtime_str = runtime.to_string_lossy().to_string();
    let self_pid = std::process::id();

    let ps = match std::process::Command::new("/bin/ps").args(["-axo", "pid=,args="]).output() {
        Ok(o) => o,
        Err(_) => return found,
    };
    let text = String::from_utf8_lossy(&ps.stdout);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(2, char::is_whitespace);
        let pid: u32 = match parts.next().and_then(|s| s.trim().parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let args = match parts.next() { Some(a) => a.trim(), None => continue };
        if pid == self_pid { continue; }
        // 跳过本程序自己的可执行文件
        if args.contains("magic-agent") || args.contains("magic_probe") || args.contains("dump_conf") {
            continue;
        }
        // 跳过本程序 runtime 目录下的内核（那是我们自己的）
        if args.contains(&runtime_str) { continue; }

        // 只取可执行文件路径部分做匹配（首个空格前的 token）。
        // 绝不用整个命令行匹配：否则任何在参数里提到 "clash"/"v2ray" 字样的进程
        // （grep、编辑器、脚本）都会被误杀。
        let exe_path = args.split_whitespace().next().unwrap_or(args).to_lowercase();
        let exe_name = exe_path.rsplit('/').next().unwrap_or(&exe_path).to_string();

        // 匹配应用可执行名（如 /Applications/FlClash.app/Contents/MacOS/FlClash → flclash）
        for (key, label) in FOREIGN_PROXY_APPS {
            if exe_path.contains(key) || exe_name == *key {
                found.push((pid, (*label).to_string()));
                break;
            }
        }
        // 匹配内核进程名
        for core in FOREIGN_PROXY_CORES {
            if exe_name.contains(core) {
                if !found.iter().any(|(p, _)| *p == pid) {
                    found.push((pid, format!("{} 内核", core)));
                }
                break;
            }
        }
    }
    found
}

/// 启动魔法代理前，清理所有第三方代理：
/// 1) 杀掉第三方代理进程（先温和 TERM，1.5 秒后仍在则 KILL）
/// 2) 关闭系统代理设置，让网络回到"未设代理"的干净状态
///
/// 返回清理掉的进程描述列表（供 UI 提示）。
fn cleanup_foreign_proxies() -> Vec<String> {
    let victims = find_foreign_proxies();
    let mut cleaned = Vec::new();
    if victims.is_empty() {
        // 没有第三方进程，但仍要确保系统代理是干净状态
        let _ = system_proxy::set_system_proxy(false, 0);
        return cleaned;
    }

    // 第一轮：SIGTERM（温和退出，让代理软件自己清理系统代理设置和防火墙规则）
    for (pid, label) in &victims {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-TERM", &pid.to_string()])
            .output();
        cleaned.push(format!("{} (PID {})", label, pid));
    }

    // 等待进程退出，最多 1.5 秒
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // 第二轮：仍在运行的升级为 SIGKILL
    for (pid, _) in &victims {
        let alive = std::process::Command::new("/bin/ps")
            .args(["-p", &pid.to_string()])
            .output()
            .map(|o| !o.stdout.is_empty() && String::from_utf8_lossy(&o.stdout).lines().count() > 1)
            .unwrap_or(false);
        if alive {
            let _ = std::process::Command::new("/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .output();
        }
    }

    // 关闭系统代理，回到干净状态（第三方软件可能残留了代理指向）
    let _ = system_proxy::set_system_proxy(false, 0);

    cleaned
}

#[tauri::command]
fn check_conflicts() -> ConflictInfo {
    let mut messages = Vec::new();
    // 1) 检测正在运行的第三方代理程序（FlClash / Clash / 外部 mihomo）
    let foreign = find_foreign_proxies();
    if !foreign.is_empty() {
        let names: Vec<String> = foreign.iter().map(|(_, l)| l.clone()).collect();
        messages.push(format!("检测到正在运行的第三方代理程序：{}", names.join("、")));
    }
    // 2) 检测本程序要用的混合端口是否已被占用（排除自己的 runtime 内核）
    let port = MihomoManager::new().port;
    if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
        let runtime = MihomoManager::new().runtime_dir;
        let runtime_str = runtime.to_string_lossy().to_string();
        let own = std::process::Command::new("/bin/ps")
            .args(["-axo", "args="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if !own.contains(&runtime_str) {
            messages.push(format!("端口 {} 已被其他程序占用", port));
        }
    }
    ConflictInfo { has_conflict: !messages.is_empty(), messages }
}

/// 供前端调用的「一键清理第三方代理」命令：
/// 杀掉所有第三方代理进程 + 关闭系统代理，把系统恢复到干净状态。
#[tauri::command]
fn kill_foreign_proxies() -> Vec<String> {
    cleanup_foreign_proxies()
}

/// 查询当前有哪些第三方代理在运行（供 UI 展示）。
#[tauri::command]
fn list_foreign_proxies() -> Vec<String> {
    find_foreign_proxies().into_iter().map(|(p, l)| format!("{} (PID {})", l, p)).collect()
}

#[tauri::command]
fn fetch_subscription(url: String) -> Result<Vec<crate::config::ProxyNode>, String> {
    // 只允许 http/https，防止 curl 访问 file:// 等本地协议造成敏感信息外泄
    let trimmed = url.trim();
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("订阅地址必须是 http:// 或 https:// 链接".to_string());
    }
    // SSRF 防护：解析出 host 并拒绝回环/内网/链路本地/组播地址，
    // 防止恶意前端借本命令拉取内网内容（如 127.0.0.1 服务、192.168.x 设备）。
    if let Some(rest) = trimmed.split_once("://").map(|(_, r)| r) {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = authority
            .rsplit_once('@') // 去掉 userinfo（user:pass@host）
            .map(|(_, h)| h)
            .unwrap_or(authority);
        if let Err(e) = crate::config::validate_public_host(host) {
            return Err(e);
        }
    }
    // 用系统 curl 拉取订阅内容
    let out = std::process::Command::new("/usr/bin/curl")
        .arg("-sL")
        .arg("--max-time").arg("15")
        // 拉订阅必须走真实链路，不受环境代理变量（HTTP_PROXY 等）劫持
        .arg("--noproxy").arg("*")
        .arg("-A").arg("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)")
        .arg(trimmed)
        .output()
        .map_err(|e| format!("调用 curl 失败: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("拉取订阅失败: {}", if err.is_empty() { "HTTP 错误".to_string() } else { err }));
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    crate::config::parse_vless_subscription(&text)
}

struct AppState {
    config: AppConfig,
    mihomo: MihomoManager,
    /// 已安装 App 列表缓存（scan_apps 时更新，get_status 只读长度，避免重扫描卡界面）
    apps_cache: Mutex<Vec<crate::apps::AppEntry>>,
    /// 用户是否期望代理在运行（start_proxy 置 true，stop_proxy 置 false）。
    /// 看门狗线程据此判断 mihomo 崩溃后是否需要自动重启。
    should_run: Arc<AtomicBool>,
}

#[tauri::command]
fn get_status(state: tauri::State<Arc<Mutex<AppState>>>, ssh: tauri::State<crate::ssh::SshManager>) -> AppStatus {
    let g = state.lock().unwrap();
    let m = g.mihomo.status();
    let apps_count = g.apps_cache.lock().unwrap().len();
    AppStatus {
        proxy_running: m.running,
        proxy_pid: m.pid,
        proxy_port: m.port,
        system_proxy: system_proxy::status().enabled,
        apps_count,
        nodes_count: g.config.nodes.len(),
        ssh: ssh.status(),
    }
}

#[tauri::command]
fn get_config(state: tauri::State<Arc<Mutex<AppState>>>) -> AppConfig {
    let mut cfg = state.lock().unwrap().config.clone();
    // 安全：apiSecret 绝不下发到前端 JS。前端不直接连 mihomo API，
    // 统一走 proxy_api 命令由 Rust 后端持有 secret 转发，防止 XSS 窃取控制密钥。
    cfg.api_secret = None;
    cfg
}

/// 前端通过本命令访问 mihomo 控制 API，secret 由后端持有并注入，
/// 前端 JS 永远拿不到 secret，也无法直接 fetch 19091（CSP 已限制）。
/// path 形如 "/connections" 或 "/proxies/<name>/delay?timeout=5000&url=..."。
/// method 目前支持 "GET"（默认）与 "PUT"。
/// 返回 (http_status, body_string)。
#[tauri::command]
fn proxy_api(
    state: tauri::State<Arc<Mutex<AppState>>>,
    path: String,
    method: Option<String>,
    body: Option<String>,
) -> Result<(u16, String), String> {
    // 路径必须以 / 开头，防止被拼成完整 URL（如 http://attacker.com）
    // 但 mihomo delay 接口的 query 参数 url=http://... 含 ://，必须放行：
    // 只检查问号前的路径段不含 ://，query 段允许含 ://。
    if !path.starts_with('/') {
        return Err("非法的 API 路径".to_string());
    }
    let path_part = path.split('?').next().unwrap_or("");
    if path_part.contains("://") {
        return Err("非法的 API 路径".to_string());
    }
    // 拒绝换行：path 直接拼进 HTTP 请求行，含 \r\n 会注入额外请求头/请求走私
    if path.contains('\r') || path.contains('\n') {
        return Err("非法的 API 路径".to_string());
    }
    let secret = {
        let g = state.lock().unwrap();
        g.config.api_secret.clone().unwrap_or_default()
    };
    let method = method.unwrap_or_else(|| "GET".to_string()).to_uppercase();
    if method != "GET" && method != "PUT" {
        return Err("不支持的方法".to_string());
    }
    let body = body.unwrap_or_default();
    // 通过 TcpStream 直连 127.0.0.1:19091 转发，secret 只在后端内存/本地传递
    let addr = ("127.0.0.1", crate::mihomo::API_PORT);
    let mut stream = std::net::TcpStream::connect(addr)
        .map_err(|e| format!("无法连接代理控制 API：{e}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .map_err(|e| e.to_string())?;

    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {secret}\r\nConnection: close\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {len}\r\n\r\n{body}",
        method = method,
        path = path,
        port = crate::mihomo::API_PORT,
        secret = secret,
        len = body.len(),
        body = body,
    );
    use std::io::{Read, Write};
    stream.write_all(req.as_bytes()).map_err(|e| e.to_string())?;

    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).map_err(|e| e.to_string())?;

    // 解析状态码与 body（按字节解析，避免 from_utf8_lossy 对非法字节的替换
    // 导致 chunked 长度切片错位）
    let (status, body_str) = parse_http_response(&resp);
    Ok((status, body_str))
}

/// 极简 HTTP 响应解析：返回 (状态码, body)。
/// 按字节解析，正确处理两种编码：
///   - Content-Length：按长度截取 body；
///   - Transfer-Encoding: chunked：逐个 chunk 块解码（mihomo 控制 API 对大响应
///     如 /connections 必然走 chunked，旧实现用 split("\r\n\r\n") 会把 chunk
///     长度块 25ca\r\n 混进 body，导致前端 JSON.parse 失败）。
/// body 仅在最后一步才从字节转 String（lossy），因此 chunk 长度按字节切分不会
/// 因中文字符或非法字节替换而错位。
fn parse_http_response(raw: &[u8]) -> (u16, String) {
    let status = raw
        .split(|&b| b == b'\n')
        .next()
        .and_then(|l| {
            let l = String::from_utf8_lossy(l);
            l.split_whitespace().nth(1).and_then(|s| s.parse::<u16>().ok())
        })
        .unwrap_or(0);

    // 拆出头与体（HTTP 头以 \r\n\r\n 结束；容忍 \n\n 的变体）
    let (head, body_raw): (&[u8], &[u8]) = if let Some(i) = find_subslice(raw, b"\r\n\r\n") {
        (&raw[..i], &raw[i + 4..])
    } else if let Some(i) = find_subslice(raw, b"\n\n") {
        (&raw[..i], &raw[i + 2..])
    } else {
        (&raw[..0], &raw[..0])
    };
    let head_lower = String::from_utf8_lossy(head).to_ascii_lowercase();

    // chunked 编码：逐块解码，块格式为 "<hex长度>\r\n<数据>\r\n"，以 "0\r\n\r\n" 结束
    if head_lower.contains("transfer-encoding: chunked") {
        let mut out: Vec<u8> = Vec::new();
        let mut rest = body_raw;
        loop {
            // 取长度行（十六进制，可能带 chunk 扩展，用分号分隔）
            let Some(nl) = find_subslice(rest, b"\r\n") else { break };
            let size_str = String::from_utf8_lossy(&rest[..nl]);
            let size_str = size_str.split(';').next().unwrap_or("").trim();
            let Ok(size) = usize::from_str_radix(size_str, 16) else { break };
            rest = &rest[nl + 2..];
            if size == 0 {
                break; // 终止块
            }
            if rest.len() < size {
                break;
            }
            out.extend_from_slice(&rest[..size]);
            rest = &rest[size..];
            // 跳过块尾的 \r\n
            if rest.starts_with(b"\r\n") {
                rest = &rest[2..];
            }
        }
        return (status, String::from_utf8_lossy(&out).to_string());
    }

    // Content-Length：按长度精确截取
    if let Some(cl) = head_lower
        .lines()
        .find(|l| l.trim_start().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        let n = cl.min(body_raw.len());
        return (status, String::from_utf8_lossy(&body_raw[..n]).to_string());
    }

    // 兜底：无显式长度，直接返回整个 body
    (status, String::from_utf8_lossy(body_raw).to_string())
}

/// 在字节切片中查找子切片，返回起始索引。
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

#[tauri::command]
fn save_config(state: tauri::State<Arc<Mutex<AppState>>>, config: AppConfig) -> Result<AppConfig, String> {
    // 只在读取 secret/运行状态时短暂持锁；热更新要全机扫描 App + lsof（秒级），
    // 全程持锁会堵死 get_status 轮询，表现为每次保存配置界面卡死数秒。
    let (api_secret, running) = {
        let g = state.lock().unwrap();
        (g.config.api_secret.clone(), g.mihomo.status().running)
    };
    let mut config = config;
    // 前端拿不到 apiSecret（get_config 已置空），这里必须保留后端持有的原 secret，
    // 否则每次保存都会把 secret 覆盖成 None，导致控制 API 鉴权失效。
    // 防御空串：前端若传 apiSecret: ""（空串而非 null），is_none 判 false 不会补回，
    // secret 会被空串覆盖导致鉴权失效。None 或空串都视为「未提供」，补回旧 secret。
    let secret_missing = config
        .api_secret
        .as_deref()
        .map(|s| s.is_empty())
        .unwrap_or(true);
    if secret_missing {
        config.api_secret = api_secret;
    }
    // 安全：SSH 明文密码/私钥内容绝不落盘 config.json。
    // 密码只存 Keychain；私钥内容只存 Keychain，config 里至多保留私钥「路径」。
    config.ssh_password = None;
    if config.ssh_private_key.as_deref().map(|k| k.contains('\n')).unwrap_or(false) {
        config.ssh_private_key = None;
    }
    // 如果代理正在运行，热更新 rules，让新保存的分流/域名规则立即生效（不重启、不弹授权框）
    let reload_err = if running {
        let app_rules = effective_app_rules(&config);
        let m = MihomoManager::new();
        m.reload_rules(&config, &app_rules).err()
    } else {
        None
    };
    state.lock().unwrap().config = config.clone();
    config::save(&config)?;
    if let Some(e) = reload_err {
        // 热更新失败不阻塞保存，但要把错误返回给前端提示
        return Err(format!("配置已保存，但规则热更新失败：{}", e));
    }
    // 返回给前端的 config 同样不能带 secret
    config.api_secret = None;
    Ok(config)
}

#[tauri::command]
fn scan_apps(state: tauri::State<Arc<Mutex<AppState>>>) -> Vec<crate::apps::AppEntry> {
    let mut list = crate::apps::scan_macos_apps();
    let g = state.lock().unwrap();
    for app in list.iter_mut() {
        if let Some(setting) = g.config.apps.iter().find(|s| s.id == app.id) {
            app.mode = setting.mode.clone();
            app.confirmed = setting.confirmed;
            app.node = setting.node.clone();
        }
    }
    // 更新缓存，供 get_status 轻量读取数量
    *g.apps_cache.lock().unwrap() = list.clone();
    list
}

/// 生成生效的软件分流规则：只处理用户已确认（confirmed）的条目。
/// 返回 (路径前缀列表, 目标) 列表；目标 = DIRECT | NODE-<节点名> | PROXY(当前选中节点)。
fn effective_app_rules(config: &AppConfig) -> Vec<(Vec<String>, String)> {
    let apps = crate::apps::scan_macos_apps();
    let mut by_id = std::collections::HashMap::new();
    for a in apps {
        by_id.insert(a.id, a.rule_paths);
    }
    // 现存节点名集合：软件分流的 node 引用已删除节点时降级为 PROXY
    let valid_nodes: std::collections::HashSet<String> =
        config.nodes.iter().map(|n| n.name.clone()).collect();
    settings_to_app_rules(&config.apps, &by_id, &valid_nodes)
}

/// 纯函数版规则生成（供 effective_app_rules 和 dump_conf bin 共用）。
/// bin-<绝对路径> 直接用 id 后半段；app-<Name> 查 path_lookup（App 扫描结果）。
pub fn settings_to_app_rules(
    settings: &[crate::config::AppSetting],
    path_lookup: &std::collections::HashMap<String, Vec<String>>,
    valid_nodes: &std::collections::HashSet<String>,
) -> Vec<(Vec<String>, String)> {
    let mut out = Vec::new();
    for setting in settings {
        if !setting.confirmed {
            continue; // 未确认的软件不进规则表，由 MATCH,DIRECT 兜底
        }
        let paths = if let Some(bin) = setting.id.strip_prefix("bin-") {
            vec![bin.to_string()]
        } else {
            match path_lookup.get(&setting.id) {
                Some(p) => p.clone(),
                None => continue,
            }
        };
        let target = if setting.mode == "proxy" {
            match &setting.node {
                // 节点名已被删除则降级为 PROXY，避免引用不存在的 NODE-xxx 组
                // 用与组名定义处一致的 sanitize_node_name，保证组名与引用一致（中文保留、逗号剔除）
                Some(n) if !n.trim().is_empty() && valid_nodes.contains(n.trim()) => {
                    format!("NODE-{}", crate::mihomo::sanitize_node_name(n.trim()))
                }
                _ => "PROXY".to_string(),
            }
        } else {
            "DIRECT".to_string()
        };
        out.push((paths, target));
    }
    out
}

#[tauri::command]
fn start_proxy(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<MihomoStatus, String> {
    // 只在读取配置时短暂持锁；启动本身（停旧进程+等API）可达数十秒，
    // 全程持锁会卡死 get_status 轮询，表现为界面"按了没反应"。
    let (cfg, already_running) = {
        let g = state.lock().unwrap();
        (g.config.clone(), g.mihomo.status().running)
    };
    if already_running {
        let m = MihomoManager::new();
        return Ok(m.status());
    }
    // 启动前自动清理所有第三方代理：杀掉 FlClash/Clash 等进程 + 关闭系统代理，
    // 让系统回到「未设代理」的干净状态，再启动本程序的代理。
    // 这样用户点一次启动就能拿到干净的代理环境，不必手动去关别的软件。
    // 只清理端口冲突和已知第三方代理；本程序自己的进程和内核会被跳过。
    cleanup_foreign_proxies();
    // 无节点时 mihomo 的 fallback 组 proxies 为空，mihomo -t 会拒绝整个配置，
    // 给出晦涩的 YAML 校验错误。提前拦截，提示用户先加节点。
    if cfg.nodes.is_empty() {
        return Err("尚未添加任何代理节点，请先到「云服务器」页添加节点或拉取订阅".to_string());
    }
    let app_rules = effective_app_rules(&cfg);
    let mihomo = MihomoManager::new();
    let status = mihomo.start(&cfg, &[], &app_rules)?;
    if cfg.system_proxy {
        let _ = system_proxy::set_system_proxy(true, mihomo.port);
    }
    // 把 PID 记回共享状态（status() 靠端口探测兜底，这里仅保持一致性）
    if let Some(pid) = status.pid {
        *state.lock().unwrap().mihomo.pid.lock().unwrap() = Some(pid);
    }
    // 通知看门狗：用户期望代理在运行，mihomo 崩溃后应自动重启
    state.lock().unwrap().should_run.store(true, Ordering::Relaxed);
    Ok(status)
}

#[tauri::command]
fn stop_proxy(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<(), String> {
    // 通知看门狗：用户主动停止，不要自动重启
    state.lock().unwrap().should_run.store(false, Ordering::Relaxed);
    let port = state.lock().unwrap().mihomo.port;
    // 锁外执行停止，避免阻塞状态轮询
    let mihomo = MihomoManager::new();
    mihomo.stop();
    *state.lock().unwrap().mihomo.pid.lock().unwrap() = None;
    let _ = system_proxy::set_system_proxy(false, port);
    Ok(())
}

#[tauri::command]
fn set_system_proxy(state: tauri::State<Arc<Mutex<AppState>>>, enabled: bool) -> Result<crate::system_proxy::SystemProxyStatus, String> {
    // networksetup 可能耗时，锁外执行避免阻塞状态轮询
    let port = state.lock().unwrap().mihomo.port;
    let status = system_proxy::set_system_proxy(enabled, port)?;
    // 同步更新并持久化 config.system_proxy，使 UI 开关与 config 一致。
    // 旧实现只调 networksetup 不改 config，导致 UI 开关后 start_proxy 仍按旧 config 判断，
    // 重启后系统代理状态与用户上次选择脱节。
    let cfg = {
        let mut g = state.lock().unwrap();
        g.config.system_proxy = enabled;
        g.config.clone()
    };
    if let Err(e) = config::save(&cfg) {
        eprintln!("[set_system_proxy] 持久化失败（不影响本次设置）: {e}");
    }
    Ok(status)
}

#[tauri::command]
fn ssh_connect(state: tauri::State<Arc<Mutex<AppState>>>, ssh: tauri::State<crate::ssh::SshManager>, host: String, port: u16, user: String, auth: String, password: Option<String>, key: Option<String>) -> Result<crate::ssh::SshSession, String> {
    // 连接+认证验证在锁外进行（最长十余秒），全程持锁会卡死状态轮询。
    // connect 内部已验证登录结果：认证失败会返回 Err，下面的 Keychain/配置写入不会执行，
    // 错误密码/私钥因此永远不会被存进 Keychain 污染后续连接。
    let session = ssh.connect(host.clone(), port, user.clone(), auth.clone(), password.clone(), key.clone())?;

    // 密码/私钥内容安全存入 Keychain，config 不落明文。
    // 注意：SSH 会话此时已经成功建立（ssh.connect 已通过认证）。
    // Keychain 存储是「持久化凭据」的副作用，不能让它把整个 ssh_connect 拉成 Err
    // ——否则用户会看到"连接失败"，但后台 session 已 live，下次 connect 才被 disconnect
    // 清理，期间会变成孤儿会话。Keychain 写失败时只标记 password_saved=false，
    // 让用户在 UI 上看到「已连接但凭据未保存」，而不是误报连接失败。
    let password_saved = if auth == "password" {
        match password {
            Some(p) if !p.trim().is_empty() => {
                match keychain::store(&crate::ssh::SshManager::password_account(&host, &user), &p) {
                    Ok(_) => true,
                    Err(e) => {
                        eprintln!("[ssh] 保存密码到 Keychain 失败（不影响本次连接）: {e}");
                        false
                    }
                }
            }
            _ => keychain::exists(&crate::ssh::SshManager::password_account(&host, &user)),
        }
    } else {
        false
    };
    let private_key_saved = if auth == "key" {
        match key.as_ref() {
            Some(k) if !k.trim().is_empty() => {
                // 如果是路径，不存内容；如果是私钥内容（多行），存 Keychain
                if k.contains('\n') {
                    match keychain::store(&crate::ssh::SshManager::key_account(&host, &user), k) {
                        Ok(_) => true,
                        Err(e) => {
                            eprintln!("[ssh] 保存私钥到 Keychain 失败（不影响本次连接）: {e}");
                            false
                        }
                    }
                } else {
                    false // 路径形式，直接引用路径
                }
            }
            _ => keychain::exists(&crate::ssh::SshManager::key_account(&host, &user)),
        }
    } else {
        false
    };

    // 更新 servers 列表
    let mut g = state.lock().unwrap();
    let id = format!("ssh-{}@{}", user, host);
    let info = crate::config::ServerInfo {
        id: id.clone(),
        name: host.clone(),
        host: host.clone(),
        port,
        user: user.clone(),
        auth: auth.clone(),
        password_saved,
        private_key_saved,
        key_path: key.as_ref().filter(|k| !k.contains('\n')).cloned(),
    };
    if let Some(existing) = g.config.servers.iter_mut().find(|s| s.id == id) {
        *existing = info.clone();
    } else {
        g.config.servers.push(info);
    }
    g.config.active_server_id = Some(id);

    // 兼容旧字段（不再存明文密码）
    g.config.ssh_host = Some(host);
    g.config.ssh_port = Some(port);
    g.config.ssh_user = Some(user);
    g.config.ssh_auth = Some(auth);
    g.config.ssh_password = None;
    g.config.ssh_private_key = None;
    // config 持久化失败不应让已成功的连接变成"失败"（session 已建立），
    // 但也不能静默吞掉——内存有服务器、磁盘没有，重启后消失且 Keychain 留孤儿凭据。
    if let Err(e) = config::save(&g.config) {
        eprintln!("[ssh_connect] config 持久化失败（不影响本次连接）: {e}");
    }
    Ok(session)
}

#[tauri::command]
fn select_ssh_server(state: tauri::State<Arc<Mutex<AppState>>>, server_id: String) -> Result<crate::config::ServerInfo, String> {
    let mut g = state.lock().unwrap();
    let server = g.config.servers.iter().find(|s| s.id == server_id)
        .cloned()
        .ok_or("服务器不存在")?;
    g.config.active_server_id = Some(server.id.clone());
    g.config.ssh_host = Some(server.host.clone());
    g.config.ssh_port = Some(server.port);
    g.config.ssh_user = Some(server.user.clone());
    g.config.ssh_auth = Some(server.auth.clone());
    g.config.ssh_password = None;
    g.config.ssh_private_key = server.key_path.clone();
    config::save(&g.config)?;
    Ok(server)
}

#[tauri::command]
fn delete_ssh_server(state: tauri::State<Arc<Mutex<AppState>>>, ssh: tauri::State<crate::ssh::SshManager>, server_id: String) -> Result<(), String> {
    let mut g = state.lock().unwrap();
    let idx = g.config.servers.iter().position(|s| s.id == server_id).ok_or("服务器不存在")?;
    let server = g.config.servers.remove(idx);
    keychain::delete(&crate::ssh::SshManager::password_account(&server.host, &server.user));
    keychain::delete(&crate::ssh::SshManager::key_account(&server.host, &server.user));
    // 若删的正是当前激活服务器，且 SSH 交互式会话还连着它，必须同步断开——
    // 否则用户删完服务器在「控制台」页依然看到"已连接"，且底层 ssh 进程仍持有
    // 该服务器凭据的会话，与"已删除"语义矛盾，存在凭据残留风险。
    let is_active = g.config.active_server_id.as_deref() == Some(server.id.as_str());
    if is_active {
        // session.id 形如 "ssh-<user>@<host>"，与 server.id 同形，直接比对即可
        let sid = ssh.session.lock().unwrap().as_ref().map(|s| s.id.clone());
        if sid.as_deref() == Some(server.id.as_str()) {
            ssh.disconnect();
        }
    }
    if g.config.active_server_id.as_deref() == Some(server.id.as_str()) {
        if let Some(next) = g.config.servers.first().cloned() {
            g.config.active_server_id = Some(next.id.clone());
            g.config.ssh_host = Some(next.host);
            g.config.ssh_port = Some(next.port);
            g.config.ssh_user = Some(next.user);
            g.config.ssh_auth = Some(next.auth);
            g.config.ssh_password = None;
            g.config.ssh_private_key = next.key_path;
        } else {
            g.config.active_server_id = None;
            g.config.ssh_host = None;
            g.config.ssh_port = Some(22);
            g.config.ssh_user = Some("root".to_string());
            g.config.ssh_auth = Some("password".to_string());
            g.config.ssh_password = None;
            g.config.ssh_private_key = None;
        }
    }
    config::save(&g.config)?;
    Ok(())
}

#[tauri::command]
fn ssh_write(ssh: tauri::State<crate::ssh::SshManager>, data: Vec<u8>) -> Result<(), String> {
    ssh.write(data)
}

#[tauri::command]
fn ssh_read(ssh: tauri::State<crate::ssh::SshManager>) -> Result<Vec<u8>, String> {
    ssh.read()
}

#[tauri::command]
fn ssh_disconnect(ssh: tauri::State<crate::ssh::SshManager>) -> Result<(), String> {
    ssh.disconnect();
    Ok(())
}

/// 在「当前激活的云服务器」上非交互式执行一条命令，返回 (stdout, stderr, exit_code)。
/// 智能体 / 前端仪表盘用来远程探测服务器状态（CPU/内存/磁盘/带宽），不污染交互式终端。
#[tauri::command]
fn ssh_exec(state: tauri::State<Arc<Mutex<AppState>>>, command: String, timeout_secs: Option<u64>) -> Result<(String, String, i32), String> {
    let (host, port, user, auth, key_path) = {
        let g = state.lock().unwrap();
        let s = g.config.active_server().ok_or("尚未配置云服务器：请先在「云服务器」页添加 SSH 连接")?;
        (s.host.clone(), s.port, s.user.clone(), s.auth.clone(), s.key_path.clone())
    };
    let timeout = timeout_secs.unwrap_or(15).clamp(5, 60);
    // 锁外执行（SSH 可能耗时数秒，避免卡住状态轮询）
    let ssh = crate::ssh::SshManager::new();
    ssh.exec(host, port, user, auth, command, timeout, key_path)
}

/// 云服务器一键探针：采集 CPU、内存、磁盘、网络带宽、负载、在线时长。
/// 返回结构化 JSON 给前端仪表盘 / 智能体（MCP 也走同一逻辑）。
#[tauri::command]
fn server_metrics(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<serde_json::Value, String> {
    let (host, port, user, auth, key_path) = {
        let g = state.lock().unwrap();
        let s = g.config.active_server().ok_or("尚未配置云服务器")?;
        (s.host.clone(), s.port, s.user.clone(), s.auth.clone(), s.key_path.clone())
    };
    let ssh = crate::ssh::SshManager::new();
    let cmd = r#"
echo '---CPU---'; top -bn1 | grep 'Cpu(s)' || echo 'n/a'
echo '---MEM---'; free -m | grep -E 'Mem|内存' || echo 'n/a'
echo '---DISK---'; df -h / | tail -1 || echo 'n/a'
echo '---LOAD---'; cat /proc/loadavg 2>/dev/null || sysctl -n vm.loadavg 2>/dev/null || echo 'n/a'
echo '---UPTIME---'; uptime | sed 's/^ *//' || echo 'n/a'
echo '---NET---'; cat /proc/net/dev | grep -E 'eth0|ens|enp' | head -5 || echo 'n/a'
"#;
    let (out, _err, code) = ssh.exec(host.clone(), port, user.clone(), auth.clone(), cmd.to_string(), 20, key_path)?;
    if code != 0 {
        return Err(format!("探针执行失败 (exit {code}): {}", _err));
    }
    Ok(parse_server_metrics(&out))
}

/// 解析探针原始输出为结构化 JSON（前端/智能体直接消费）。
fn parse_server_metrics(raw: &str) -> serde_json::Value {
    use serde_json::json;
    let mut m = serde_json::Map::new();

    let mut section = "";
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("---") && t.ends_with("---") {
            section = t.trim_matches('-').trim();
            continue;
        }
        match section {
            "CPU" => {
                // top: %Cpu(s):  us, sy, ni, id, wa...
                if t.starts_with("Cpu") || t.starts_with("%Cpu") || t.contains("us,") {
                    let id = extract_pct(t, "id");
                    m.insert("cpu_usage_pct".into(), json!(100.0 - id));
                }
            }
            "MEM" => {
                // free -m:  total used free shared buff/cache available
                let cols: Vec<&str> = t.split_whitespace().collect();
                if cols.len() >= 7 {
                    if let (Ok(total), Ok(used), Ok(avail)) =
                        (cols[1].parse::<f64>(), cols[2].parse::<f64>(), cols[6].parse::<f64>())
                    {
                        m.insert("mem_total_mb".into(), json!(total));
                        m.insert("mem_used_mb".into(), json!(used));
                        m.insert("mem_avail_mb".into(), json!(avail));
                        m.insert("mem_usage_pct".into(), json!((used / total * 100.0 * 10.0).round() / 10.0));
                    }
                }
            }
            "DISK" => {
                // df -h: Filesystem Size Used Avail Use% Mounted
                let cols: Vec<&str> = t.split_whitespace().collect();
                if cols.len() >= 5 {
                    m.insert("disk_size".into(), json!(cols[1]));
                    m.insert("disk_used".into(), json!(cols[2]));
                    m.insert("disk_avail".into(), json!(cols[3]));
                    m.insert("disk_usage_pct".into(), json!(cols[4].trim_end_matches('%')));
                }
            }
            "LOAD" => {
                // loadavg: 0.12 0.09 0.08 1/123 456
                let cols: Vec<&str> = t.split_whitespace().collect();
                if cols.len() >= 3 {
                    m.insert("load_1m".into(), json!(cols[0]));
                    m.insert("load_5m".into(), json!(cols[1]));
                    m.insert("load_15m".into(), json!(cols[2]));
                }
            }
            "UPTIME" => {
                m.insert("uptime".into(), json!(t));
            }
            "NET" => {
                // eth0: 1234 5 0 0 0 0 0 0 5678 9 ...  (RX 累计字节在第1列，TX 在第9列)
                if let Some(colon) = t.find(':') {
                    let ifname = t[..colon].trim().to_string();
                    let nums: Vec<&str> = t[colon + 1..].split_whitespace().collect();
                    if nums.len() >= 10 {
                        let rx = nums[0].parse::<f64>().unwrap_or(0.0);
                        let tx = nums[8].parse::<f64>().unwrap_or(0.0);
                        m.insert(format!("net_{}_rx_bytes", ifname), json!(rx));
                        m.insert(format!("net_{}_tx_bytes", ifname), json!(tx));
                    }
                }
            }
            _ => {}
        }
    }
    m.insert("probe_ok".into(), json!(true));
    serde_json::Value::Object(m)
}

fn extract_pct(line: &str, key: &str) -> f64 {
    // top 的 CPU 行形如 "%Cpu(s):  5.2 us,  3.1 sy,  0.0 ni, 89.8 id,  1.9 wa"
    // 每段是 "<值> <字段名>"，字段名可能带尾逗号。抓 key 前紧邻的浮点数。
    for part in line.split(',') {
        let p = part.trim();
        let mut prev_val: Option<f64> = None;
        for w in p.split_whitespace() {
            let field = w.trim_end_matches(',');
            if field == key {
                if let Some(v) = prev_val { return v; }
            }
            prev_val = w.trim_end_matches(',').trim_end_matches('%').parse::<f64>().ok();
        }
    }
    0.0
}

#[tauri::command]
fn self_test(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<String, String> {
    let g = state.lock().unwrap();
    let bin = g.mihomo.bin_path_for_test();
    let geo = g.mihomo.geo_dir_for_test();
    let mut lines = Vec::new();
    lines.push(format!("mihomo_bin: {}", bin.display()));
    lines.push(format!("mihomo_exists: {}", bin.exists()));
    lines.push(format!("geo_dir: {}", geo.display()));
    lines.push(format!("geo_dir_exists: {}", geo.exists()));
    Ok(lines.join("\n"))
}


pub fn start_proxy_standalone() -> Result<MihomoStatus, String> {
    let cfg = config::load();
    let mihomo = MihomoManager::new();
    let app_rules = effective_app_rules(&cfg);
    let status = mihomo.start(&cfg, &[], &app_rules)?;
    if cfg.system_proxy {
        let _ = system_proxy::set_system_proxy(true, mihomo.port);
    }
    Ok(status)
}

pub fn stop_proxy_standalone() -> Result<(), String> {
    let mihomo = MihomoManager::new();
    let port = mihomo.port;
    // 首选：特权控制器零弹窗（已安装 sudoers 白名单时）
    if MihomoManager::ctl("stop").is_some() {
        let _ = system_proxy::set_system_proxy(false, port);
        return Ok(());
    }
    // start_proxy_standalone 与 stop_proxy_standalone 各自创建实例无法共享 pid，
    // 这里改为按启动参数（runtime 下的 mihomo.yaml）精确查找并提权结束 mihomo 进程。
    let runtime = mihomo.runtime_dir;
    let conf_path = runtime.join("mihomo.yaml");
    let conf_str = conf_path.to_string_lossy().to_string();
    let mut pids = Vec::new();
    if let Ok(out) = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,args="])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("mihomo") && line.contains(&conf_str) {
                if let Some(pid_str) = line.split_whitespace().next() {
                    if let Ok(pid) = pid_str.parse::<i32>() {
                        pids.push(pid);
                    }
                }
            }
        }
    }
    for pid in pids {
        // mihomo 以 root 运行，普通 kill 会被拒；用 osascript 提权 kill
        let script = format!(
            "do shell script \"/bin/kill {}\" with administrator privileges",
            pid
        );
        let _ = std::process::Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&script)
            .output();
    }
    let _ = system_proxy::set_system_proxy(false, port);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let should_run = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(AppState {
        config: config::load(),
        mihomo: MihomoManager::new(),
        apps_cache: Mutex::new(Vec::new()),
        should_run: should_run.clone(),
    }));

    // mihomo 看门狗：每 30s 检查一次。用户启动代理后 should_run=true，
    // 若 mihomo 崩溃（端口探测失败）则自动拉起——但只走特权控制器零弹窗路径
    // （ctl("start")），不弹 osascript 反复骚扰用户。重启失败则关掉系统代理，
    // 避免 mihomo 死了但系统代理仍指向 127.0.0.1:7891 导致全机断网。
    let watchdog_state = state.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            if !should_run.load(Ordering::Relaxed) { continue; }
            let mihomo = MihomoManager::new();
            if mihomo.status().running { continue; }
            eprintln!("[watchdog] mihomo 已停止但 should_run=true，尝试自动重启...");
            if let Some(pid_str) = MihomoManager::ctl("start") {
                if pid_str == "already-running" || pid_str.parse::<u32>().is_ok() {
                    if mihomo.wait_api() {
                        eprintln!("[watchdog] mihomo 自动重启成功");
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            *watchdog_state.lock().unwrap().mihomo.pid.lock().unwrap() = Some(pid);
                        }
                        continue;
                    }
                }
            }
            // 重启失败：关掉系统代理，避免死代理端口导致全机断网
            eprintln!("[watchdog] mihomo 自动重启失败，关闭系统代理以恢复直连");
            let port = watchdog_state.lock().unwrap().mihomo.port;
            let _ = system_proxy::set_system_proxy(false, port);
        }
    });

    // 启动时自动清理第三方代理：App 一打开就把系统里其他代理（FlClash/Clash 等）
    // 全部关掉，并把系统代理恢复为「未设置」，让网络回到最初干净状态。
    // 这样用户打开本软件即成为系统唯一代理，不会有多个代理抢流量/抢端口。
    // 放在后台线程执行，避免阻塞窗口显示（清理含 1.5 秒等待）。
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(800));
        let cleaned = cleanup_foreign_proxies();
        if !cleaned.is_empty() {
            eprintln!("[startup] 已清理第三方代理: {}", cleaned.join("、"));
        }
    });

    tauri::Builder::default()
        .manage(state)
        // SSH 会话单独管理：连接验证（最长十余秒）与终端读写不经过配置大锁，
        // 避免连接期间/终端高频 IO 卡住 get_status 轮询
        .manage(crate::ssh::SshManager::new())
        // 自动更新：updater 下载 .app.tar.gz + Ed25519 验签 + 整体覆盖；
        // dialog 用于弹「发现新版本」原生对话框；process 用于安装后 relaunch 重启
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_config,
            save_config,
            scan_apps,
            start_proxy,
            stop_proxy,
            set_system_proxy,
            ssh_connect,
            ssh_write,
            ssh_read,
            ssh_disconnect,
            ssh_exec,
            server_metrics,
            select_ssh_server,
            delete_ssh_server,
            self_test,
            check_conflicts,
            kill_foreign_proxies,
            list_foreign_proxies,
            fetch_subscription,
            proxy_api
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // ── 退出收尾钩子（2026-09-03 事故根因修复）──
            // 铁律：App 生命周期必须完整覆盖内核生命周期——关 App = 关代理 + 关系统代理。
            // 此前 App 退出没有任何清理钩子，root mihomo 变成孤儿进程继续用 TUN
            // 接管全机流量，劫持其他应用（WorkBuddy 中转站请求被掐成 ECONNRESET）。
            // RunEvent::Exit 在所有退出路径（关窗、Cmd+Q、app.exit()、系统注销）必经。
            if let tauri::RunEvent::Exit = event {
                eprintln!("[magic-agent] RunEvent::Exit：开始收尾（停内核+断SSH+关系统代理）");
                let state = app.state::<Arc<Mutex<AppState>>>();
                // 锁可能被毒化（其他线程持锁 panic），退出路径绝不能再 panic
                let g = state.lock().unwrap_or_else(|e| e.into_inner());
                // 先通知看门狗停止，避免退出时它检测到 mihomo 已死又自动拉起
                g.should_run.store(false, Ordering::Relaxed);
                g.mihomo.stop();
                // 断开 SSH 会话：不断开则 ssh/expect 子进程变孤儿，继续占着远端连接
                app.state::<crate::ssh::SshManager>().disconnect();
                let _ = system_proxy::set_system_proxy(false, g.mihomo.port);
                eprintln!("[magic-agent] RunEvent::Exit：收尾完成");
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_response_chunked() {
        // mihomo /connections 实际返回：Transfer-Encoding: chunked
        // 注意：chunk 长度必须与实际数据字节数一致（这里 body 就是完整内容）
        let body_json = "{\"downloadTotal\":18428227,\"connections\":[]}";
        let size = format!("{:x}", body_json.len());
        let raw = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{size}\r\n{body_json}\r\n0\r\n\r\n"
        );
        let (status, body) = parse_http_response(raw.as_bytes());
        assert_eq!(status, 200);
        // 关键：chunk 长度块与终止块都被剥掉，body 是纯 JSON
        assert!(!body.contains("\r\n0\r\n"), "终止块未被剥离");
        assert_eq!(body, body_json);
        // 可被 JSON 解析
        let _: serde_json::Value = serde_json::from_str(&body).expect("body 应是合法 JSON");
    }

    #[test]
    fn parse_http_response_content_length() {
        // mihomo 401 实际返回：body 含结尾换行，Content-Length 精确匹配
        let raw = "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: 27\r\n\r\n{\"message\":\"Unauthorized\"}\n";
        let (status, body) = parse_http_response(raw.as_bytes());
        assert_eq!(status, 401);
        assert_eq!(body, "{\"message\":\"Unauthorized\"}\n");
    }

    #[test]
    fn parse_http_response_multi_chunk() {
        // 多个 chunk 块拼接
        let raw = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
                   5\r\nhello\r\n\
                   6\r\n world\r\n\
                   0\r\n\r\n";
        let (_, body) = parse_http_response(raw.as_bytes());
        assert_eq!(body, "hello world");
    }

    #[test]
    fn parse_http_response_chunked_with_utf8() {
        // chunk 内含多字节 UTF-8（中文），按字节切分不能错位
        let body_json = "{\"chains\":[\"示例节点2\",\"NODE-示例节点2\"]}";
        let size = format!("{:x}", body_json.len());
        let raw = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{size}\r\n{body_json}\r\n0\r\n\r\n"
        );
        let (_, body) = parse_http_response(raw.as_bytes());
        assert_eq!(body, body_json);
        let _: serde_json::Value = serde_json::from_str(&body).expect("UTF-8 body 应可解析");
    }
}

#[cfg(test)]
mod server_metrics_tests {
    use super::*;

    #[test]
    fn parse_metrics_linux() {
        let raw = r#"---CPU---
%Cpu(s):  5.2 us,  3.1 sy,  0.0 ni, 89.8 id,  1.9 wa,  0.0 hi,  0.0 si,  0.0 st
---MEM---
              total        used        free      shared  buff/cache   available
Mem:            1986         823         214          42         948        1038
---DISK---
/dev/vda1        59G   28G   29G  50% /
---LOAD---
0.12 0.09 0.08 1/123 456
---UPTIME---
12:34:56 up 10 days,  3:04,  1 user,  load average: 0.12, 0.09, 0.08
---NET---
eth0: 1234567890 1000 0 0 0 0 0 0 9876543210 2000 0 0 0 0 0 0
"#;
        let m = parse_server_metrics(raw);
        assert_eq!(m["probe_ok"], true);
        assert!((m["cpu_usage_pct"].as_f64().unwrap() - 10.2).abs() < 0.01);
        assert_eq!(m["mem_total_mb"], 1986.0);
        assert_eq!(m["mem_used_mb"], 823.0);
        assert_eq!(m["disk_usage_pct"], "50");
        assert_eq!(m["load_1m"], "0.12");
        assert_eq!(m["net_eth0_rx_bytes"], 1234567890.0);
        assert_eq!(m["net_eth0_tx_bytes"], 9876543210.0);
    }

    #[test]
    fn parse_metrics_empty_sections() {
        let raw = "---CPU---\nn/a\n---MEM---\nn/a\n---DISK---\nn/a\n";
        let m = parse_server_metrics(raw);
        assert_eq!(m["probe_ok"], true);
        assert!(m.get("cpu_usage_pct").is_none());
    }
}
