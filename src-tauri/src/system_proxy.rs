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
    for svc in &services {
        if enable {
            let _ = Command::new("networksetup").args(["-setwebproxy", svc, "127.0.0.1", &port.to_string()]).output();
            let _ = Command::new("networksetup").args(["-setsecurewebproxy", svc, "127.0.0.1", &port.to_string()]).output();
            let _ = Command::new("networksetup").args(["-setsocksfirewallproxy", svc, "127.0.0.1", &port.to_string()]).output();
            let _ = Command::new("networksetup").args(["-setwebproxystate", svc, "on"]).output();
            let _ = Command::new("networksetup").args(["-setsecurewebproxystate", svc, "on"]).output();
            let _ = Command::new("networksetup").args(["-setsocksfirewallproxystate", svc, "on"]).output();
        } else {
            let _ = Command::new("networksetup").args(["-setwebproxystate", svc, "off"]).output();
            let _ = Command::new("networksetup").args(["-setsecurewebproxystate", svc, "off"]).output();
            let _ = Command::new("networksetup").args(["-setsocksfirewallproxystate", svc, "off"]).output();
        }
    }
    Ok(SystemProxyStatus { enabled: enable, http_port: port, socks_port: port })
}

pub fn status() -> SystemProxyStatus {
    let output = Command::new("scutil").arg("--proxy").output().ok();
    let text = output.and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    let enabled = text.contains("HTTPEnable : 1") || text.contains("SOCKSEnable : 1");
    SystemProxyStatus { enabled, http_port: 7890, socks_port: 7890 }
}

fn list_services() -> Vec<String> {
    let output = Command::new("networksetup").arg("-listallnetworkservices").output().ok();
    let text = output.and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    text.lines().filter(|l| !l.is_empty() && !l.contains("asterisk") && !l.contains("An asterisk")).map(|s| s.trim().to_string()).collect()
}
