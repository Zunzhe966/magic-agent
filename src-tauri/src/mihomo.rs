use serde::{Deserialize, Serialize};
use std::net::TcpStream;
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
    pub node: Option<String>,
}

pub struct MihomoManager {
    pub child: Mutex<Option<Child>>,
    pub port: u16,
    pub runtime_dir: PathBuf,
}

impl MihomoManager {
    pub fn new() -> Self {
        let runtime_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("magic-agent")
            .join("runtime");
        Self { child: Mutex::new(None), port: 7891, runtime_dir }
    }

    pub fn status(&self) -> MihomoStatus {
        let mut guard = self.child.lock().unwrap();
        let alive = guard.as_mut().map(|c| c.try_wait().ok().flatten().is_none()).unwrap_or(false);
        if !alive {
            *guard = None;
        }
        MihomoStatus {
            running: alive,
            pid: guard.as_ref().and_then(|c| c.id().into()),
            port: self.port,
            node: None,
        }
    }

    pub fn start(&self, cfg: &AppConfig, rules: &[String], direct_apps: &[String], proxy_apps: &[String]) -> Result<MihomoStatus, String> {
        self.stop();
        let _ = std::fs::create_dir_all(&self.runtime_dir);
        self.copy_geo_files()?;
        let conf = self.build_conf(cfg, rules, direct_apps, proxy_apps);
        let conf_path = self.runtime_dir.join("mihomo.yaml");
        std::fs::write(&conf_path, conf).map_err(|e| format!("写配置失败: {e}"))?;

        let bin = self.bin_path();
        if !bin.exists() {
            return Err(format!("代理内核不存在: {}", bin.display()));
        }
        let child = Command::new(&bin)
            .arg("-f")
            .arg(&conf_path)
            .arg("-d")
            .arg(&self.runtime_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(std::fs::File::create(self.runtime_dir.join("mihomo.log")).map_err(|e| format!("创建日志失败: {e}"))?))
            .stderr(Stdio::from(std::fs::File::create(self.runtime_dir.join("mihomo.err.log")).map_err(|e| format!("创建错误日志失败: {e}"))?))
            .spawn()
            .map_err(|e| format!("启动 mihomo 失败: {e}"))?;
        let pid = child.id();
        *self.child.lock().unwrap() = Some(child);
        let mut ok = false;
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                ok = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if !ok {
            return Err(format!("代理端口 {} 未能启动", self.port));
        }
        Ok(MihomoStatus {
            running: true,
            pid: Some(pid),
            port: self.port,
            node: cfg.selected_node.clone(),
        })
    }

    pub fn stop(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
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

    fn build_conf(&self, cfg: &AppConfig, rules: &[String], direct_apps: &[String], proxy_apps: &[String]) -> String {
        let mut out = String::new();
        out.push_str(&format!("mixed-port: {}
", self.port));
        out.push_str("mode: rule\n");
        out.push_str("log-level: info\n");
        out.push_str("allow-lan: false\n");
        out.push_str("ipv6: false\n");
        out.push_str("find-process-mode: always\n");
        out.push_str("external-controller: 127.0.0.1:19091\n");
        out.push_str("geo-auto-update: false\n");
        out.push_str("geodata-mode: false\n");
        out.push_str("geodata-loader: memconservative\n\n");
        out.push_str("dns:\n  enable: true\n  listen: 127.0.0.1:1054\n  enhanced-mode: redir-host\n  nameserver:\n    - 223.5.5.5\n    - 119.29.29.29\n  fallback:\n    - tls://8.8.8.8\n    - tls://1.1.1.1\n  fallback-filter:\n    geoip: true\n    geoip-code: CN\n\n");

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


        out.push_str("\nrules:\n");
        // 1) 按软件：显式直连，优先级最高
        for a in direct_apps {
            out.push_str(&format!("  - PROCESS-PATH-REGEX,{},DIRECT\n", regex_escape_path(a)));
        }
        // 2) 按软件：显式代理
        for a in proxy_apps {
            out.push_str(&format!("  - PROCESS-PATH-REGEX,{},PROXY\n", regex_escape_path(a)));
        }
        // 3) 用户规则
        for r in rules {
            out.push_str(&format!("  - {}\n", r));
        }
        // 4) 国内/内网直连，避免代理吞掉本地流量
        out.push_str("  - GEOIP,CN,DIRECT\n  - GEOIP,LAN,DIRECT\n  - GEOSITE,cn,DIRECT\n");
        // 5) 其余走代理
        out.push_str("  - MATCH,PROXY\n");
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
    #[test]
    fn resource_root_exists() {
        let m = MihomoManager::new();
        let root = m.resource_root();
        assert!(root.exists(), "resource root missing: {}", root.display());
        assert!(root.join("bin/mihomo").exists(), "mihomo binary missing");
    }
}
