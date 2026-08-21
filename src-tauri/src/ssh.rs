use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::keychain;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshSession {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub status: String,
}

pub struct SshManager {
    pub child: Mutex<Option<Child>>,
    pub stdin: Mutex<Option<ChildStdin>>,
    pub buffer: Arc<Mutex<Vec<u8>>>,
    pub session: Mutex<Option<SshSession>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            buffer: Arc::new(Mutex::new(Vec::new())),
            session: Mutex::new(None),
        }
    }

    /// Keychain 中存储 SSH 密码所用的 account 名
    pub fn password_account(host: &str, user: &str) -> String {
        format!("ssh-password-{}@{}", user, host)
    }

    /// Keychain 中存储 SSH 私钥内容所用的 account 名
    pub fn key_account(host: &str, user: &str) -> String {
        format!("ssh-key-{}@{}", user, host)
    }

    pub fn connect(&self, host: String, port: u16, user: String, auth: String, password: Option<String>, key: Option<String>) -> Result<SshSession, String> {
        self.disconnect();

        // key 路径展开（~ 支持）
        let key = key.map(|k| {
            let t = k.trim().to_string();
            if t.is_empty() { t } else { expand_ssh_key(&t).to_string_lossy().to_string() }
        });

        // 如果调用方没有给明文密码，尝试从 Keychain 读取
        let pw_from_chain = keychain::get(&Self::password_account(&host, &user)).ok();
        let password = password
            .filter(|p| !p.trim().is_empty())
            .or(pw_from_chain);

        let mut cmd = Command::new("/usr/bin/ssh");
        cmd.arg("-tt")
            .arg("-o").arg("StrictHostKeyChecking=accept-new")
            .arg("-o").arg("ServerAliveInterval=15")
            .arg("-o").arg("ServerAliveCountMax=3")
            .arg("-o").arg("ConnectTimeout=10");
        if port != 22 { cmd.arg("-p").arg(port.to_string()); }
        if let Some(ref k) = key { if !k.trim().is_empty() && std::path::Path::new(k.trim()).exists() { cmd.arg("-i").arg(k.trim()); } }
        cmd.arg(format!("{}@{}", user, host));

        let use_password = auth == "password" && password.as_deref().unwrap_or("").len() > 0;
        let mut child = if use_password {
            let pw = password.unwrap_or_default();
            let mut args = vec!["/usr/bin/ssh", "-tt", "-o", "StrictHostKeyChecking=accept-new", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=3", "-o", "ConnectTimeout=10"];
            let port_str = port.to_string();
            if port != 22 { args.push("-p"); args.push(&port_str); }
            if let Some(ref k) = key { if !k.trim().is_empty() && std::path::Path::new(k.trim()).exists() { args.push("-i"); args.push(k.trim()); } }
            let user_host = format!("{}@{}", user, host);
            args.push(&user_host);

            let escaped = pw
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace('`', "\\`")
                .replace('[', "\\[")
                .replace(']', "\\]");
            let script = format!(
                "#!/usr/bin/expect -f\nset timeout 20\nspawn {} {}\nexpect {{\n  -re \"(?i)password:\\s*\" {{\n    send \"{}\\r\"\n  }}\n  -re \"Are you sure.*\" {{\n    send \"yes\\r\"\n    exp_continue\n  }}\n  eof {{ exit 1 }}\n}}\ninteract\n",
                args.join(" "), String::new(), escaped
            );
            let tmp = std::env::temp_dir().join(format!("magic-ssh-{}.expect", std::process::id()));
            std::fs::write(&tmp, script).map_err(|e| format!("写 SSH 脚本失败: {e}"))?;
            let mut s = Command::new("/usr/bin/expect");
            s.arg("-f").arg(&tmp);
            s.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let spawned = s.spawn().map_err(|e| format!("启动 SSH 失败: {e}"))?;
            // 临时脚本含明文密码，spawn 成功后立即删除，避免残留
            let _ = std::fs::remove_file(&tmp);
            spawned
        } else {
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            cmd.spawn().map_err(|e| format!("启动 SSH 失败: {e}"))?
        };

        let stdin = child.stdin.take().ok_or("无法获取 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取 stdout")?;
        *self.child.lock().unwrap() = Some(child);
        *self.stdin.lock().unwrap() = Some(stdin);

        // 后台线程持续读取 stdout 到共享 buffer，避免阻塞 Tauri 命令线程
        let buf = self.buffer.clone();
        {
            let mut b = buf.lock().unwrap();
            b.clear();
        }
        let mut reader = stdout;
        std::thread::spawn(move || {
            let mut tmp = [0u8; 4096];
            loop {
                match reader.read(&mut tmp) {
                    Ok(n) if n > 0 => {
                        if let Ok(mut b) = buf.lock() {
                            b.extend_from_slice(&tmp[..n]);
                        }
                    }
                    _ => break,
                }
            }
        });

        let sess = SshSession { id: format!("ssh-{}@{}", user, host), host, port, user, status: "connected".to_string() };
        *self.session.lock().unwrap() = Some(sess.clone());
        Ok(sess)
    }

    pub fn disconnect(&self) {
        let mut child = self.child.lock().unwrap();
        if let Some(mut c) = child.take() { let _ = c.kill(); let _ = c.wait(); }
        *self.stdin.lock().unwrap() = None;
        *self.session.lock().unwrap() = None;
        if let Ok(mut b) = self.buffer.lock() {
            b.clear();
        }
    }

    pub fn write(&self, data: Vec<u8>) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap();
        let s = stdin.as_mut().ok_or("未连接")?;
        s.write_all(&data).map_err(|e| e.to_string())
    }

    pub fn read(&self) -> Result<Vec<u8>, String> {
        let mut buf = self.buffer.lock().unwrap();
        if buf.is_empty() { return Ok(vec![]); }
        Ok(std::mem::take(&mut *buf))
    }

    pub fn status(&self) -> Option<SshSession> {
        self.session.lock().unwrap().clone()
    }
}

pub fn expand_ssh_key(path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            let mut np = home;
            let comps: Vec<_> = p.components().skip(1).collect();
            for c in comps { np.push(c.as_os_str()); }
            return np;
        }
    }
    p
}
