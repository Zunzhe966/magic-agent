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
        // 如果 PID 不在（例如 mihomo 由外部/MCP 启动），但混合端口开放，
        // 也认为代理在运行，这样界面状态与真实状态一致。
        if !alive && TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
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
        self.copy_geo_files()?;
        let conf = self.build_conf(cfg, rules, app_rules);
        let conf_path = self.runtime_dir.join("mihomo.yaml");
        std::fs::write(&conf_path, conf).map_err(|e| format!("写配置失败: {e}"))?;

        let bin = self.bin_path();
        if !bin.exists() {
            return Err(format!("代理内核不存在: {}", bin.display()));
        }

        let log_path = self.runtime_dir.join("mihomo.log");
        let err_path = self.runtime_dir.join("mihomo.err.log");
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
        let conf_path = self.runtime_dir.join("mihomo.yaml");
        std::fs::write(&conf_path, conf).map_err(|e| format!("写配置失败: {e}"))?;

        // 2. 找 mihomo PID
        let out = Command::new("/bin/ps")
            .args(["-axo", "pid=,args="])
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
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

    /// 只生成 rules 列表（供热更新用），逻辑与 build_conf 的 rules 段保持一致。
    fn build_rules(&self, cfg: &AppConfig, rules: &[String], app_rules: &[(Vec<String>, String)]) -> Vec<String> {
        let mut out = Vec::new();
        for node in &cfg.nodes {
            if node.server.parse::<std::net::IpAddr>().is_ok() {
                out.push(format!("IP-CIDR,{}/32,DIRECT,no-resolve", node.server));
            } else {
                out.push(format!("DOMAIN-SUFFIX,{},DIRECT", node.server));
            }
        }
        for (paths, target) in app_rules {
            for p in paths {
                out.push(format!("PROCESS-PATH-REGEX,^{},{}", regex_escape_path(p), target));
            }
        }
        for dr in &cfg.domain_rules {
            let target = if dr.target == "proxy" { "PROXY" } else { "DIRECT" };
            out.push(format!("DOMAIN-SUFFIX,{},{}", dr.domain, target));
        }
        for r in rules {
            out.push(r.clone());
        }
        out.push("GEOIP,LAN,DIRECT,no-resolve".to_string());
        out.push("MATCH,DIRECT".to_string());
        out
    }

    pub fn stop(&self) {
        let mut guard = self.pid.lock().unwrap();
        if let Some(pid) = guard.take() {
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
        let p = self.resource_root().join("bin/mihomo");
        if p.exists() { p } else { PathBuf::from("/usr/local/bin/mihomo") }
    }

    fn build_conf(&self, cfg: &AppConfig, rules: &[String], app_rules: &[(Vec<String>, String)]) -> String {
        let mut out = String::new();
        out.push_str(&format!("mixed-port: {}\n", self.port));
        out.push_str("mode: rule\n");
        // TUN 模式：mihomo 以 root 运行并接管系统路由，PROCESS-PATH-REGEX 依赖 TUN 连接表做进程匹配
        out.push_str("tun:\n  enable: true\n  stack: system\n  auto-route: true\n  auto-detect-interface: true\n  dns-hijack:\n    - any:53\n");
        out.push_str("log-level: info\n");
        out.push_str("allow-lan: false\n");
        out.push_str("ipv6: false\n");
        out.push_str("find-process-mode: always\n");
        out.push_str("external-controller: 127.0.0.1:19091\n");
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
                node.name, node.server, node.port, node.uuid, node.network, node.tls, node.udp, node.flow, node.fingerprint
            ));
            if !node.sni.is_empty() {
                out.push_str(&format!("    servername: \"{}\"\n", node.sni));
            }
            if !node.public_key.is_empty() {
                out.push_str(&format!(
                    "    reality-opts:\n      public-key: \"{}\"\n      short-id: \"{}\"\n",
                    node.public_key, node.short_id
                ));
            }
        }

        out.push_str("\nproxy-groups:\n");
        // PROXY 组：把选中的节点排到第一个，select 默认选中第一项，从而让 selected_node 真正生效
        out.push_str("  - name: PROXY\n    type: select\n    proxies:\n");
        let selected = cfg.selected_node.clone().unwrap_or_default();
        let mut ordered: Vec<&crate::config::ProxyNode> = cfg.nodes.iter().collect();
        ordered.sort_by_key(|n| if n.name == selected { 0 } else { 1 });
        for node in ordered {
            out.push_str(&format!("      - \"{}\"\n", node.name));
        }
        // 每个节点独立成组：软件分流规则可以精确指向"走哪个节点"
        for node in &cfg.nodes {
            out.push_str(&format!(
                "  - name: \"NODE-{}\"\n    type: select\n    proxies:\n      - \"{}\"\n",
                node.name, node.name
            ));
        }
        out.push_str("  - name: AUTO\n    type: url-test\n    url: http://www.gstatic.com/generate_204\n    interval: 300\n    proxies:\n");
        for node in &cfg.nodes {
            out.push_str(&format!("      - \"{}\"\n", node.name));
        }

        out.push_str("\nrules:\n");
        // rules 段与热更新共用同一份生成逻辑，避免两处不一致
        for r in self.build_rules(cfg, rules, app_rules) {
            out.push_str(&format!("  - {}\n", r));
        }
        out
    }
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
    fn generated_conf_is_valid_mihomo_yaml() {
        let m = MihomoManager::new();
        let node = ProxyNode {
            name: "test-node".to_string(),
            server: "104.160.40.35".to_string(),
            port: 443,
            uuid: "test-uuid".to_string(),
            flow: "xtls-rprx-vision".to_string(),
            network: "tcp".to_string(),
            tls: true,
            udp: true,
            fingerprint: "chrome".to_string(),
            public_key: "c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0".to_string(),
            short_id: "be08e6123ddcaf32".to_string(),
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
}
