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
    /// 节点来源："manual" 手动添加 | "subscription" 订阅拉取
    #[serde(default = "default_source")]
    pub source: String,
    /// 地区标签（如 美国/日本/香港），空则展示时从名称猜测
    #[serde(default)]
    pub region: String,
}

fn default_source() -> String {
    "manual".to_string()
}

/// 软件分流设置：两态模型，没有"智能"。
/// 每个软件由用户与 AI 探讨后确认：直连，或走代理（可指定到具体节点）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSetting {
    pub id: String,
    /// "direct" 直连 | "proxy" 走代理
    pub mode: String,
    /// mode=proxy 时指定的节点名；None 表示用"当前选中节点"
    #[serde(default)]
    pub node: Option<String>,
    /// 为什么这么定（AI 探讨时给出的理由，留在表里可查）
    #[serde(default)]
    pub reason: String,
    /// 用户确认过才生效；未确认的一律按直连处理
    #[serde(default)]
    pub confirmed: bool,
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
    #[serde(default)]
    pub key_path: Option<String>,
}

/// 域名分流规则：按域名后缀/关键字指定走代理或直连。
/// 优先级在"软件规则"之后、"兜底直连"之前，解决同一个软件下载混合源的问题。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainRule {
    pub domain: String,   // 如 github.com、huggingface.co
    pub target: String,   // "proxy" 走代理 | "direct" 直连
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub nodes: Vec<ProxyNode>,
    pub selected_node: Option<String>,
    pub apps: Vec<AppSetting>,
    pub system_proxy: bool,
    pub auto_global: String,
    #[serde(default)]
    pub subscription_url: Option<String>,
    #[serde(default)]
    pub servers: Vec<ServerInfo>,
    #[serde(default)]
    pub active_server_id: Option<String>,
    #[serde(default)]
    pub domain_rules: Vec<DomainRule>,
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
            // 节点默认空：不硬编码任何真实节点信息（隐私），由用户手动添加或订阅拉取
            nodes: vec![],
            selected_node: None,
            apps: vec![],
            system_proxy: true,
            auto_global: "auto".to_string(),
            subscription_url: None,
            servers: vec![],
            active_server_id: None,
            domain_rules: vec![],
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
    /// 当前激活的 SSH 服务器（优先 active_server_id，其次 servers 首项和旧字段）
    pub fn active_server(&self) -> Option<ServerInfo> {
        if let Some(id) = &self.active_server_id {
            if let Some(s) = self.servers.iter().find(|s| &s.id == id) {
                return Some(s.clone());
            }
        }
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
            key_path: self.ssh_private_key.clone(),
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


/// 从订阅文本中解析 VLESS 节点。
/// 订阅内容可能是：明文 vless:// 链接、每行一个，或 base64 编码的整段内容。
pub fn parse_vless_subscription(text: &str) -> Result<Vec<ProxyNode>, String> {
    // 若文本不含 vless://，尝试 base64 解码（macOS 自带 base64 -D）
    let mut content = text.to_string();
    if !content.contains("vless://") {
        let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        if !cleaned.is_empty() {
            let out = std::process::Command::new("/usr/bin/base64")
                .arg("-D")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut c| {
                    use std::io::Write;
                    if let Some(mut stdin) = c.stdin.take() {
                        let _ = stdin.write_all(cleaned.as_bytes());
                    }
                    c.wait_with_output()
                });
            if let Ok(out) = out {
                if out.status.success() {
                    if let Ok(decoded) = String::from_utf8(out.stdout) {
                        content = decoded;
                    }
                }
            }
        }
    }

    let mut nodes = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        let Some(idx) = line.find("vless://") else { continue };
        let uri = &line[idx..];
        if let Ok(node) = parse_vless_uri(uri) {
            nodes.push(node);
        }
    }
    if nodes.is_empty() {
        return Err("未能从订阅中解析出任何 VLESS 节点".to_string());
    }
    Ok(nodes)
}

/// 解析单个 vless:// URI 为 ProxyNode。
fn parse_vless_uri(uri: &str) -> Result<ProxyNode, String> {
    let rest = uri.strip_prefix("vless://").ok_or("不是 vless 链接")?;
    // 分离 query 和 fragment
    let (auth_part, after) = match rest.find('?') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };
    let (query, fragment) = match after.find('#') {
        Some(i) => (&after[..i], &after[i + 1..]),
        None => (after, ""),
    };

    // auth_part: uuid@host:port
    let (userinfo, hostport) = auth_part.rsplit_once('@').ok_or("缺少 @")?;
    let (host, port_str) = hostport.rsplit_once(':').ok_or("缺少端口")?;
    let port: u16 = port_str.parse().map_err(|_| "端口无效")?;

    // 解析 query 参数
    let mut params = std::collections::HashMap::new();
    for kv in query.split('&') {
        if let Some((k, v)) = kv.split_once('=') {
            params.insert(k, url_decode(v));
        }
    }

    let name = if fragment.is_empty() {
        host.to_string()
    } else {
        url_decode(fragment)
    };

    Ok(ProxyNode {
        region: guess_region(&name),
        name,
        server: host.to_string(),
        port,
        uuid: userinfo.to_string(),
        flow: params.get("flow").cloned().unwrap_or_default(),
        network: params.get("type").cloned().unwrap_or_else(|| "tcp".to_string()),
        tls: params.get("security").map(|s| s == "reality" || s == "tls").unwrap_or(false),
        udp: true,
        fingerprint: params.get("fp").cloned().unwrap_or_else(|| "chrome".to_string()),
        public_key: params.get("pbk").cloned().unwrap_or_default(),
        short_id: params.get("sid").cloned().unwrap_or_default(),
        sni: params.get("sni").cloned().unwrap_or_default(),
        source: "subscription".to_string(),
    })
}

/// 从节点名称猜测地区标签（用于分组展示）
pub fn guess_region(name: &str) -> String {
    let n = name.to_lowercase();
    let pairs = [
        ("美国", vec!["美国", "美", "us", "usa", "america", "texas", "硅谷", "洛杉矶", "纽约"]),
        ("日本", vec!["日本", "日", "jp", "japan", "tokyo", "东京", "大阪"]),
        ("香港", vec!["香港", "港", "hk", "hongkong", "hong kong"]),
        ("台湾", vec!["台湾", "台", "tw", "taiwan", "台北"]),
        ("新加坡", vec!["新加坡", "新", "sg", "singapore", "狮城"]),
        ("韩国", vec!["韩国", "韩", "kr", "korea", "首尔"]),
        ("英国", vec!["英国", "英", "uk", "britain", "伦敦"]),
        ("德国", vec!["德国", "德", "de", "germany", "法兰克福"]),
    ];
    for (region, keys) in pairs {
        for k in keys {
            if n.contains(k) {
                return region.to_string();
            }
        }
    }
    String::new()
}

/// 简易 URL 百分号解码（%XX）
fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Ok(h), Ok(l)) = (
                u8::from_str_radix(&s[i + 1..i + 2], 16),
                u8::from_str_radix(&s[i + 2..i + 3], 16),
            ) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vless_uri_reality() {
        let uri = "vless://268a1166-d31e-478c-a66f-7f9c06c9afaa@104.160.40.35:443?encryption=none&security=reality&sni=www.microsoft.com&fp=chrome&pbk=c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0&sid=be08e6123ddcaf32&flow=xtls-rprx-vision&type=tcp#%E6%90%AC%E7%93%A6%E5%B7%A5%E7%9B%B4%E8%BF%9E";
        let node = parse_vless_uri(uri).expect("parse should succeed");
        assert_eq!(node.server, "104.160.40.35");
        assert_eq!(node.port, 443);
        assert_eq!(node.uuid, "268a1166-d31e-478c-a66f-7f9c06c9afaa");
        assert_eq!(node.flow, "xtls-rprx-vision");
        assert_eq!(node.public_key, "c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0");
        assert_eq!(node.short_id, "be08e6123ddcaf32");
        assert_eq!(node.sni, "www.microsoft.com");
        assert_eq!(node.name, "\u{642c}\u{74e6}\u{5de5}\u{76f4}\u{8fde}");
    }
}
