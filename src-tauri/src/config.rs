use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyNode {
    pub name: String,
    pub server: String,
    pub port: u16,
    pub uuid: String,
    pub flow: String,
    pub network: String,
    pub tls: bool,
    pub udp: bool,
    pub fingerprint: String,
    pub public_key: String,
    pub short_id: String,
    pub sni: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSetting {
    pub id: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub nodes: Vec<ProxyNode>,
    pub apps: Vec<AppSetting>,
    pub system_proxy: bool,
    pub auto_global: String,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<u16>,
    pub ssh_user: Option<String>,
    pub ssh_auth: Option<String>,
    pub ssh_password: Option<String>,
    pub ssh_private_key: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            nodes: vec![
                ProxyNode {
                    name: "示例节点".to_string(),
                    server: "1.1.1.1".to_string(),
                    port: 443,
                    uuid: "00000000-0000-4000-8000-000000000000".to_string(),
                    flow: "xtls-rprx-vision".to_string(),
                    network: "tcp".to_string(),
                    tls: true,
                    udp: true,
                    fingerprint: "chrome".to_string(),
                    public_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                    short_id: "0000000000000000".to_string(),
                    sni: String::new(),
                },
                ProxyNode {
                    name: "示例节点2".to_string(),
                    server: "1.1.1.1".to_string(),
                    port: 443,
                    uuid: "00000000-0000-4000-8000-000000000000".to_string(),
                    flow: "xtls-rprx-vision".to_string(),
                    network: "tcp".to_string(),
                    tls: true,
                    udp: true,
                    fingerprint: "chrome".to_string(),
                    public_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
                    short_id: "0000000000000000".to_string(),
                    sni: String::new(),
                },
            ],
            apps: vec![],
            system_proxy: true,
            auto_global: "auto".to_string(),
            ssh_host: None,
            ssh_port: Some(22),
            ssh_user: Some("root".to_string()),
            ssh_auth: Some("password".to_string()),
            ssh_password: None,
            ssh_private_key: None,
        }
    }
}

pub fn config_path() -> PathBuf {
    let dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.join("magic-agent").join("config.json")
}

pub fn load() -> AppConfig {
    let p = config_path();
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let p = config_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&p, serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
