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

/// SSH 服务器连接信息（不存明文密码/私钥，只存标记）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: String, // "password" | "key"
    pub password_saved: bool,
    pub private_key_saved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub nodes: Vec<ProxyNode>,
    pub selected_node: Option<String>,
    pub apps: Vec<AppSetting>,
    pub system_proxy: bool,
    pub auto_global: String,
    pub subscription_url: Option<String>,
    pub servers: Vec<ServerInfo>,
    // 兼容旧配置：仍保留这几个字段，但新逻辑不再把明文写进 config.json
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
                    name: "搬瓦工直连".to_string(),
                    server: "104.160.40.35".to_string(),
                    port: 443,
                    uuid: "268a1166-d31e-478c-a66f-7f9c06c9afaa".to_string(),
                    flow: "xtls-rprx-vision".to_string(),
                    network: "tcp".to_string(),
                    tls: true,
                    udp: true,
                    fingerprint: "chrome".to_string(),
                    public_key: "c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0".to_string(),
                    short_id: "be08e6123ddcaf32".to_string(),
                    sni: String::new(),
                },
                ProxyNode {
                    name: "Texas住宅".to_string(),
                    server: "104.160.40.35".to_string(),
                    port: 443,
                    uuid: "7f3e9a2b-4c5d-6e8f-1a2b-3c4d5e6f7a8b".to_string(),
                    flow: "xtls-rprx-vision".to_string(),
                    network: "tcp".to_string(),
                    tls: true,
                    udp: true,
                    fingerprint: "chrome".to_string(),
                    public_key: "c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0".to_string(),
                    short_id: "bca4b7cfbcb66d57".to_string(),
                    sni: String::new(),
                },
            ],
            selected_node: Some("搬瓦工直连".to_string()),
            apps: vec![
                AppSetting { id: "app-Google Chrome".to_string(), mode: "proxy".to_string() },
                AppSetting { id: "app-Safari".to_string(), mode: "direct".to_string() },
                AppSetting { id: "app-WeChat".to_string(), mode: "direct".to_string() },
                AppSetting { id: "app-QQ".to_string(), mode: "direct".to_string() },
            ],
            system_proxy: true,
            auto_global: "auto".to_string(),
            subscription_url: None,
            servers: vec![],
            ssh_host: None,
            ssh_port: Some(22),
            ssh_user: Some("root".to_string()),
            ssh_auth: Some("password".to_string()),
            ssh_password: None,
            ssh_private_key: None,
        }
    }
}

impl AppConfig {
    /// 当前激活的 SSH 服务器（优先 servers 列表，其次旧字段）
    pub fn active_server(&self) -> Option<ServerInfo> {
        if let Some(s) = self.servers.first() {
            return Some(s.clone());
        }
        // 旧字段兼容
        let host = self.ssh_host.clone()?;
        Some(ServerInfo {
            id: format!("ssh-{}", host),
            name: host.clone(),
            host,
            port: self.ssh_port.unwrap_or(22),
            user: self.ssh_user.clone().unwrap_or_else(|| "root".to_string()),
            auth: self.ssh_auth.clone().unwrap_or_else(|| "password".to_string()),
            password_saved: self.ssh_password.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
            private_key_saved: self.ssh_private_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
        })
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
