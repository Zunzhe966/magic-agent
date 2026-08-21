use serde::Serialize;
use std::process::Command;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyStatus {
    pub enabled: bool,
    pub http_port: u16,
    pub socks_port: u16,
}

pub fn set_system_proxy(enable: bool, port: u16) -> Result<SystemProxyStatus, String> {
    let services = list_services();
    let port_str = port.to_string();
    let mut any_success = false;
    let mut first_err = String::new();
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
            match Command::new("networksetup").args(&args).output() {
                Ok(out) if out.status.success() => any_success = true,
                Ok(out) => {
                    if first_err.is_empty() {
                        first_err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    }
                }
                Err(e) => {
                    if first_err.is_empty() {
                        first_err = e.to_string();
                    }
                }
            }
        }
    }
    if !any_success && !services.is_empty() {
        return Err(format!("设置系统代理失败（可能需要管理员权限或授权）：{}", first_err));
    }
    Ok(SystemProxyStatus { enabled: enable, http_port: port, socks_port: port })
}

pub fn status() -> SystemProxyStatus {
    let output = Command::new("scutil").arg("--proxy").output().ok();
    let text = output.and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    let enabled = text.contains("HTTPEnable : 1") || text.contains("SOCKSEnable : 1");
    let http_port = parse_port(&text, "HTTPPort");
    let socks_port = parse_port(&text, "SOCKSPort");
    SystemProxyStatus { enabled, http_port: http_port.unwrap_or(0), socks_port: socks_port.unwrap_or(0) }
}

fn list_services() -> Vec<String> {
    let output = Command::new("networksetup").arg("-listallnetworkservices").output().ok();
    let text = output.and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    text.lines()
        .map(|s| s.trim().to_string())
        .filter(|l| {
            if l.is_empty() { return false; }
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
            let v = rest.trim_start_matches(':').trim();
            return v.parse().ok();
        }
    }
    None
}
