use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MihomoStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub node: Option<String>,
}

/// 本程序内核的控制 API 固定端口（external-controller）
pub const API_PORT: u16 = 19091;

/// 「坐飞机」端口：进去后无条件走代理（PROXY），mihomo 不做国内外自动分流。
/// 智能体自己决定要不要走这条路——把需要出国的请求直接送进这个口。
pub const PROXY_PORT: u16 = 7893;

/// 「坐火车」端口：进去后无条件直连（DIRECT），绝不碰任何节点。
/// 智能体自己决定走国内直连时送进这个口。
pub const DIRECT_PORT: u16 = 7892;

/// 保命直连名单（域名后缀）：这些目标永远走系统原生路由，绝不进 TUN、绝不进代理端口。
/// 尤其 AI 助手本体访问的中转站——模型请求是超长流式 JSON，一旦被 TUN/应用层代理"拆-组"
/// 就会损坏请求体（表现为 400 Invalid JSON body）。必须让它从网卡直接出网。
const PROTECTED_DIRECT_DOMAINS: &[&str] = &[
    // WorkBuddy 接的中转站（ai-relay），模型请求必须原样透传
    "203.0.113.74",
];

pub struct MihomoManager {
    /// mihomo 以 root 权限启动（TUN 需要），无法作为普通子进程管理，记录 PID 即可
    pub pid: Mutex<Option<u32>>,
    pub port: u16,
    pub runtime_dir: PathBuf,
}

fn process_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl MihomoManager {
    pub fn new() -> Self {
        let runtime_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("magic-agent")
            .join("runtime");
        Self { pid: Mutex::new(None), port: 7891, runtime_dir }
    }

    pub fn status(&self) -> MihomoStatus {
        let mut guard = self.pid.lock().unwrap();
        let mut alive = guard.map(process_alive).unwrap_or(false);
        if !alive {
            *guard = None;
        }
        // 如果 PID 不在（例如 mihomo 由外部/MCP 启动），但混合端口和控制 API 都开放，
        // 才认为代理在运行——只查混合端口会把占用 7891 的第三方代理误判成自己。
        if !alive
            && TcpStream::connect(("127.0.0.1", self.port)).is_ok()
            && TcpStream::connect(("127.0.0.1", API_PORT)).is_ok()
        {
            alive = true;
        }
        MihomoStatus {
            running: alive,
            pid: *guard,
            port: self.port,
            node: None,
        }
    }

    /// 启动 mihomo。
    /// TUN 模式需要 root 权限，通过 osascript 弹管理员授权后以 root 启动。
    /// app_rules: (路径前缀列表, 目标) 列表，目标为 "DIRECT" 或 "NODE-<节点名>" / "PROXY"。
    pub fn start(&self, cfg: &AppConfig, rules: &[String], app_rules: &[(Vec<String>, String)]) -> Result<MihomoStatus, String> {
        self.stop();
        let _ = std::fs::create_dir_all(&self.runtime_dir);
        self.ensure_runtime_bin()?;
        self.copy_geo_files()?;
        let conf = self.build_conf(cfg, rules, app_rules);
        self.write_conf(&conf)?;

        // 首选：特权控制器零弹窗启动（已安装 sudoers 白名单时）
        if let Some(pid_str) = Self::ctl("start") {
            if let Ok(pid) = pid_str.parse::<u32>() {
                *self.pid.lock().unwrap() = Some(pid);
                if self.wait_api() {
                    return Ok(MihomoStatus { running: true, pid: Some(pid), port: self.port, node: cfg.selected_node.clone() });
                }
                return Err(format!("内核已启动（PID {pid}）但控制 API 未就绪"));
            }
            if pid_str == "already-running" && self.wait_api() {
                // 已有实例在跑（如 runtime 常驻副本），接管它
                return Ok(MihomoStatus { running: true, pid: None, port: self.port, node: cfg.selected_node.clone() });
            }
            // ctl 启动失败则继续走 osascript 弹窗路径
        }

        let bin = self.bin_path();
        if !bin.exists() {
            return Err(format!("代理内核不存在: {}", bin.display()));
        }

        let log_path = self.runtime_dir.join("mihomo.log");
        let err_path = self.runtime_dir.join("mihomo.err.log");
        let conf_path = self.runtime_dir.join("mihomo.yaml");
        // 日志轮转：超 10MB 改名为 .old（启动前执行，运行中的旧进程持有 fd 不受影响）
        for p in [&log_path, &err_path] {
            if let Ok(meta) = std::fs::metadata(p) {
                if meta.len() > 10 * 1024 * 1024 {
                    let old = p.with_extension("log.old");
                    let _ = std::fs::remove_file(&old);
                    let _ = std::fs::rename(p, &old);
                }
            }
        }
        // 直接 & 后台启动并输出 $!（后台进程 PID），由 osascript 以管理员权限执行。
        // 注意：do shell script 会等待前台命令结束，但 & 让 mihomo 立即后台化，$! 被 echo 返回。
        // 不能用 nohup：osascript 的 shell 没有 TTY，nohup 会报 "can't detach from console"。
        let shell_cmd = format!(
            "'{}' -f '{}' -d '{}' > '{}' 2> '{}' & echo $!",
            bin.display(),
            conf_path.display(),
            self.runtime_dir.display(),
            log_path.display(),
            err_path.display()
        );
        let escaped = shell_cmd
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let apple_script = format!("do shell script \"{}\" with administrator privileges", escaped);

        let out = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&apple_script)
            .output()
            .map_err(|e| format!("调用 osascript 请求管理员权限失败: {e}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if err.to_lowercase().contains("canceled") || err.to_lowercase().contains("cancel") {
                return Err("用户取消了管理员授权，代理内核未启动".to_string());
            }
            return Err(format!("以管理员权限启动 mihomo 失败: {}", err));
        }
        let pid_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let pid: u32 = pid_str
            .parse()
            .map_err(|e| format!("解析 mihomo PID 失败（返回: {:?}）: {e}", pid_str))?;
        *self.pid.lock().unwrap() = Some(pid);

        let mut ok = false;
        for _ in 0..150 {
            if TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                ok = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if !ok {
            // 启动失败时清理
            let _ = Command::new("/bin/kill").arg(pid.to_string()).output();
            *self.pid.lock().unwrap() = None;
            let err_tail = std::fs::read_to_string(&err_path).unwrap_or_default();
            let err_suffix: String = err_tail.chars().rev().take(600).collect::<String>().chars().rev().collect();
            return Err(format!(
                "代理端口 {} 未能在 30 秒内就绪，mihomo 可能启动失败。错误日志末尾：{}",
                self.port,
                if err_suffix.trim().is_empty() { "空" } else { err_suffix.trim() }
            ));
        }
        Ok(MihomoStatus {
            running: true,
            pid: Some(pid),
            port: self.port,
            node: cfg.selected_node.clone(),
        })
    }

    /// 热更新配置：写完整 YAML 到 mihomo.yaml，然后提权发送 SIGHUP 让 mihomo 重载。
    /// 不用 PATCH /configs——实测 mihomo 的 PATCH 对多条 PROCESS-PATH-REGEX 只保留第一条。
    pub fn reload_rules(&self, cfg: &AppConfig, app_rules: &[(Vec<String>, String)]) -> Result<(), String> {
        // 1. 写完整配置
        let conf = self.build_conf(cfg, &[], app_rules);
        self.write_conf(&conf)?;

        // 首选：特权控制器零弹窗重载（与 start/stop 一致，装了 sudoers 白名单时无弹窗）
        if let Some(out) = Self::ctl("reload") {
            // ctl reload 通过 pkill -HUP 发送，成功后无需再走 osascript 提权
            if !out.trim().is_empty() || Self::ctl("status").is_some() {
                return Ok(());
            }
        }

        // 2. 找 mihomo PID
        let out = Command::new("/bin/ps")
            .args(["-axo", "pid=,args="])
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        let conf_path = self.runtime_dir.join("mihomo.yaml");
        let conf_str = conf_path.to_string_lossy().to_string();
        let mut pid: Option<String> = None;
        for line in text.lines() {
            if line.contains("mihomo") && line.contains(&conf_str) {
                pid = line.split_whitespace().next().map(|s| s.to_string());
                break;
            }
        }
        let Some(pid) = pid else {
            return Err("未找到运行中的 mihomo 进程".to_string());
        };

        // 3. 提权发送 SIGHUP 重载配置
        let script = format!("do shell script \"/bin/kill -HUP {}\" with administrator privileges", pid);
        let out = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("调用 osascript 失败: {e}"))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(format!("SIGHUP 重载失败（用户可能取消了授权）: {}", err));
        }
        Ok(())
    }

    /// 生成规则列表。三层漏斗模型，mihomo 规则首条命中生效，顺序即优先级：
    ///   第1层 进程级（谁进隧道）：防卷优先——云服务器自身流量永远直连；
    ///   第2层 域名级（去了哪）：显式域名规则 → 国内域名/IP（GEOSITE,cn + GEOIP,CN）直连；
    ///   第3层 节点级（谁来送）：进程规则把该软件剩余流量交给指定节点，兜底直连。
    pub fn build_rules_for(&self, cfg: &AppConfig, app_rules: &[(Vec<String>, String)]) -> Vec<String> {
        self.build_rules(cfg, &[], app_rules)
    }
    fn build_rules(&self, cfg: &AppConfig, rules: &[String], app_rules: &[(Vec<String>, String)]) -> Vec<String> {
        let mut out = Vec::new();

        // ── 第 -1 层：两条「路」的死锁分流（IN-PORT 双保险）──
        // 用户要求 mihomo 不自动判断国内外，而是智能体自己选「坐飞机(走代理) / 坐火车(直连)」。
        // listeners 的 proxy 字段已能强制死锁；这里再加 IN-PORT 规则双保险，确保无论
        // mihomo 版本对 listeners.proxy 的语义如何，从 7893 进来的流量都无条件 PROXY、
        // 从 7892 进来的都无条件 DIRECT，优先级高于下面所有规则（含 GEOSITE,cn）。
        out.push(format!("IN-PORT,{},PROXY", PROXY_PORT));
        out.push(format!("IN-PORT,{},DIRECT", DIRECT_PORT));

        // ── 第0层：保命直连名单。这些目标永远走系统原生路由（DIRECT），
        // 优先级高于一切规则、域名、节点。无论梯子开/关、重载，都不允许影响它们。
        for d in PROTECTED_DIRECT_DOMAINS {
            out.push(format!("IP-CIDR,{}/32,DIRECT,no-resolve", d));
        }

        // ── 第1层：防卷。节点服务器自身的流量绝不能进隧道，否则死循环 ──
        let mut seen_servers = std::collections::HashSet::new();
        for node in &cfg.nodes {
            if !seen_servers.insert(node.server.clone()) {
                continue; // 多节点共用同一服务器时只生成一次
            }
            // server 含非法字符（换行/逗号/引号等）视为恶意输入，跳过，绝不生成规则
            let server = sanitize_rule_field(&node.server);
            if server.is_empty() || server != node.server.trim() {
                continue;
            }
            if server.parse::<std::net::IpAddr>().is_ok() {
                out.push(format!("IP-CIDR,{}/32,DIRECT,no-resolve", server));
            } else {
                out.push(format!("DOMAIN-SUFFIX,{},DIRECT", server));
            }
        }

        // ── 第2层：显式域名规则（用户与 AI 探讨后定死的），优先于一切自动判断 ──
        // target 支持 "direct" | "proxy" | 节点名（映射为 NODE-<节点名> 组）
        for dr in &cfg.domain_rules {
            // 域名若含非法字符（换行/逗号/引号等），视为恶意/损坏输入，整条丢弃，绝不生成规则
            let domain = sanitize_rule_field(&dr.domain);
            if domain.is_empty() || domain != dr.domain.trim() {
                continue;
            }
            let target = match dr.target.as_str() {
                "direct" => "DIRECT".to_string(),
                "proxy" => "PROXY".to_string(),
                other => {
                    // 节点名已被删除则降级为 PROXY（fallback 组恒存在），
                    // 否则 mihomo 因引用不存在的 proxy-group 而拒绝整个配置。
                    if cfg.nodes.iter().any(|n| n.name == other) {
                        // 用与组名定义处完全一致的 sanitize_node_name，保证组名与引用一致
                        format!("NODE-{}", sanitize_node_name(other))
                    } else {
                        "PROXY".to_string()
                    }
                }
            };
            out.push(format!("DOMAIN-SUFFIX,{},{}", domain, target));
        }

        // ── 第2层续：国内域名/IP 清单直连。解决"软件设为代理后访问百度绕美国"的问题 ──
        out.push("GEOSITE,cn,DIRECT".to_string());
        out.push("GEOIP,CN,DIRECT".to_string());

        // ── 第3层：进程规则（软件默认去向）。隧道内该软件未被上面命中的流量按此转发 ──
        for (paths, target) in app_rules {
            for p in paths {
                out.push(format!("PROCESS-PATH-REGEX,^{},{}", regex_escape_path(p), target));
            }
        }

        // 额外自定义规则（预留）
        for r in rules {
            out.push(r.clone());
        }

        out.push("GEOIP,LAN,DIRECT,no-resolve".to_string());
        out.push("MATCH,DIRECT".to_string());
        out
    }

    /// 按启动参数（runtime 下的 mihomo.yaml）查找运行中的 mihomo PID。
    /// App 重启后内存里的 PID 丢失，靠这个兜底才能停掉/重启代理。
    fn find_running_pid(&self) -> Option<u32> {
        let conf_path = self.runtime_dir.join("mihomo.yaml");
        let conf_str = conf_path.to_string_lossy().to_string();
        let out = Command::new("/bin/ps")
            .args(["-axo", "pid=,args="])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("mihomo") && line.contains(&conf_str) {
                if let Some(pid_str) = line.split_whitespace().next() {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        return Some(pid);
                    }
                }
            }
        }
        None
    }

    pub fn stop(&self) {
        // 首选：特权控制器零弹窗（已安装 sudoers 白名单时；沙箱/未安装则回退）
        if Self::ctl("stop").is_some() {
            *self.pid.lock().unwrap() = None;
            return;
        }
        // 内存 PID 优先；App 重启后 PID 丢失，退回按配置路径查找
        let pid_opt = self.pid.lock().unwrap().take().or_else(|| self.find_running_pid());
        if let Some(pid) = pid_opt {
            let _ = Command::new("/bin/kill").arg(pid.to_string()).output();
            // 等待进程退出，最多 10 秒
            for _ in 0..100 {
                if !process_alive(pid) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
       }
    }

    /// 调用特权控制器（sudo -n，免弹窗）。返回 Some(stdout)=成功。
    pub fn ctl(action: &str) -> Option<String> {
        const CTL: &str = "/usr/local/lib/magic-agent/mihomo-ctl.sh";
        if !std::path::Path::new(CTL).exists() {
            return None;
        }
        let out = Command::new("sudo")
            .arg("-n")
            .arg(CTL)
            .arg(action)
            .output()
            .ok()?;
        if out.status.success() {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            None
        }
    }

    fn copy_geo_files(&self) -> Result<(), String> {
        let src_dir = self.resource_root().join("geo");
        if !src_dir.exists() {
            return Ok(());
        }
        let files = ["geoip.dat", "geosite.dat", "ASN.mmdb", "geoip.metadb"];
        for name in files {
            let src = src_dir.join(name);
            let dst = self.runtime_dir.join(name);
            if src.exists() {
                if let Err(e) = std::fs::copy(&src, &dst) {
                    return Err(format!("复制 {} 失败: {e}", name));
                }
            }
        }
        Ok(())
    }

    pub fn bin_path_for_test(&self) -> PathBuf {
        self.bin_path()
    }

    /// 等待控制 API 端口就绪，最多 15 秒
    fn wait_api(&self) -> bool {
        for _ in 0..150 {
            if std::net::TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    pub fn geo_dir_for_test(&self) -> PathBuf {
        self.resource_root().join("geo")
    }

    fn resource_root(&self) -> PathBuf {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        if dev.exists() {
            return dev;
        }
        let mut exe = std::env::current_exe().unwrap_or_default();
        exe.pop();
        exe.pop();
        exe.pop();
        exe.push("Resources");
        exe
    }

    fn bin_path(&self) -> PathBuf {
        // 优先用 runtime 常驻副本：App 升级替换 .app 时不会触碰正在运行的内核
        let runtime_copy = self.runtime_dir.join("bin/mihomo");
        if runtime_copy.exists() {
            return runtime_copy;
        }
        let p = self.resource_root().join("bin/mihomo");
        if p.exists() {
            return p;
        }
        PathBuf::from("/usr/local/bin/mihomo")
    }

    /// 把内核复制到 runtime/bin 常驻（幂等，内容一致则跳过）。
    /// 目的：升级替换 .app 不影响运行中的 mihomo。
    fn ensure_runtime_bin(&self) -> Result<(), String> {
        let src = self.resource_root().join("bin/mihomo");
        if !src.exists() {
            return Ok(()); // 无内置内核（如 /usr/local/bin 安装），无需复制
        }
        let dst_dir = self.runtime_dir.join("bin");
        let _ = std::fs::create_dir_all(&dst_dir);
        let dst = dst_dir.join("mihomo");
        let need = match std::fs::read(&dst) {
            Ok(cur) => cur != std::fs::read(&src).unwrap_or_default(),
            Err(_) => true,
        };
        if need {
            std::fs::copy(&src, &dst).map_err(|e| format!("复制内核到 runtime 失败: {e}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755));
            }
        }
        Ok(())
    }

    /// 写 mihomo.yaml 并收紧为 0600（内含控制 API secret、节点 UUID、reality 公钥，
    /// 0644 会让本机其他用户可读，属信息泄露）。
    fn write_conf(&self, conf: &str) -> Result<(), String> {
        let conf_path = self.runtime_dir.join("mihomo.yaml");
        std::fs::write(&conf_path, conf).map_err(|e| format!("写配置失败: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&conf_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn build_conf(&self, cfg: &AppConfig, rules: &[String], app_rules: &[(Vec<String>, String)]) -> String {
        let mut out = String::new();
        out.push_str(&format!("mixed-port: {}\n", self.port));
        out.push_str("mode: rule\n");
        // TUN 只接管「该走代理」的流量，绝不碰直连流量：
        //   - auto-route: false —— 不再改系统默认路由，避免把所有流量（含 AI 助手的直连请求）
        //     兜进虚拟网卡再"进-出"一圈导致请求体被污染。
        //   - strict-route: true —— 只按规则把明确要代理的流量拉进 TUN，其余走系统原生直连。
        out.push_str("tun:\n  enable: true\n  stack: system\n  auto-route: false\n  strict-route: true\n  auto-detect-interface: true\n  dns-hijack:\n    - any:53\n");
        // ── 两条物理上分开的「路」，决策权在智能体，不在 mihomo ──
        // 用户明确要求：不要 mihomo 自动判断国内外分流，而是给智能体两条明确的路，
        // 智能体当场看清路况后自己拍板——坐飞机（走代理）还是坐火车（直连），
        // 然后把请求直接送进对应的口，进去后不再被二次判断。
        //   proxy-only  → PROXY_PORT (7893)：无条件 PROXY，连国内域名也强制走节点。
        //   direct-only → DIRECT_PORT (7892)：无条件 DIRECT，绝不碰节点。
        out.push_str(&format!(
            "listeners:\n  - name: proxy-only\n    type: mixed\n    port: {}\n    proxy: PROXY\n  - name: direct-only\n    type: mixed\n    port: {}\n    proxy: DIRECT\n",
            PROXY_PORT, DIRECT_PORT
        ));
        out.push_str("log-level: info\n");
        out.push_str("allow-lan: false\n");
        out.push_str("ipv6: false\n");
        out.push_str("find-process-mode: always\n");
        out.push_str("external-controller: 127.0.0.1:19091\n");
        // 控制 API 鉴权：本机任意进程/网页 CSRF 都可能打这个端口，必须带 secret
        if let Some(s) = cfg.api_secret.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!("secret: {}\n", s));
        }
        out.push_str("geo-auto-update: false\n");
        out.push_str("geodata-mode: false\n");
        out.push_str("geodata-loader: memconservative\n\n");
        // DNS 全加密：国内 DoH + 境外 DoT。国内明文 DNS（223.5.5.5）会看到用户解析了哪些域名，
        // 属于隐私泄漏。全部改成加密通道，国内 DNS 看不到任何明文查询。
        out.push_str("dns:\n  enable: true\n  listen: 127.0.0.1:1054\n  enhanced-mode: redir-host\n  nameserver:\n    - https://dns.alidns.com/dns-query\n    - https://doh.pub/dns-query\n  fallback:\n    - tls://8.8.8.8\n    - tls://1.1.1.1\n  fallback-filter:\n    geoip: true\n    geoip-code: CN\n\n");

        out.push_str("proxies:\n");
        for node in &cfg.nodes {
            out.push_str(&format!(
                "  - name: \"{}\"\n    type: vless\n    server: \"{}\"\n    port: {}\n    uuid: \"{}\"\n    network: {}\n    tls: {}\n    udp: {}\n    flow: \"{}\"\n    client-fingerprint: \"{}\"\n",
                yaml_quote(&node.name), yaml_quote(&node.server), node.port, yaml_quote(&node.uuid),
                yaml_quote(&node.network), node.tls, node.udp, yaml_quote(&node.flow), yaml_quote(&node.fingerprint)
            ));
            if !node.sni.is_empty() {
                out.push_str(&format!("    servername: \"{}\"\n", yaml_quote(&node.sni)));
            }
            if !node.public_key.is_empty() {
                out.push_str(&format!(
                    "    reality-opts:\n      public-key: \"{}\"\n      short-id: \"{}\"\n",
                    yaml_quote(&node.public_key), yaml_quote(&node.short_id)
                ));
            }
        }

        out.push_str("\nproxy-groups:\n");
        // PROXY 组：fallback 类型——选中节点排第一优先，探测失败自动落到下一个可用节点（自动故障转移）
        // switch_node 依然生效：重排顺序后 PUT /configs 重载，选中节点回到第一位
        out.push_str("  - name: PROXY\n    type: fallback\n    url: http://www.gstatic.com/generate_204\n    interval: 60\n    proxies:\n");
        let selected = cfg.selected_node.clone().unwrap_or_default();
        let mut ordered: Vec<&crate::config::ProxyNode> = cfg.nodes.iter().collect();
        ordered.sort_by_key(|n| if n.name == selected { 0 } else { 1 });
        for node in ordered {
            out.push_str(&format!("      - \"{}\"\n", yaml_quote(&node.name)));
        }
        // 每个节点独立成组：软件/域名分流规则可以精确指向"走哪个节点"（钉死，不参与故障转移）
        // 组名必须与 build_rules 里的规则引用用同一净化结果（sanitize_node_name），否则对不上。
        for node in &cfg.nodes {
            let gname = sanitize_node_name(&node.name);
            out.push_str(&format!(
                "  - name: \"NODE-{}\"\n    type: select\n    proxies:\n      - \"{}\"\n",
                yaml_quote(&gname), yaml_quote(&node.name)
            ));
        }

        out.push_str("\nrules:\n");
        // rules 段与热更新共用同一份生成逻辑，避免两处不一致
        for r in self.build_rules(cfg, rules, app_rules) {
            out.push_str(&format!("  - {}\n", r));
        }
        out
    }
}

/// 把任意字符串安全地嵌入 YAML 双引号字符串，防止恶意订阅注入配置。
/// 转义反斜杠、双引号、换行、制表符及其他控制字符，杜绝 `"` 或 `\n` 打破
/// 字符串边界、注入额外配置行（如改 allow-lan、去 secret、加恶意 DNS）。
fn yaml_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// 净化规则字段（域名/IP）。mihomo 规则是逗号分隔、换行分隔多条，
/// 外部可控的订阅字段若含 `,` 或 `\n` 会注入额外规则。这里只保留域名/IP
/// 的合法 ASCII 字符（字母数字、点、横线、下划线、冒号、星号、斜杠），其余剔除。
/// 注意：域名/IP 本就应只含 ASCII，非 ASCII（如中文）视为异常输入剔除是安全的。
fn sanitize_rule_field(s: &str) -> String {
    s.chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '.' | '-' | '_' | ':' | '*' | '/' | '#' | '[' | ']')
        })
        .collect()
}

/// 净化节点名（用于生成 NODE-<name> 组名及其规则引用）。
///
/// 节点名是用户可自定义的（可含中文、空格），但会同时出现在「proxy-group 的 name」
/// 和「规则的 target」两处，必须用**同一个**净化结果，否则组名与引用对不上，规则静默失效。
///
/// 与 sanitize_rule_field 的关键区别：这里**保留中文与空格**（它们对 mihomo 组名合法），
/// 只剔除会破坏规则结构的字符——逗号（规则字段分隔符）、换行/回车（行分隔符）及其他控制字符。
/// 其余可见字符（含 CJK、空格、字母数字、标点）一律保留，确保组名与引用严格一致。
pub(crate) fn sanitize_node_name(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ',' | '\n' | '\r') && !c.is_control())
        .collect()
}

fn regex_escape_path(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' | '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '?' | '*' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProxyNode;

    #[test]
    fn resource_root_exists() {
        let m = MihomoManager::new();
        let root = m.resource_root();
        assert!(root.exists(), "resource root missing: {}", root.display());
        assert!(root.join("bin/mihomo").exists(), "mihomo binary missing");
    }

    #[test]
    fn rules_follow_three_layer_funnel_order() {
        let m = MihomoManager::new();
        let node = ProxyNode {
            name: "test-node".to_string(),
            server: "203.0.113.10".to_string(),
            port: 443,
            uuid: "test-uuid".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            network: "tcp".to_string(),
            tls: true,
            udp: true,
            fingerprint: "chrome".to_string(),
            public_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            short_id: "0000000000000000".to_string(),
            sni: String::new(),
            source: "manual".to_string(),
            region: String::new(),
        };
        let cfg = AppConfig {
            nodes: vec![node],
            selected_node: Some("test-node".to_string()),
            domain_rules: vec![
                crate::config::DomainRule { domain: "github.com".into(), target: "proxy".into(), reason: String::new() },
                crate::config::DomainRule { domain: "bilibili.com".into(), target: "direct".into(), reason: String::new() },
                crate::config::DomainRule { domain: "openai.com".into(), target: "test-node".into(), reason: String::new() },
            ],
            ..Default::default()
        };
        let app_rules = vec![(vec!["/Applications/Google Chrome.app".to_string()], "PROXY".to_string())];
        let rules = m.build_rules(&cfg, &[], &app_rules);

        let pos = |s: &str| rules.iter().position(|r| r.contains(s)).expect(s);
        // 第 -1 层：IN-PORT 双保险最前（两条路的死锁分流，优先级最高）
        assert!(rules[0].starts_with(&format!("IN-PORT,{},PROXY", PROXY_PORT)));
        assert!(rules[1].starts_with(&format!("IN-PORT,{},DIRECT", DIRECT_PORT)));
        // 保命直连名单次之（第0层，先于防卷）
        let protected_idx = pos("IP-CIDR,203.0.113.74/32,DIRECT,no-resolve");
        assert!(protected_idx >= 2, "保命直连名单应在 IN-PORT 双保险之后");
        // 防卷再次之
        assert!(pos("IP-CIDR,203.0.113.10/32,DIRECT,no-resolve") > protected_idx);
        // 域名规则在 GEOSITE,cn 之前
        assert!(pos("DOMAIN-SUFFIX,github.com,PROXY") < pos("GEOSITE,cn,DIRECT"));
        assert!(pos("DOMAIN-SUFFIX,bilibili.com,DIRECT") < pos("GEOSITE,cn,DIRECT"));
        // 节点名 target 映射为 NODE- 组
        assert!(rules.iter().any(|r| r == "DOMAIN-SUFFIX,openai.com,NODE-test-node"));
        // 国内清单在进程规则之前
        assert!(pos("GEOSITE,cn,DIRECT") < pos("PROCESS-PATH-REGEX"));
        // 兜底
        assert_eq!(rules.last().unwrap(), "MATCH,DIRECT");
    }

    #[test]
    fn generated_conf_is_valid_mihomo_yaml() {
        let m = MihomoManager::new();
        let node = ProxyNode {
            name: "test-node".to_string(),
            server: "203.0.113.10".to_string(),
            port: 443,
            uuid: "test-uuid".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            network: "tcp".to_string(),
            tls: true,
            udp: true,
            fingerprint: "chrome".to_string(),
            public_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            short_id: "0000000000000000".to_string(),
            sni: String::new(),
            source: "manual".to_string(),
            region: String::new(),
        };
        let cfg = AppConfig {
            nodes: vec![node],
            selected_node: Some("test-node".to_string()),
            ..Default::default()
        };
        let conf = m.build_conf(&cfg, &[], &[]);
        // PROXY 组必须是 fallback（自动故障转移），且不再有死代码 AUTO 组
        assert!(conf.contains("  - name: PROXY\n    type: fallback\n"));
        assert!(!conf.contains("url-test"));
        // 「两条路」监听端口：坐飞机(7893)=无条件 PROXY，坐火车(7892)=无条件 DIRECT。
        // 用户明确要求 mihomo 不自动分流，决策权在智能体——这里必须死锁两个端口。
        assert!(conf.contains(&format!("listeners:\n  - name: proxy-only\n    type: mixed\n    port: {}\n    proxy: PROXY\n", PROXY_PORT)));
        assert!(conf.contains(&format!("  - name: direct-only\n    type: mixed\n    port: {}\n    proxy: DIRECT\n", DIRECT_PORT)));
        // IN-PORT 双保险：规则最前面必须有按端口死锁的两条，且优先级高于 GEOSITE,cn
        assert!(conf.contains(&format!("IN-PORT,{},PROXY", PROXY_PORT)));
        assert!(conf.contains(&format!("IN-PORT,{},DIRECT", DIRECT_PORT)));
        let ip_rule_idx = conf.find(&format!("IN-PORT,{},PROXY", PROXY_PORT)).unwrap();
        let geosite_idx = conf.find("GEOSITE,cn,DIRECT").unwrap();
        assert!(ip_rule_idx < geosite_idx, "IN-PORT 双保险必须在 GEOSITE,cn 之前");
        let tmp = std::env::temp_dir().join("magic-agent-test-conf.yaml");
        std::fs::write(&tmp, &conf).unwrap();
        let out = Command::new(m.bin_path())
            .arg("-t")
            .arg("-f")
            .arg(&tmp)
            .output()
            .expect("run mihomo -t");
        let _ = std::fs::remove_file(&tmp);
        assert!(
            out.status.success(),
            "mihomo -t rejected config:\nSTDOUT: {}\nSTDERR: {}\nCONF:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
            conf
        );
    }

    #[test]
    fn chinese_node_name_maps_to_matching_group() {
        // 回归：中文节点名不能被 sanitize 成空，导致规则引用 NODE-（不存在组）拒绝配置。
        let m = MihomoManager::new();
        let node = ProxyNode {
            name: "示例节点".to_string(),
            server: "203.0.113.10".to_string(),
            port: 443,
            uuid: "u".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            network: "tcp".to_string(),
            tls: true,
            udp: true,
            fingerprint: "chrome".to_string(),
            public_key: "pk".to_string(),
            short_id: "sid".to_string(),
            sni: String::new(),
            source: "manual".to_string(),
            region: String::new(),
        };
        let cfg = AppConfig {
            nodes: vec![node],
            selected_node: Some("示例节点".to_string()),
            domain_rules: vec![crate::config::DomainRule {
                domain: "openai.com".into(),
                target: "示例节点".into(),
                reason: String::new(),
            }],
            ..Default::default()
        };
        let conf = m.build_conf(&cfg, &[], &[]);
        // 组名定义与规则引用必须一致，且都保留中文
        assert!(conf.contains("name: \"NODE-示例节点\""), "组名应保留中文: \n{conf}");
        assert!(conf.contains("DOMAIN-SUFFIX,openai.com,NODE-示例节点"), "规则应引用完整中文组名: \n{conf}");
        // 不能出现空引用 NODE-
        assert!(!conf.contains("NODE-\n"), "不得出现空节点组引用");
    }

    #[test]
    fn node_name_with_comma_is_sanitized_consistently() {
        // 节点名含逗号会破坏规则字段分隔；组名与规则引用必须同源净化，保持一致。
        let m = MihomoManager::new();
        let node = ProxyNode {
            name: "node,a".to_string(),
            server: "203.0.113.10".to_string(),
            port: 443,
            uuid: "u".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            network: "tcp".to_string(),
            tls: true,
            udp: true,
            fingerprint: "chrome".to_string(),
            public_key: "pk".to_string(),
            short_id: "sid".to_string(),
            sni: String::new(),
            source: "manual".to_string(),
            region: String::new(),
        };
        let cfg = AppConfig {
            nodes: vec![node],
            selected_node: Some("node,a".to_string()),
            domain_rules: vec![crate::config::DomainRule {
                domain: "openai.com".into(),
                target: "node,a".into(),
                reason: String::new(),
            }],
            ..Default::default()
        };
        let conf = m.build_conf(&cfg, &[], &[]);
        // 逗号被剔除：组名和规则引用都应是 NODE-nodea
        assert!(conf.contains("name: \"NODE-nodea\""), "组名应剔除逗号: \n{conf}");
        assert!(conf.contains("DOMAIN-SUFFIX,openai.com,NODE-nodea"), "规则应引用剔除逗号后的组名: \n{conf}");
    }

    #[test]
    fn malicious_subscription_fields_are_escaped() {
        // 安全回归：恶意订阅字段（含双引号/换行/逗号）不能注入额外配置行或规则。
        let m = MihomoManager::new();
        let evil_name = "evil\"\n    type: trojan\n    server: attacker.com\n    password: hacked";
        let evil_server = "attacker.com\n    allow-lan: true";
        let node = ProxyNode {
            name: evil_name.to_string(),
            server: evil_server.to_string(),
            port: 443,
            uuid: "u\"\n".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            network: "tcp".to_string(),
            tls: true,
            udp: true,
            fingerprint: "chrome".to_string(),
            public_key: "pk\"\n".to_string(),
            short_id: "sid".to_string(),
            sni: "sni\"\n".to_string(),
            source: "subscription".to_string(),
            region: String::new(),
        };
        let cfg = AppConfig {
            nodes: vec![node],
            selected_node: None,
            domain_rules: vec![crate::config::DomainRule {
                domain: "evil.com\n    - DOMAIN-SUFFIX,injected.com,PROXY".to_string(),
                target: "proxy".to_string(),
                reason: String::new(),
            }],
            ..Default::default()
        };
        let conf = m.build_conf(&cfg, &[], &[]);
        // 恶意注入的关键载荷不能以"裸配置行"形式出现在输出里
        assert!(!conf.contains("\n    type: trojan\n"), "节点名注入了 trojan 配置");
        assert!(!conf.contains("\n    allow-lan: true"), "server 注入了 allow-lan");
        assert!(!conf.contains("\n    password: hacked"), "节点名注入了 password");
        // 转义后应包含转义序列，而不是原始换行 + 双引号
        assert!(conf.contains("\\\""), "双引号应被转义");
        assert!(conf.contains("\\n"), "换行应被转义");
        // 规则里的域名逗号/换行被净化，不能注入额外规则
        let rules = m.build_rules(&cfg, &[], &[]);
        for r in &rules {
            assert!(!r.contains('\n'), "规则不得含换行: {:?}", r);
            assert!(!r.contains("injected.com"), "域名规则注入了额外域名: {:?}", r);
        }
    }
}
