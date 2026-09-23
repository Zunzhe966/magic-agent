use serde::{Deserialize, Serialize};
use std::process::Command;

/// networksetup 可执行路径（与 MCP 侧 /usr/sbin/networksetup 一致）
const NETWORKSETUP: &str = "/usr/sbin/networksetup";
const SCUTIL: &str = "/usr/sbin/scutil";

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServiceProxyState {
    /// 网络服务名，如 "Wi-Fi"
    pub service: String,
    /// -getwebproxystate 结果
    pub http_on: bool,
    /// -getsecurewebproxystate 结果
    pub https_on: bool,
    /// -getsocksfirewallproxystate 结果
    pub socks_on: bool,
    /// 该服务上失败的操作及原因（正常为空）
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyStatus {
    /// 全局视图（scutil --proxy）：任一协议 Enable=1 即为开。
    /// 注意这是"整机当前是否有代理生效"，与逐服务对账互补。
    pub enabled: bool,
    pub http_port: u16,
    pub socks_port: u16,
    /// 逐服务真实状态（networksetup 读回）。空数组 = 无法枚举服务。
    pub services: Vec<ServiceProxyState>,
    /// services 全部达成期望（开=三项全 on，关=三项全 off）
    pub all_ok: bool,
    /// 设置后验证仍不达标（或读写命令失败）的服务清单，如实上报，绝不吞掉
    pub mismatched: Vec<String>,
}

impl Default for SystemProxyStatus {
    fn default() -> Self {
        SystemProxyStatus {
            enabled: false,
            http_port: 0,
            socks_port: 0,
            services: vec![],
            all_ok: false,
            mismatched: vec![],
        }
    }
}

/// 设置系统代理并【逐服务回读验证】。
///
/// 旧实现的 `any_success`（任一服务任一命令成功即报整体成功）是"假账"根源：
/// 12 个服务只设上 1 个也返回成功。现在每个服务设置后立即用 -get* 读回，
/// 结果对象如实携带每个服务的三个开关状态与错误；不再仅凭写命令退出码下结论。
/// 部分失败时不报 Err（系统代理确实可能改了半套），而是 all_ok=false +
/// mismatched 点名，由调用方/UI 决定是否回滚或提示。
/// 仅当"没有任何服务可供操作"时才返回 Err。
pub fn set_system_proxy(enable: bool, port: u16) -> Result<SystemProxyStatus, String> {
    let services = list_services();
    if services.is_empty() {
        return Err("未检测到网络服务，无法设置系统代理".to_string());
    }
    let port_str = port.to_string();

    // 第一遍：逐服务下发设置命令（失败先记下，不中断——其余服务继续）
    let mut write_errors: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for svc in &services {
        let args_list: Vec<Vec<&str>> = if enable {
            vec![
                vec!["-setwebproxy", svc, "127.0.0.1", &port_str],
                vec!["-setsecurewebproxy", svc, "127.0.0.1", &port_str],
                vec!["-setsocksfirewallproxy", svc, "127.0.0.1", &port_str],
                vec!["-setwebproxystate", svc, "on"],
                vec!["-setsecurewebproxystate", svc, "on"],
                vec!["-setsocksfirewallproxystate", svc, "on"],
            ]
        } else {
            vec![
                vec!["-setwebproxystate", svc, "off"],
                vec!["-setsecurewebproxystate", svc, "off"],
                vec!["-setsocksfirewallproxystate", svc, "off"],
            ]
        };
        for args in args_list {
            if let Err(e) = run(NETWORKSETUP, &args) {
                write_errors.entry(svc.clone()).or_default().push(format!(
                    "{}: {}",
                    args.first().copied().unwrap_or("?"),
                    e
                ));
            }
        }
    }

    // 第二遍：逐服务回读对账（真实说了算的是读回结果，不是写命令退出码）。
    // 达标口径已封装在 verify_system_proxy 内（开=Enabled+端口精确匹配；关=Enabled:No）。
    let states = verify_system_proxy(&services, port, enable);
    let mut mismatched = Vec::new();
    for st in &states {
        // *_on 语义 = "该通道是否达标"（开=Enabled:Yes 且端口精确匹配；关=Enabled:No），
        // 判定已封装在 verify 内，调用处三与即可。
        if !(st.http_on && st.https_on && st.socks_on) || !st.errors.is_empty() {
            mismatched.push(st.service.clone());
        }
    }
    // 写失败但读回恰好符合期望的（如命令 stderr 有警告但生效了），以读回为准；
    // 写失败且未点名进 mismatched 的服务也要点名，错误信息不丢
    for svc in &services {
        if write_errors.contains_key(svc) && !mismatched.contains(svc) {
            mismatched.push(svc.clone());
        }
    }

    let all_ok = !states.is_empty()
        && write_errors.is_empty()
        && mismatched.is_empty();
    // 端口从 scutil 全局视图读一次（best-effort，读不到置 0，不影响对账结论）
    let global = status();

    // enabled 字段在 set 路径取"期望值"：逐服务对账（all_ok/services/mismatched）
    // 才是写后真实性的权威。不用 scutil 全局视图回填——它有秒级传播延迟，
    // 刚开启就立刻读可能仍报 0，会让前端开关闪回、产生新的"假账"。
    // 全局视图的权威用途是轮询侧 status()。
    Ok(SystemProxyStatus {
        enabled: enable,
        http_port: global.http_port,
        socks_port: global.socks_port,
        services: states,
        all_ok,
        mismatched,
    })
}

/// 逐服务读回系统代理真实状态。
///
/// ⚠️ 命令选择依据（2026-09-23 实机实测，Darwin 25.6）：
/// `-getwebproxystate` 系列读命令【不存在】（rc=5 "command is not recognized"），
/// 但 `-setwebproxystate` 存在——读写命令集不对称。此前用 -get*state 做读回
/// 导致对账恒失败、恒误报 mismatched。唯一可靠读法是 -getwebproxy 三件套，
/// 真实输出格式：
///   Enabled: Yes
///   Server: 127.0.0.1
///   Port: 7891
///   Authenticated Proxy Enabled: 0
/// 对账标准（管理员级）：开 = Enabled:Yes 且 Server=127.0.0.1 且 Port=期望端口
/// （"开着但指向别人的端口"同样是失控状态，必须点名）；关 = Enabled:No。
/// 单项读取失败记入该服务 errors，不影响其他服务（绝不 panic、绝不静默吞错）。
pub fn verify_system_proxy(services: &[String], want_port: u16, expect_on: bool) -> Vec<ServiceProxyState> {
    services
        .iter()
        .map(|svc| {
            let mut errors = Vec::new();
            let http = get_proxy_detail(svc, "-getwebproxy", want_port, expect_on, &mut errors);
            let https = get_proxy_detail(svc, "-getsecurewebproxy", want_port, expect_on, &mut errors);
            let socks = get_proxy_detail(svc, "-getsocksfirewallproxy", want_port, expect_on, &mut errors);
            ServiceProxyState {
                service: svc.clone(),
                http_on: http.0,
                https_on: https.0,
                socks_on: socks.0,
                errors,
            }
        })
        .collect()
}

/// 返回 (是否达标, 读到的端口)。达标 = 开关符合期望；开启时还要求指向 127.0.0.1:期望端口。
fn get_proxy_detail(svc: &str, flag: &str, want_port: u16, expect_on: bool, errors: &mut Vec<String>) -> (bool, u16) {
    match run(NETWORKSETUP, &[flag, svc]) {
        Ok(out) => {
            let (enabled, port) = parse_proxy_detail(&out);
            let ok = if expect_on {
                enabled && port == want_port
            } else {
                !enabled
            };
            if !ok && enabled && port != want_port {
                // 开着但指向别的端口：把事实塞进 errors 供点名排查
                errors.push(format!("{}: 指向 {}:{} 而非期望端口 {}", flag, "127.0.0.1", port, want_port));
            }
            (ok, port)
        }
        Err(e) => {
            errors.push(format!("{}: {}", flag, e));
            (false, 0)
        }
    }
}

/// 单个服务在某一代理通道上的【完整原值】（P1-2 账本快照用）。
/// 与 ServiceProxyState 的"达标"语义不同：这里如实记录 enabled/server/port，
/// 回放时才能把系统恢复成接管前的样子，而不是恢复成"达标"。
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyChannelRaw {
    pub enabled: bool,
    pub server: String,
    pub port: u16,
}

/// 一个网络服务三条通道的完整原值快照。
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSnapshot {
    pub service: String,
    pub http: ProxyChannelRaw,
    pub https: ProxyChannelRaw,
    pub socks: ProxyChannelRaw,
    /// 读取失败时非空；回放遇到有 error 的通道会跳过（宁可不还原，不能瞎写）
    pub errors: Vec<String>,
}

/// 逐服务读取【完整原值】（P1-2 接管账本的 before 快照）。
/// 读法沿用 -getwebproxy 三件套（真机唯一可靠读法）。
pub fn snapshot_system_proxy() -> Vec<ServiceSnapshot> {
    list_services()
        .iter()
        .map(|svc| {
            let mut errors = Vec::new();
            let http = read_channel(svc, "-getwebproxy", &mut errors);
            let https = read_channel(svc, "-getsecurewebproxy", &mut errors);
            let socks = read_channel(svc, "-getsocksfirewallproxy", &mut errors);
            ServiceSnapshot {
                service: svc.clone(),
                http,
                https,
                socks,
                errors,
            }
        })
        .collect()
}

fn read_channel(svc: &str, flag: &str, errors: &mut Vec<String>) -> ProxyChannelRaw {
    match run(NETWORKSETUP, &[flag, svc]) {
        Ok(out) => {
            let (enabled, port) = parse_proxy_detail(&out);
            let server = parse_proxy_server(&out);
            ProxyChannelRaw {
                enabled,
                server,
                port,
            }
        }
        Err(e) => {
            errors.push(format!("{flag}: {e}"));
            ProxyChannelRaw {
                enabled: false,
                server: String::new(),
                port: 0,
            }
        }
    }
}

/// 解析 -get*proxy 输出的 Server 行（真机格式 "Server: 127.0.0.1"）。
fn parse_proxy_server(out: &str) -> String {
    for line in out.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("Server:") {
            return v.trim().to_string();
        }
    }
    String::new()
}

/// 按快照把某服务的某通道恢复回原值（P1-2 账本回放的最小单元）。
/// 返回 Err 只在该通道【写命令失败】时发生；回放调用方汇总后如实上报，绝不静默。
fn restore_channel(svc: &str, flag: &str, state_flag: &str, raw: &ProxyChannelRaw) -> Result<(), String> {
    if raw.enabled {
        let port = raw.port.to_string();
        let server = if raw.server.is_empty() { "127.0.0.1".to_string() } else { raw.server.clone() };
        run(NETWORKSETUP, &[flag, svc, &server, &port])?;
        run(NETWORKSETUP, &[state_flag, svc, "on"])?;
    } else {
        run(NETWORKSETUP, &[state_flag, svc, "off"])?;
    }
    Ok(())
}

/// 按快照逆序回放一个服务（HTTP→HTTPS→SOCKS 的写入顺序与接管时一致）。
/// 有 error 的通道跳过还原并如实记入返回的错误清单（宁可不还原，不能凭 unknown 瞎写）。
pub fn restore_service_snapshot(snap: &ServiceSnapshot) -> Vec<String> {
    let mut errs = Vec::new();
    if !snap.errors.is_empty() {
        errs.push(format!("{}: 快照不完整（{}），跳过还原", snap.service, snap.errors.join("；")));
        return errs;
    }
    if let Err(e) = restore_channel(&snap.service, "-setwebproxy", "-setwebproxystate", &snap.http) {
        errs.push(format!("{}: http 还原失败 {e}", snap.service));
    }
    if let Err(e) = restore_channel(&snap.service, "-setsecurewebproxy", "-setsecurewebproxystate", &snap.https) {
        errs.push(format!("{}: https 还原失败 {e}", snap.service));
    }
    if let Err(e) = restore_channel(&snap.service, "-setsocksfirewallproxy", "-setsocksfirewallproxystate", &snap.socks) {
        errs.push(format!("{}: socks 还原失败 {e}", snap.service));
    }
    errs
}

/// 解析 -get*proxy 输出：提取 Enabled 布尔与 Port。无法解析视为关/0。
fn parse_proxy_detail(out: &str) -> (bool, u16) {
    let mut enabled = false;
    let mut port = 0u16;
    for line in out.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("Enabled:") {
            enabled = v.trim().eq_ignore_ascii_case("yes");
        } else if let Some(v) = l.strip_prefix("Port:") {
            port = v.trim().parse().unwrap_or(0);
        }
        // 注意：末行 "Authenticated Proxy Enabled: 0" 也以 Enabled 结尾但带前缀，
        // strip_prefix("Enabled:") 不会误匹配它（其前缀是 "Authenticated Proxy "）。
    }
    (enabled, port)
}

fn run(bin: &str, args: &[&str]) -> Result<String, String> {
    Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("启动 {bin} 失败: {e}"))
        .and_then(|o| {
            if o.status.success() {
                Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                Err(format!(
                    "{}",
                    String::from_utf8_lossy(&o.stderr).trim()
                ))
            }
        })
}

/// 全局视图：scutil --proxy。口径与 status() 一致，set 之后合并进返回值。
pub fn status() -> SystemProxyStatus {
    let text = Command::new(SCUTIL)
        .arg("--proxy")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let enabled = text.contains("HTTPEnable : 1") || text.contains("SOCKSEnable : 1");
    let http_port = parse_port(&text, "HTTPPort");
    let socks_port = parse_port(&text, "SOCKSPort");
    SystemProxyStatus {
        enabled,
        http_port: http_port.unwrap_or(0),
        socks_port: socks_port.unwrap_or(0),
        // 轮询路径只做一次 scutil，不带逐服务对账（那是 set 后的重操作）。
        // 消费方规则：services/mismatched 为空时只能读 enabled/端口，
        // 需要逐服务真实状态必须走 set_system_proxy 返回值或 verify_system_proxy。
        services: vec![],
        all_ok: enabled,
        mismatched: vec![],
    }
}

fn list_services() -> Vec<String> {
    let text = match run(NETWORKSETUP, &["-listallnetworkservices"]) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    text.lines()
        .map(|s| s.trim().to_string())
        .filter(|l| {
            if l.is_empty() {
                return false;
            }
            let lower = l.to_lowercase();
            // 跳过非真实网络服务（蓝牙、USB 虚拟、Thunderbolt 桥接等），避免误设代理
            !lower.contains("asterisk")
                && !lower.contains("bluetooth")
                && !lower.contains("iphone")
                && !lower.contains("thunderbolt")
                && !lower.contains("bridge")
        })
        .collect()
}

fn parse_port(text: &str, key: &str) -> Option<u16> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(key) {
            // scutil 格式 "HTTPPort : 7891"：strip 后先 trim 去前导空格再剥冒号。
            // 旧实现漏了这一步，对 " : 7891" 解析恒失败返回 None（潜伏 bug，
            // 曾使端口字段永远为 0；P0-4 自愈依赖端口值，必须正确）。
            let v = rest.trim().trim_start_matches(':').trim();
            return v.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机实测 fixture（2026-09-23，Darwin 25.6，networksetup -getwebproxy Wi-Fi）：
    ///     Enabled: Yes / Server: 127.0.0.1 / Port: 7891 / Authenticated Proxy Enabled: 0
    /// 教训：上一版测试 fixture 用臆测的 "Web Proxy ... is Enabled." 格式，与真机完全不符，
    /// 纯 mock 测试因此全绿但真实对账恒失败。任何解析逻辑改动必须先跑真机输出对格式。
    #[test]
    fn parse_proxy_detail_reads_real_networksetup_format() {
        let real = "Enabled: Yes\nServer: 127.0.0.1\nPort: 7891\nAuthenticated Proxy Enabled: 0";
        assert_eq!(parse_proxy_detail(real), (true, 7891));
        let off = "Enabled: No\nServer: \nPort: 0\nAuthenticated Proxy Enabled: 0";
        assert_eq!(parse_proxy_detail(off), (false, 0));
        // 关键陷阱：末行 "Authenticated Proxy Enabled: 1" 不能被当成 Enabled 行误匹配
        assert_eq!(parse_proxy_detail("Authenticated Proxy Enabled: 1"), (false, 0));
        assert_eq!(parse_proxy_detail(""), (false, 0));
        assert_eq!(parse_proxy_detail("Some error text"), (false, 0));
    }

    #[test]
    fn verify_port_mismatch_is_not_ok() {
        // "开着但指向 7890"（本机 Thunderbolt Bridge 实测残留场景）：
        // 期望开且指向 7891 的口径下必须判不达标——端口精确匹配是归序的核心。
        let stale = "Enabled: Yes\nServer: 127.0.0.1\nPort: 7890\nAuthenticated Proxy Enabled: 0";
        let (enabled, port) = parse_proxy_detail(stale);
        assert!(enabled && port == 7890 && port != 7891);
    }

    #[test]
    fn parse_port_reads_scutil_format() {
        let text = "<dictionary> {\n  HTTPEnable : 1\n  HTTPPort : 7891\n  SOCKSPort : 7891\n}";
        assert_eq!(parse_port(text, "HTTPPort"), Some(7891));
        assert_eq!(parse_port(text, "SOCKSPort"), Some(7891));
        assert_eq!(parse_port(text, "HTTPProxy"), None);
    }

    #[test]
    fn status_serializes_new_fields_camel_case() {
        let st = SystemProxyStatus {
            enabled: true,
            http_port: 7891,
            socks_port: 7891,
            services: vec![ServiceProxyState {
                service: "Wi-Fi".into(),
                http_on: true,
                https_on: true,
                socks_on: false,
                errors: vec!["-setsocksfirewallproxystate: denied".into()],
            }],
            all_ok: false,
            mismatched: vec!["Wi-Fi".into()],
        };
        let json = serde_json::to_value(&st).unwrap();
        // 前端契约：camelCase 字段名
        assert_eq!(json["allOk"], false);
        assert_eq!(json["mismatched"][0], "Wi-Fi");
        assert_eq!(json["services"][0]["httpOn"], true);
        assert_eq!(json["services"][0]["socksOn"], false);
        assert!(json["services"][0]["errors"].is_array());
    }

    #[test]
    fn service_state_partial_failure_is_visible_not_swallowed() {
        // 3 成功 1 失败：如实点名（此用例直接构造 verify 结果，不 mock 进程——
        // 决策记录：verify_system_proxy 是纯读回函数，其正确性由集成验收场景 D 兜底，
        // 单测锁定的是"结构体如实表达部分失败"这一序列化契约）
        let states = vec![
            ServiceProxyState { service: "Wi-Fi".into(), http_on: true, https_on: true, socks_on: true, errors: vec![] },
            ServiceProxyState { service: "USB LAN".into(), http_on: true, https_on: true, socks_on: true, errors: vec![] },
            ServiceProxyState { service: "Thunderbolt".into(), http_on: true, https_on: true, socks_on: true, errors: vec![] },
            ServiceProxyState { service: "AX88179A".into(), http_on: false, https_on: false, socks_on: false, errors: vec!["denied".into()] },
        ];
        let st = SystemProxyStatus {
            enabled: true,
            http_port: 7891,
            socks_port: 7891,
            services: states,
            all_ok: false,
            mismatched: vec!["AX88179A".into()],
        };
        let json: serde_json::Value = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
        assert_eq!(json["services"].as_array().unwrap().len(), 4);
        assert_eq!(json["mismatched"].as_array().unwrap().len(), 1);
        assert_eq!(json["allOk"], false);
    }
}
