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

/// 域名分流规则：按域名后缀指定去向，优先级高于国内域名清单（GEOSITE,cn）和进程规则。
/// 解决同一个软件内部混合源的问题（如下载器：GitHub 走代理、国内源直连）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainRule {
    pub domain: String,   // 如 github.com、huggingface.co
    pub target: String,   // "proxy" 走代理 | "direct" 直连 | 节点名（走该节点）
    /// 服务于哪个密钥/软件（如"WorkBuddy 的 OpenRouter 密钥"），防止日久失忆误删
    #[serde(default)]
    pub reason: String,
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
    /// mihomo 控制 API（127.0.0.1:19091）的鉴权 secret。
    /// 缺失时在 load() 自动生成并持久化，防止本机任意进程/网页 CSRF 操控代理。
    #[serde(default)]
    pub api_secret: Option<String>,
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
            api_secret: None,
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
    let mut cfg: AppConfig = match std::fs::read_to_string(&p) {
        Ok(s) => match serde_json::from_str::<AppConfig>(&s) {
            Ok(c) => c,
            Err(e) => {
                // 配置损坏：不静默吞掉，先备份坏文件再回退默认，避免用户节点/设置全丢
                eprintln!("[magic-agent] config.json 解析失败，已备份为 .corrupt 并回退默认配置: {e}");
                let bak = p.with_extension("json.corrupt");
                let _ = std::fs::copy(&p, &bak);
                AppConfig::default()
            }
        },
        Err(_) => AppConfig::default(),
    };
    // 首次运行生成 API secret 并持久化（mihomo external-controller 的鉴权令牌）
    if cfg.api_secret.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
        cfg.api_secret = Some(generate_api_secret());
        let _ = save(&cfg);
    }
    cfg
}

/// 从 /dev/urandom 读 16 字节转 hex，无第三方依赖。
/// 失败时用当前纳秒时间戳 + 进程 id + 计数器异或打散，避免退化成全 0 的弱密钥。
fn generate_api_secret() -> String {
    use std::io::Read;
    let mut buf = [0u8; 16];
    let mut filled = false;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            filled = true;
        }
    }
    if !filled {
        // 兜底：时间戳/进程号/地址异或，仍比全 0 强，且每次调用都不同
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let pid = std::process::id() as u64;
        let addr = &buf as *const _ as u64;
        let mut seed = now ^ pid.rotate_left(17) ^ addr.rotate_left(31) ^ 0x9e3779b97f4a7c15u64;
        for b in buf.iter_mut() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *b = (seed & 0xff) as u8;
        }
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let p = config_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // 原子写：临时文件+rename，防止与 Python(MCP) 并发写时出现半截 JSON
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    // 配置含 apiSecret 与节点密钥，收紧为仅当前用户可读写
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}


/// 校验订阅 URL 的 host 是否指向公网，拒绝回环/内网/链路本地/组播/保留地址，
/// 防止恶意前端借 fetch_subscription 触发 SSRF 拉取内网内容。
/// 返回 Ok(()) 表示可安全访问；Err(msg) 表示被拦截。
pub fn validate_public_host(host: &str) -> Result<(), String> {
    // 去端口（IPv6 用方括号包裹）
    let h = host.trim();
    let host_only = if h.starts_with('[') {
        // [::1]:8080 形式
        match h.find(']') {
            Some(i) => &h[1..i],
            None => h,
        }
    } else if h.matches(':').count() > 1 {
        // 裸 IPv6（无方括号、无端口），如 ::1、fe80::1
        h
    } else {
        // host 或 host:port；IPv4 直接按冒号分隔，域名无冒号
        match h.rfind(':') {
            Some(i) if h[i + 1..].chars().all(|c| c.is_ascii_digit()) => &h[..i],
            _ => h,
        }
    };
    if host_only.is_empty() {
        return Err("订阅地址 host 为空".to_string());
    }
    // 字面 IP：直接判内网/保留地址
    if let Ok(ip) = host_only.parse::<std::net::IpAddr>() {
        if is_private_or_reserved(ip) {
            return Err(format!("订阅地址指向内网/保留地址 {}，已拦截", ip));
        }
        return Ok(());
    }
    // 域名：做 DNS 解析，任一解析结果落到内网/保留地址即拦截，
    // 堵住「域名解析到内网」的 SSRF 绕过（如 http://intranet.corp.local/）。
    // 注意：这里只做静态解析校验；真正的拉取仍走 curl，需配合 --resolve 或在
    // 解析后重连才 100% 防 DNS rebinding，但本机单人场景下先拦常见绕过。
    if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(host_only, 443)) {
        for a in addrs {
            if is_private_or_reserved(a.ip()) {
                return Err(format!("订阅域名 {} 解析到内网/保留地址 {}，已拦截", host_only, a.ip()));
            }
        }
    }
    Ok(())
}

/// 判断 IP 是否为回环/内网/链路本地/组播/未指定等非公网地址。
pub fn is_private_or_reserved(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                // 100.64.0.0/10 运营商级 NAT（CGNAT），非公网
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 0x40)
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // IPv4-mapped IPv6 内网地址（::ffff:127.0.0.1 等）
                || v6.to_ipv4().map(is_private_or_reserved_ipv4).unwrap_or(false)
                // 唯一本地地址 fc00::/7（含 fd00::/8）
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // 链路本地 fe80::/10
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

pub fn is_private_or_reserved_ipv4(v4: std::net::Ipv4Addr) -> bool {
    is_private_or_reserved(std::net::IpAddr::V4(v4))
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
    let mut seen = std::collections::HashSet::new();
    for line in content.lines() {
        let line = line.trim();
        let Some(idx) = line.find("vless://") else { continue };
        let uri = &line[idx..];
        if let Ok(node) = parse_vless_uri(uri) {
            // 按 server:port 去重：订阅可能重复返回同一节点，避免重复添加
            let key = format!("{}:{}", node.server, node.port);
            if seen.insert(key) {
                nodes.push(node);
            }
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
    fn validate_public_host_blocks_private() {
        // 回环
        assert!(validate_public_host("127.0.0.1").is_err());
        assert!(validate_public_host("127.0.0.1:8080").is_err());
        assert!(validate_public_host("[::1]:8080").is_err());
        assert!(validate_public_host("::1").is_err());
        // 内网
        assert!(validate_public_host("192.168.1.1").is_err());
        assert!(validate_public_host("10.0.0.1").is_err());
        assert!(validate_public_host("172.16.0.1").is_err());
        assert!(validate_public_host("100.64.0.1").is_err());
        // 链路本地 / 组播 / 未指定
        assert!(validate_public_host("169.254.1.1").is_err());
        assert!(validate_public_host("224.0.0.1").is_err());
        assert!(validate_public_host("0.0.0.0").is_err());
        assert!(validate_public_host("fe80::1").is_err());
        assert!(validate_public_host("fc00::1").is_err());
        assert!(validate_public_host("fd00::1").is_err());
        // IPv4-mapped 内网
        assert!(validate_public_host("::ffff:127.0.0.1").is_err());
    }

    #[test]
    fn validate_public_host_allows_public_and_domain() {
        // 公网 IP
        assert!(validate_public_host("8.8.8.8").is_ok());
        assert!(validate_public_host("1.1.1.1:443").is_ok());
        // 域名
        assert!(validate_public_host("example.com").is_ok());
        assert!(validate_public_host("sub.example.com:8080").is_ok());
        // 空 host 拒绝
        assert!(validate_public_host("").is_err());
        assert!(validate_public_host(":8080").is_err());
    }

    #[test]
    fn validate_public_host_blocks_domain_resolving_to_localhost() {
        // 域名解析到回环地址，应被 SSRF 防护拦截（堵住域名绕过）
        assert!(validate_public_host("localhost").is_err());
        assert!(validate_public_host("localhost.localdomain").is_err());
    }

    #[test]
    fn parse_vless_uri_reality() {
        let uri = "vless://00000000-0000-4000-8000-000000000000@1.1.1.1:443?encryption=none&security=reality&sni=www.example.com&fp=chrome&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=0000000000000000&flow=xtls-rprx-vision&type=tcp#%E7%A4%BA%E4%BE%8B%E8%8A%82%E7%82%B9";
        let node = parse_vless_uri(uri).expect("parse should succeed");
        assert_eq!(node.server, "1.1.1.1");
        assert_eq!(node.port, 443);
        assert_eq!(node.uuid, "00000000-0000-4000-8000-000000000000");
        assert_eq!(node.flow, "xtls-rprx-vision");
        assert_eq!(node.public_key, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        assert_eq!(node.short_id, "0000000000000000");
        assert_eq!(node.sni, "www.example.com");
        assert_eq!(node.name, "\u{793a}\u{4f8b}\u{8282}\u{70b9}");
    }

    #[test]
    fn active_server_returns_key_auth_with_key_path() {
        // 密钥认证服务器：active_server 必须保留 key_path（否则 SSH 探针无法用密钥登录）
        let cfg = AppConfig {
            servers: vec![ServerInfo {
                id: "server-example".into(),
                name: "示例服务器".into(),
                host: "1.1.1.1".into(),
                port: 22022,
                user: "root".into(),
                auth: "key".into(),
                password_saved: false,
                private_key_saved: true,
                key_path: Some("~/.ssh/example_ed25519".into()),
            }],
            active_server_id: Some("server-example".into()),
            ..Default::default()
        };
        let s = cfg.active_server().expect("active_server 应返回服务器");
        assert_eq!(s.host, "1.1.1.1");
        assert_eq!(s.port, 22022);
        assert_eq!(s.auth, "key");
        assert_eq!(s.key_path.as_deref(), Some("~/.ssh/example_ed25519"));
    }
}
