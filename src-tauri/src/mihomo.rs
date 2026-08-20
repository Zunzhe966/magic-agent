use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MihomoStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
}

pub struct MihomoManager {
    pub child: Mutex<Option<Child>>,
    pub port: u16,
    pub config_dir: PathBuf,
}

impl MihomoManager {
    pub fn new() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("magic-agent");
        Self { child: Mutex::new(None), port: 7890, config_dir }
    }

    pub fn status(&self) -> MihomoStatus {
        let mut guard = self.child.lock().unwrap();
        let alive = guard.as_mut().map(|c| c.try_wait().ok().flatten().is_none()).unwrap_or(false);
        if !alive {
            *guard = None;
        }
        MihomoStatus { running: alive, pid: guard.as_ref().and_then(|c| c.id().into()), port: self.port }
    }

    pub fn start(&self, cfg: &AppConfig, rules: &[String], direct_apps: &[String], proxy_apps: &[String]) -> Result<MihomoStatus, String> {
        self.stop();
        let _ = std::fs::create_dir_all(&self.config_dir);
        let conf = self.build_conf(cfg, rules, direct_apps, proxy_apps);
        let conf_path = self.config_dir.join("mihomo.yaml");
        std::fs::write(&conf_path, conf).map_err(|e| format!("写配置失败: {e}"))?;

        let bin = self.bin_path();
        let child = Command::new(&bin)
            .arg("-f")
            .arg(&conf_path)
            .arg("-d")
            .arg(&self.config_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("启动 mihomo 失败: {e}"))?;
        let pid = child.id();
        *self.child.lock().unwrap() = Some(child);
        // 等待端口起来
        for _ in 0..50 {
            if std::net::TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Ok(MihomoStatus { running: true, pid: Some(pid), port: self.port })
    }

    pub fn stop(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(child) = guard.take() {
            let _ = kill_child(child);
        }
    }

    fn bin_path(&self) -> PathBuf {
        let rel = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/mihomo");
        if rel.exists() {
            rel
        } else {
            PathBuf::from("/Applications/魔法代理.app/Contents/Resources/bin/mihomo")
        }
    }

    fn build_conf(&self, cfg: &AppConfig, rules: &[String], direct_apps: &[String], proxy_apps: &[String]) -> String {
        let mut out = String::new();
        out.push_str(&format!("mixed-port: {}\n", self.port));
        out.push_str("mode: rule\n");
        out.push_str("log-level: info\n");
        out.push_str("allow-lan: false\n");
        out.push_str("ipv6: false\n");
        out.push_str("find-process-mode: always\n");
        out.push_str("external-controller: 127.0.0.1:19090\n");
        out.push_str("\ndns:\n  enable: true\n  listen: 127.0.0.1:1053\n  enhanced-mode: fake-ip\n  fake-ip-range: 198.18.0.1/16\n  nameserver:\n    - 223.5.5.5\n    - 119.29.29.29\n  fallback:\n    - tls://8.8.8.8\n    - tls://1.1.1.1\n\n");

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
        out.push_str("  - name: PROXY\n    type: select\n    proxies:\n");
        for node in &cfg.nodes {
            out.push_str(&format!("      - \"{}\"\n", node.name));
        }
        out.push_str("  - name: AUTO\n    type: url-test\n    url: http://www.gstatic.com/generate_204\n    interval: 300\n    proxies:\n");
        for node in &cfg.nodes {
            out.push_str(&format!("      - \"{}\"\n", node.name));
        }
        out.push_str("  - name: DIRECT\n    type: select\n    proxies:\n      - DIRECT\n");

        out.push_str("\nrules:\n");
        // 1) 按软件：显式直连
        for a in direct_apps {
            out.push_str(&format!("  - PROCESS-PATH-REGEX,{},\"DIRECT\"\n", regex_escape_path(a)));
        }
        // 2) 按软件：显式代理
        for a in proxy_apps {
            out.push_str(&format!("  - PROCESS-PATH-REGEX,{},\"PROXY\"\n", regex_escape_path(a)));
        }
        // 3) 用户规则
        for r in rules {
            out.push_str(&format!("  - {}\n", r));
        }
        out.push_str("  - MATCH,PROXY\n");
        out
    }
}

fn kill_child(mut child: Child) -> Result<(), String> {
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

fn regex_escape_path(s: &str) -> String {
    // 转成正则：把普通路径字符转义，/ 保留，点/括号转义
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
