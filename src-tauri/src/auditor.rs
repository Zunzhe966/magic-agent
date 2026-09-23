// P1-1 网络体检审计引擎（docs/重构计划.md）。
//
// 定位：**纯只读采集**，零副作用——不改任何系统状态，只产出 NetworkAuditReport。
// 它是"一键归序"（P1-3）的眼睛：先看清混乱源，才谈得上接管与回滚。
//
// 设计红线（继承 CONTRACT）：
// 1. 单项采集失败/超时 → 该维度标 unknown，绝不 panic、绝不拖垮整份报告。
// 2. 解析 fixture 必须取自真机输出（教训见 system_proxy.rs::tests 注释）。
// 3. 第三方代理检测与执行侧 find_foreign_proxies 同源（lib.rs），
//    保证"体检报的"和"归序清的"是同一份名单，不存在两套口径。
// 4. 所有调用方必须走 spawn_blocking（本模块内部是串行子进程，最坏 ~10s）。

use serde::Serialize;

/// 三态：真 / 假 / 未知（采集失败）。unknown 与 false 严格区分——
/// 报"没问题"和"没查出来"是两回事，混淆即假账。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Triage {
    Yes,
    No,
    Unknown,
}

/// 单个监听套接字（lsof LISTEN 一行）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenSocket {
    pub command: String,
    pub pid: u32,
    pub user: String,
    /// 形如 "127.0.0.1:7891" / "*:7890" / "[::1]:7890"
    pub address: String,
    pub port: u16,
    /// 是否绑在非回环地址（* 或 0.0.0.0 或具体外网 IP）= 局域网可达
    pub lan_exposed: bool,
}

/// 本程序端口被第三方占用的冲突项。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortConflict {
    pub port: u16,
    pub label: String, // 用途描述：混合端口 / SOCKS 死端口 等
    pub holder_command: String,
    pub holder_pid: u32,
    /// 占用者是否是本程序自己的内核（是则不算冲突）
    pub ours: bool,
}

/// 本程序端口绑在非回环地址（局域网可达）的暴露项。
/// 与 MCP 侧 audit_network 的 ownPortLanExposed 元素【同形】：{command, pid, port}。
/// GUI 消费的是 Rust 序列化结果，字段名/大小写漂移 = 前端渲染崩溃
/// （2026-09-23 v0.3.0 体检卡黑屏事故：前端引用 ownPortLanExposed 而 Rust 无此字段）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposedPort {
    pub command: String,
    pub pid: u32,
    pub port: u16,
}

/// 网关路由采集结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteState {
    pub state: Triage, // unknown = route 命令失败
    pub gateway: String,
    pub interface: String,
    /// 默认路由是否走 utun*（TUN 类接口）
    pub tun_interface: bool,
}

/// 本程序死端口（7891/7892/7893）残留在系统代理里的检测。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleProxyState {
    /// 系统代理是否指向本程序端口但对应内核没在监听（= 崩溃残留，P0-4 同源口径）
    pub detected: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsState {
    pub state: Triage,
    /// resolver #1 的 nameserver 列表
    pub nameservers: Vec<String>,
    /// 是否全部为已知公共/加密 DNS 出口（启发式，unknown 时不评判）
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvProxyHints {
    pub vars: Vec<String>, // 形如 "http_proxy=..."（值已截断）
    pub shell_profile_hits: Vec<String>, // ~/.zshrc 等文件里的代理 export 行
}

/// 体检总报告。字段与 docs/重构计划.md P1-1 的 NetworkAuditReport 对齐。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAuditReport {
    pub ts: String,
    /// 第三方代理进程（与清理侧同源检测）
    pub foreign_procs: Vec<String>,
    pub port_conflicts: Vec<PortConflict>,
    /// 全机 LISTEN 套接字（供归序决策与 UI 展示）
    pub listen_sockets: Vec<ListenSocket>,
    /// 崩溃残留：系统代理指向本程序死端口
    pub stale_proxy: StaleProxyState,
    /// P1-2 账本视角：未结的接管账本摘要（None = 无未结账本）。
    /// 与 MCP 侧 audit_network 的 openLedger 字段同源同语义。
    pub open_ledger: Option<serde_json::Value>,
    /// 本程序端口（7891/7892/7893）中绑在非回环地址的 = 局域网可达。
    /// 前端体检卡直接消费此字段（v0.3.0 曾因缺它整页白屏崩溃）。
    /// 与 MCP 侧 ownPortLanExposed 同形。
    pub own_port_lan_exposed: Vec<ExposedPort>,
    pub route: RouteState,
    pub tun_interfaces: Vec<String>,
    pub dns: DnsState,
    pub pac_enabled: Triage,
    pub env_proxy: EnvProxyHints,
    /// 一句话结论：给 UI 头部与 MCP 摘要用
    pub summary: String,
    /// 采集过程中降级的维度名（诚实暴露信息缺口）
    pub degraded: Vec<String>,
}

const LSOF: &str = "/usr/sbin/lsof";
const ROUTE: &str = "/sbin/route";
const IFCONFIG: &str = "/sbin/ifconfig";
const SCUTIL: &str = "/usr/sbin/scutil";

/// 统一子进程执行：硬超时（SIGKILL）+ 输出捕获。Err = 启动失败/超时/非零退出。
/// 与 apps.rs::scan_network_connections 同款 try_wait 轮询模式（rust 标准库无 wait_timeout）。
fn run_capped(bin: &str, args: &[&str], cap_ms: u64) -> Result<String, String> {
    let mut child = std::process::Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 {bin} 失败: {e}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(cap_ms);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = child
                    .wait_with_output()
                    .map_err(|e| format!("读取 {bin} 输出失败: {e}"))?;
                if !status.success() {
                    return Err(format!("{bin} 退出码 {:?}", status.code()));
                }
                return Ok(String::from_utf8_lossy(&out.stdout).to_string());
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{bin} 超时 {cap_ms}ms，已放弃"));
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("{bin} try_wait 失败: {e}"));
            }
        }
    }
}

/// 解析 lsof LISTEN 输出为结构化套接字。表头行/坏行跳过（只丢该行不丢整批）。
/// 真机格式（2026-09-23 Darwin 25.6 取证）：
///   COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
///   rapportd 653 someuser 4u IPv4 ... 0t0 TCP *:49187 (LISTEN)
///   python3.1 2587 someuser 9u IPv4 ... 0t0 TCP 127.0.0.1:8000 (LISTEN)
pub fn parse_listen_sockets(text: &str) -> Vec<ListenSocket> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        // NAME 列形如 "*:49187" / "127.0.0.1:8000" / "[::1]:7890" / "*.49187"(IPv6 星号)
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 9 || cols[cols.len() - 1] != "(LISTEN)" {
            continue;
        }
        let name = cols[cols.len() - 2];
        let Some((addr, port_s)) = split_name_addr_port(name) else {
            continue;
        };
        let Ok(port) = port_s.parse::<u16>() else {
            continue;
        };
        let pid = match cols[1].parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let lan_exposed = !(addr.starts_with("127.") || addr == "::1" || addr == "[::1]" || addr == "localhost");
        // "*" / "0.0.0.0" / "[::]" / 具体外网 IP 都算局域网可达
        out.push(ListenSocket {
            command: cols[0].to_string(),
            pid,
            user: cols[2].to_string(),
            address: name.rsplit_once(" (LISTEN)").map(|(a, _)| a).unwrap_or(name).to_string(),
            port,
            lan_exposed,
        });
    }
    out
}

/// "*:49187" -> ("*", "49187"); "[::1]:7890" -> ("[::1]", "7890")
fn split_name_addr_port(name: &str) -> Option<(&str, &str)> {
    let idx = name.rfind(':')?;
    Some((&name[..idx], &name[idx + 1..]))
}

/// 默认路由解析。真机格式（2026-09-23 取证）关键字段行：
///   gateway: 172.18.100.1
///   interface: en0
pub fn parse_default_route(text: &str) -> (String, String) {
    let mut gateway = String::new();
    let mut interface = String::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("gateway:") {
            gateway = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("interface:") {
            interface = v.trim().to_string();
        }
    }
    (gateway, interface)
}

/// ifconfig -l 输出挑出 utun*。真机：空格分隔单行。
pub fn parse_utun_list(text: &str) -> Vec<String> {
    text.split_whitespace().filter(|i| i.starts_with("utun")).map(String::from).collect()
}

/// scutil --dns 提取 resolver #1 的 nameserver。真机格式（2026-09-23 取证）：
///   resolver #1
///     nameserver[0] : 223.5.5.5
pub fn parse_dns_nameservers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_first = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("resolver #") {
            if l == "resolver #1" {
                in_first = true;
            } else if in_first {
                break; // 只取 #1（系统默认出口），后续是分流 resolver
            }
        }
        if in_first {
            if let Some(rest) = l.strip_prefix("nameserver[") {
                if let Some(v) = rest.split_once(']') {
                    let ip = v.1.trim().trim_start_matches(':').trim();
                    if !ip.is_empty() {
                        out.push(ip.to_string());
                    }
                }
            }
        }
    }
    out
}

/// scutil --proxy 全局视图 → (stale 检测, pac 状态)。
/// 死端口表与 lib.rs::startup_self_check 的 P0-4 口径一致（7891/7892/7893），
/// 但本模块只**报告**不自愈——改状态是 P1-3 编排的事。
pub fn parse_proxy_global(text: &str, own_ports: &[u16], kernel_up: bool) -> (StaleProxyState, Triage) {
    let pac = if text.contains("ProxyAutoConfigEnable : 1") {
        Triage::Yes
    } else if text.contains("ProxyAutoConfigEnable : 0") {
        Triage::No
    } else {
        Triage::Unknown
    };
    let http_on = text.contains("HTTPEnable : 1");
    let socks_on = text.contains("SOCKSEnable : 1");
    let http_port = parse_scutil_port(text, "HTTPPort");
    let socks_port = parse_scutil_port(text, "SOCKSPort");
    let mut hits: Vec<String> = Vec::new();
    if !kernel_up {
        if let Some(p) = http_port {
            if http_on && own_ports.contains(&p) {
                hits.push(format!("HTTP 代理指向死端口 {p}"));
            }
        }
        if let Some(p) = socks_port {
            if socks_on && own_ports.contains(&p) {
                hits.push(format!("SOCKS 代理指向死端口 {p}"));
            }
        }
    }
    (
        StaleProxyState {
            detected: !hits.is_empty(),
            detail: hits.join("；"),
        },
        pac,
    )
}

/// 复用 system_proxy::parse_port 同款逻辑（scutil 的 "HTTPPort : 7891" 带前导空格）。
fn parse_scutil_port(text: &str, key: &str) -> Option<u16> {
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix(key) {
            let v = rest.trim().trim_start_matches(':').trim();
            return v.parse().ok();
        }
    }
    None
}

/// 按字符（非字节）截断，避免中文文件名/值在多字节边界 panic。
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max).collect();
    format!("{head}…")
}

/// 环境变量代理提示：只报告，值已截断。
fn collect_env_proxy() -> EnvProxyHints {
    let names = ["http_proxy", "https_proxy", "all_proxy", "ftp_proxy", "no_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"];
    let mut vars = Vec::new();
    for n in names {
        if let Ok(v) = std::env::var(n) {
            let s = format!("{n}={v}");
            vars.push(truncate_chars(&s, 80));
        }
    }
    // shell profile 里的 export 提示（~/.zshrc / ~/.zprofile / ~/.bash_profile）
    let mut hits = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    for f in [".zshrc", ".zprofile", ".bash_profile", ".profile"] {
        let path = std::path::Path::new(&home).join(f);
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        for (i, line) in content.lines().enumerate() {
            let l = line.trim();
            let lower = l.to_lowercase();
            if (lower.starts_with("export") || lower.starts_with("setenv"))
                && (lower.contains("_proxy=") || lower.contains("_proxy ") || lower.contains("proxy_url"))
            {
                hits.push(format!("{f}:{}: {}", i + 1, truncate_chars(l, 80)));
            }
        }
    }
    EnvProxyHints { vars, shell_profile_hits: hits }
}

/// 体检主流程：串行采集（调用方负责 spawn_blocking）。单项失败 → degraded + unknown。
pub fn audit(own_ports: &[u16], our_pids: &[u32]) -> NetworkAuditReport {
    let mut degraded: Vec<String> = Vec::new();

    // 1) 第三方代理进程：与执行侧同源（lib.rs::find_foreign_proxies），杜绝两套口径
    let foreign_procs: Vec<String> = crate::find_foreign_proxies()
        .into_iter()
        .map(|(pid, label)| format!("{label} (PID {pid})"))
        .collect();

    // 2) LISTEN 扫描（2.5s 硬超时，与 apps.rs 同款约束；失败降级 unknown 不影响其余）
    let listen_sockets = match run_capped(LSOF, &["-nP", "-iTCP", "-sTCP:LISTEN"], 2500) {
        Ok(t) => parse_listen_sockets(&t),
        Err(_) => {
            degraded.push("listen_sockets".into());
            Vec::new()
        }
    };

    // 3) 端口冲突：本程序端口 LISTEN 者是否我们自己的内核
    let port_conflicts: Vec<PortConflict> = own_ports
        .iter()
        .filter_map(|p| {
            listen_sockets
                .iter()
                .find(|s| s.port == *p && !our_pids.contains(&s.pid))
                .map(|s| PortConflict {
                    port: *p,
                    label: format!("本程序端口 {p}"),
                    holder_command: s.command.clone(),
                    holder_pid: s.pid,
                    ours: false,
                })
        })
        .collect();

    // 内核是否在跑：按内核进程判定（与 MCP 侧 pgrep 同口径）。
    // 绝不能用"own 端口有 LISTEN"判定——第三方占用 7891 时内核根本没跑，
    // 那样会把真·死端口残留漏报（2026-09-23 回归测试抓出的口径错误）。
    let kernel_up = !our_pids.is_empty();

    // 4) 默认路由
    let route = match run_capped(ROUTE, &["-n", "get", "default"], 2000) {
        Ok(t) => {
            let (gateway, interface) = parse_default_route(&t);
            if gateway.is_empty() && interface.is_empty() {
                degraded.push("route".into());
                RouteState { state: Triage::Unknown, gateway: String::new(), interface: String::new(), tun_interface: false }
            } else {
                RouteState {
                    state: Triage::Yes,
                    tun_interface: interface.starts_with("utun"),
                    gateway,
                    interface,
                }
            }
        }
        Err(_) => {
            degraded.push("route".into());
            RouteState { state: Triage::Unknown, gateway: String::new(), interface: String::new(), tun_interface: false }
        }
    };

    // 5) TUN 接口清单（无默认路由≠无 TUN；接管审计要看得见）
    let tun_interfaces = match run_capped(IFCONFIG, &["-l"], 1000) {
        Ok(t) => parse_utun_list(&t),
        Err(_) => {
            degraded.push("tun_interfaces".into());
            Vec::new()
        }
    };

    // 6) DNS（resolver #1）
    let dns = match run_capped(SCUTIL, &["--dns"], 2000) {
        Ok(t) => {
            let ns = parse_dns_nameservers(&t);
            if ns.is_empty() {
                degraded.push("dns".into());
                DnsState { state: Triage::Unknown, nameservers: ns, note: "未解析到 resolver #1".into() }
            } else {
                DnsState { state: Triage::Yes, nameservers: ns.clone(), note: classify_dns(&ns) }
            }
        }
        Err(_) => {
            degraded.push("dns".into());
            DnsState { state: Triage::Unknown, nameservers: Vec::new(), note: String::new() }
        }
    };

    // 7) 系统代理全局视图（stale 检测 + PAC）
    let (stale_proxy, pac_enabled) = match run_capped(SCUTIL, &["--proxy"], 2000) {
        Ok(t) => parse_proxy_global(&t, own_ports, kernel_up),
        Err(_) => {
            degraded.push("proxy_global".into());
            (StaleProxyState { detected: false, detail: "scutil 采集失败".into() }, Triage::Unknown)
        }
    };

    // 8) 环境变量代理
    let env_proxy = collect_env_proxy();

    // 9) P1-2 账本视角：未结账本如实入报告（读账本失败按降级处理，不炸整报）
    let open_ledger = match crate::ledger::LedgerFile::default().open_session() {
        Some(session) => serde_json::to_value(serde_json::json!({
            "id": session.id,
            "reason": session.reason,
            "startedTs": session.started_ts,
            "entries": session.entries.len(),
        }))
        .ok(),
        None => None,
    };

    let summary = build_summary(&foreign_procs, &port_conflicts, &stale_proxy, &route, &pac_enabled, open_ledger.is_some(), &degraded);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    // 本程序端口绑在非回环 = 局域网可达（P0-2 钉回环的现值复核，与 MCP 同口径）
    let own_port_lan_exposed: Vec<ExposedPort> = listen_sockets
        .iter()
        .filter(|s| own_ports.contains(&s.port) && s.lan_exposed)
        .map(|s| ExposedPort {
            command: s.command.clone(),
            pid: s.pid,
            port: s.port,
        })
        .collect();

    NetworkAuditReport {
        ts,
        foreign_procs,
        port_conflicts,
        listen_sockets,
        stale_proxy,
        open_ledger,
        own_port_lan_exposed,
        route,
        tun_interfaces,
        dns,
        pac_enabled,
        env_proxy,
        summary,
        degraded,
    }
}

/// DNS 启发式归类（只陈述事实，不做"被污染/正常"的定论——那是分析层的事）。
fn classify_dns(ns: &[String]) -> String {
    let known: &[(&str, &str)] = &[
        ("223.5.5.5", "阿里公共 DNS"),
        ("223.6.6.6", "阿里公共 DNS"),
        ("119.29.29.29", "腾讯 DNSPod"),
        ("182.254.116.116", "腾讯 DNSPod"),
        ("1.1.1.1", "Cloudflare"),
        ("8.8.8.8", "Google"),
        ("114.114.114.114", "114 DNS"),
    ];
    let tags: Vec<String> = ns
        .iter()
        .map(|ip| {
            known
                .iter()
                .find(|(k, _)| k == ip)
                .map(|(_, label)| format!("{ip}（{label}）"))
                .unwrap_or_else(|| ip.clone())
        })
        .collect();
    tags.join("、")
}

fn build_summary(
    foreign: &[String],
    conflicts: &[PortConflict],
    stale: &StaleProxyState,
    route: &RouteState,
    pac: &Triage,
    open_ledger: bool,
    degraded: &[String],
) -> String {
    let mut items: Vec<String> = Vec::new();
    if !foreign.is_empty() {
        items.push(format!("{} 个第三方代理进程", foreign.len()));
    }
    if !conflicts.is_empty() {
        items.push(format!("{} 个本程序端口被占", conflicts.len()));
    }
    if stale.detected {
        items.push("系统代理残留指向死端口".into());
    }
    if matches!(pac, Triage::Yes) {
        items.push("PAC 自动代理已启用".into());
    }
    if route.tun_interface {
        items.push(format!("默认路由走 {}", route.interface));
    }
    if open_ledger {
        items.push("存在未结接管账本".into());
    }
    let mut s = if items.is_empty() { "未发现混乱源".into() } else { items.join("、") };
    if !degraded.is_empty() {
        s.push_str(&format!("（{} 项采集降级）", degraded.len()));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机取证 fixture（2026-09-23，本机 lsof 实际输出行）。
    #[test]
    fn parse_listen_real_format() {
        let real = "COMMAND     PID        USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME\n\
rapportd    653 someuser    4u  IPv4 0x93d4a09d3cbe7efe      0t0  TCP *:49187 (LISTEN)\n\
python3.1  2587 someuser    9u  IPv4 0x28058e917d656561      0t0  TCP 127.0.0.1:8000 (LISTEN)\n\
magic-ag  44755        root    8u  IPv6 0xb1f20d29513b6d18      0t0  TCP [::1]:7891 (LISTEN)\n";
        let s = parse_listen_sockets(real);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].port, 49187);
        assert!(s[0].lan_exposed, "* 绑定应标记局域网可达");
        assert_eq!(s[1].port, 8000);
        assert!(!s[1].lan_exposed, "127.0.0.1 不应标记");
        assert_eq!(s[2].port, 7891);
        assert!(!s[2].lan_exposed, "[::1] 不应标记");
    }

    /// 坏行只跳过不炸整批。
    #[test]
    fn parse_listen_skips_junk_lines() {
        let junk = "HDR HDR HDR HDR HDR HDR HDR HDR HDR\n\
broken no cols\n\
x 1 u f T D S TCP notaport (LISTEN)\n\
good 42 u f T D S TCP 127.0.0.1:9999 (LISTEN)\n";
        let s = parse_listen_sockets(junk);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].port, 9999);
    }

    /// route 真机 fixture（gateway/interface 行带前导空格）。
    #[test]
    fn parse_route_real_format() {
        let real = "   route to: default\ndestination: default\n       mask: default\n    gateway: 172.18.100.1\n  interface: en0\n      flags: <UP,GATEWAY,DONE,STATIC,PRCLONING,GLOBAL>\n";
        let (g, i) = parse_default_route(real);
        assert_eq!(g, "172.18.100.1");
        assert_eq!(i, "en0");
    }

    #[test]
    fn parse_utun_real_format() {
        let real = "lo0 gif0 stf0 anpi1 anpi2 anpi0 en3 en4 en5 en1 en6 en2 bridge0 ap1 utun0 en0 utun1 utun2 awdl0 llw0 utun3\n";
        assert_eq!(parse_utun_list(real), vec!["utun0", "utun1", "utun2", "utun3"]);
    }

    /// scutil --dns 真机 fixture：只取 resolver #1，后续分流 resolver 不混入。
    #[test]
    fn parse_dns_first_resolver_only() {
        let real = "DNS configuration\n\nresolver #1\n  nameserver[0] : 223.5.5.5\n  flags    : Request A records\n  reach    : 0x00000002 (Reachable)\n\nresolver #2\n  domain   : local\n  nameserver[0] : 10.0.0.2\n";
        assert_eq!(parse_dns_nameservers(real), vec!["223.5.5.5"]);
    }

    /// 真机 scutil --proxy 全关态：无 stale、PAC=No。
    #[test]
    fn parse_proxy_global_all_off() {
        let real = "<dictionary> {\n  FTPPassive : 1\n  HTTPEnable : 0\n  HTTPSEnable : 0\n  ProxyAutoConfigEnable : 0\n  SOCKSEnable : 0\n}";
        let (stale, pac) = parse_proxy_global(real, &[7891, 7892, 7893], false);
        assert!(!stale.detected);
        assert_eq!(pac, Triage::No);
    }

    /// 崩溃残留态：内核不在跑但 HTTPEnable=1 指向 7891 → 必须点名（P0-4 同源场景）
    #[test]
    fn parse_proxy_global_stale_dead_port() {
        let real = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 7891\n  ProxyAutoConfigEnable : 0\n  SOCKSEnable : 0\n}";
        let (stale, _) = parse_proxy_global(real, &[7891, 7892, 7893], false);
        assert!(stale.detected);
        assert!(stale.detail.contains("7891"));
    }

    /// 内核在跑时（kernel_up），同状态不算 stale——死端口判定以"无监听"为前提
    #[test]
    fn parse_proxy_global_alive_kernel_not_stale() {
        let real = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 7891\n  ProxyAutoConfigEnable : 0\n  SOCKSEnable : 0\n}";
        let (stale, _) = parse_proxy_global(real, &[7891, 7892, 7893], true);
        assert!(!stale.detected);
    }

    /// 第三方端口指向（7890 = 别的软件）绝不误报 stale——与 startup_self_check 同一铁律
    #[test]
    fn parse_proxy_global_third_party_port_ignored() {
        let real = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 7890\n  ProxyAutoConfigEnable : 0\n  SOCKSEnable : 0\n}";
        let (stale, _) = parse_proxy_global(real, &[7891, 7892, 7893], false);
        assert!(!stale.detected);
    }

    #[test]
    fn parse_proxy_global_pac_yes() {
        let real = "<dictionary> {\n  ProxyAutoConfigEnable : 1\n  ProxyAutoConfigURLString : http://x/pac\n}";
        let (_, pac) = parse_proxy_global(real, &[7891], false);
        assert_eq!(pac, Triage::Yes);
    }

    /// 前导空格端口解析（与 system_proxy parse_port 同款陷阱回归）
    #[test]
    fn scutil_port_leading_space() {
        assert_eq!(parse_scutil_port("  HTTPPort : 7891", "HTTPPort"), Some(7891));
        assert_eq!(parse_scutil_port("  HTTPPort : abc", "HTTPPort"), None);
    }

    #[test]
    fn classify_dns_known_and_unknown() {
        assert!(classify_dns(&["223.5.5.5".into()]).contains("阿里"));
        assert_eq!(classify_dns(&["192.168.1.1".into()]), "192.168.1.1");
    }

    #[test]
    fn summary_counts_items() {
        let s = build_summary(
            &["FlClash".into()],
            &[],
            &StaleProxyState { detected: true, detail: "x".into() },
            &RouteState { state: Triage::Yes, gateway: "g".into(), interface: "utun5".into(), tun_interface: true },
            &Triage::No,
            true,
            &[],
        );
        assert!(s.contains("1 个第三方代理进程"));
        assert!(s.contains("死端口"));
        assert!(s.contains("utun5"));
        assert!(s.contains("未结接管账本"));
    }

    /// run_capped：超时路径必须返回 Err 而非挂死（sleep 10s，上限 200ms）
    #[test]
    fn run_capped_timeout() {
        let t0 = std::time::Instant::now();
        let r = run_capped("/bin/sleep", &["10"], 200);
        assert!(r.is_err());
        assert!(t0.elapsed() < std::time::Duration::from_secs(2), "超时后必须快速返回，实际 {:?}", t0.elapsed());
    }

    #[test]
    fn run_capped_success_and_fail() {
        assert!(run_capped("/usr/bin/true", &[], 1000).is_ok());
        assert!(run_capped("/usr/bin/false", &[], 1000).is_err());
        assert!(run_capped("/nonexistent/bin", &[], 1000).is_err());
    }

    /// 前端契约锁：Dashboard 体检卡直接消费 audit.ownPortLanExposed /
    /// audit.staleProxy.detected / audit.pacEnabled 等键——v0.3.0 曾因 Rust 报告
    /// 缺 ownPortLanExposed 且前端无防护，点「开始体检」渲染崩溃整页黑屏。
    /// 报告 JSON 键集合必须与前端引用逐一对应，改字段名 = 崩 GUI，必须有测试挡。
    /// （前端侧同时已加可选链防御，双保险；本锁保"字段存在"这半边。）
    #[test]
    fn report_json_keys_match_frontend_contract() {
        let r = NetworkAuditReport {
            ts: "0".into(),
            foreign_procs: vec![],
            port_conflicts: vec![],
            listen_sockets: vec![],
            stale_proxy: StaleProxyState { detected: false, detail: String::new() },
            open_ledger: None,
            own_port_lan_exposed: vec![ExposedPort { command: "mihomo".into(), pid: 1, port: 7891 }],
            route: RouteState { state: Triage::Yes, gateway: String::new(), interface: "en0".into(), tun_interface: false },
            tun_interfaces: vec![],
            dns: DnsState { state: Triage::Yes, nameservers: vec![], note: String::new() },
            pac_enabled: Triage::No,
            env_proxy: EnvProxyHints { vars: vec![], shell_profile_hits: vec![] },
            summary: String::new(),
            degraded: vec![],
        };
        let v = serde_json::to_value(&r).unwrap();
        for key in [
            "foreignProcs", "portConflicts", "listenSockets", "staleProxy",
            "openLedger", "ownPortLanExposed", "route", "tunInterfaces",
            "dns", "pacEnabled", "envProxy", "summary", "degraded",
        ] {
            assert!(v.get(key).is_some(), "体检报告缺前端契约键 {key}（GUI 会白屏）");
        }
        // 嵌套消费点：前端直接读 staleProxy.detected / ownPortLanExposed[].port
        assert!(v["staleProxy"].get("detected").is_some());
        assert_eq!(v["ownPortLanExposed"][0]["port"], serde_json::json!(7891));
    }
}
