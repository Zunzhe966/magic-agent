mod apps;
// config / mihomo 对 bin 工具（dump_conf）公开
pub mod config;
mod keychain;
pub mod mihomo;
mod ssh;
mod system_proxy;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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

#[tauri::command]
fn check_conflicts() -> ConflictInfo {
    let mut messages = Vec::new();
    // 本程序自己启动的 mihomo 会带 runtime 目录参数，遇到时跳过，避免误报自身
    let runtime = MihomoManager::new().runtime_dir;
    let runtime_str = runtime.to_string_lossy().to_string();
    // 1) 检测正在运行的第三方代理程序（FlClash / Clash / 外部 mihomo）
    if let Ok(ps) = std::process::Command::new("/bin/ps").args(["-axo", "args="]).output() {
        let text = String::from_utf8_lossy(&ps.stdout);
        let mut foreign = false;
        for line in text.lines() {
            let lower = line.to_lowercase();
            let hit = lower.contains("mihomo") || lower.contains("clash") || lower.contains("flclash");
            if !hit { continue; }
            // 跳过本程序 runtime 目录启动的 mihomo
            if line.contains(&runtime_str) {
                continue;
            }
            foreign = true;
        }
        if foreign {
            messages.push("检测到正在运行的第三方代理程序（FlClash/Clash/mihomo），请先关闭".to_string());
        }
    }
    // 2) 检测本程序要用的混合端口是否已被占用。
    //    如果占用端口的是本程序 runtime 目录的 mihomo，不算冲突。
    let port = MihomoManager::new().port;
    if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
        let own = std::process::Command::new("/bin/ps")
            .args(["-axo", "args="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        if !own.contains("mihomo") || !own.contains(&runtime_str) {
            messages.push(format!("端口 {} 已被其他程序占用，请先关闭冲突程序", port));
        }
    }
    ConflictInfo { has_conflict: !messages.is_empty(), messages }
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
    ssh: crate::ssh::SshManager,
    /// 已安装 App 列表缓存（scan_apps 时更新，get_status 只读长度，避免重扫描卡界面）
    apps_cache: Mutex<Vec<crate::apps::AppEntry>>,
}

#[tauri::command]
fn get_status(state: tauri::State<Arc<Mutex<AppState>>>) -> AppStatus {
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
        ssh: g.ssh.status(),
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
    // 只允许绝对路径形式，防止被拼成任意 URL（如 http://attacker.com）
    if !path.starts_with('/') || path.contains("://") {
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
    let mut g = state.lock().unwrap();
    // 前端拿不到 apiSecret（get_config 已置空），这里必须保留后端持有的原 secret，
    // 否则每次保存都会把 secret 覆盖成 None，导致控制 API 鉴权失效。
    let mut config = config;
    if config.api_secret.is_none() {
        config.api_secret = g.config.api_secret.clone();
    }
    g.config = config.clone();
    config::save(&config)?;
    // 如果代理正在运行，热更新 rules，让新保存的分流/域名规则立即生效（不重启、不弹授权框）
    if g.mihomo.status().running {
        let app_rules = effective_app_rules(&config);
        if let Err(e) = g.mihomo.reload_rules(&config, &app_rules) {
            // 热更新失败不阻塞保存，但要把错误返回给前端提示
            return Err(format!("配置已保存，但规则热更新失败：{}", e));
        }
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
    // 检测第三方代理冲突：FlClash/mihomo 占用端口时直接提示，避免抢端口
    let conflict = check_conflicts();
    if conflict.has_conflict {
        return Err(conflict.messages.join("；"));
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
    Ok(status)
}

#[tauri::command]
fn stop_proxy(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<(), String> {
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
    let g = state.lock().unwrap();
    system_proxy::set_system_proxy(enabled, g.mihomo.port)
}

#[tauri::command]
fn ssh_connect(state: tauri::State<Arc<Mutex<AppState>>>, host: String, port: u16, user: String, auth: String, password: Option<String>, key: Option<String>) -> Result<crate::ssh::SshSession, String> {
    let mut g = state.lock().unwrap();
    let session = g.ssh.connect(host.clone(), port, user.clone(), auth.clone(), password.clone(), key.clone())?;

    // 密码/私钥内容安全存入 Keychain，config 不落明文
    let password_saved = if auth == "password" {
        match password {
            Some(p) if !p.trim().is_empty() => {
                keychain::store(&crate::ssh::SshManager::password_account(&host, &user), &p)?;
                true
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
                    keychain::store(&crate::ssh::SshManager::key_account(&host, &user), k)?;
                    true
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
    let _ = config::save(&g.config);
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
fn delete_ssh_server(state: tauri::State<Arc<Mutex<AppState>>>, server_id: String) -> Result<(), String> {
    let mut g = state.lock().unwrap();
    let idx = g.config.servers.iter().position(|s| s.id == server_id).ok_or("服务器不存在")?;
    let server = g.config.servers.remove(idx);
    keychain::delete(&crate::ssh::SshManager::password_account(&server.host, &server.user));
    keychain::delete(&crate::ssh::SshManager::key_account(&server.host, &server.user));
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
fn ssh_write(state: tauri::State<Arc<Mutex<AppState>>>, data: Vec<u8>) -> Result<(), String> {
    let g = state.lock().unwrap();
    g.ssh.write(data)
}

#[tauri::command]
fn ssh_read(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<Vec<u8>, String> {
    let g = state.lock().unwrap();
    g.ssh.read()
}

#[tauri::command]
fn ssh_disconnect(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<(), String> {
    let g = state.lock().unwrap();
    g.ssh.disconnect();
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
    let state = Arc::new(Mutex::new(AppState {
        config: config::load(),
        mihomo: MihomoManager::new(),
        ssh: crate::ssh::SshManager::new(),
        apps_cache: Mutex::new(Vec::new()),
    }));
    tauri::Builder::default()
        .manage(state)
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
            fetch_subscription,
            proxy_api
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
        let body_json = "{\"chains\":[\"Texas住宅\",\"NODE-Texas住宅\"]}";
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
