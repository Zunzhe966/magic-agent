use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

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
    pub stdout: Mutex<Option<ChildStdout>>,
    pub session: Mutex<Option<SshSession>>,
}

impl SshManager {
    pub fn new() -> Self {
        Self { child: Mutex::new(None), stdin: Mutex::new(None), stdout: Mutex::new(None), session: Mutex::new(None) }
    }

    pub fn connect(&self, host: String, port: u16, user: String, auth: String, password: Option<String>, key: Option<String>) -> Result<SshSession, String> {
        self.disconnect();
        let mut cmd = Command::new("/usr/bin/ssh");
        cmd.arg("-tt").arg("-o").arg("StrictHostKeyChecking=accept-new").arg("-o").arg("ServerAliveInterval=15");
        if port != 22 {
            cmd.arg("-p").arg(port.to_string());
        }
        if let Some(k) = key {
            if !k.trim().is_empty() {
                cmd.arg("-i").arg(k);
            }
        }
        cmd.arg(format!("{}@{}", user, host));
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("启动 ssh 失败: {e}"))?;
        let stdin = child.stdin.take().ok_or("无法获取 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取 stdout")?;
        // 若密码方式，用 expect 脚本包装更可靠
        // 此处保留基础实现：密码走 expect 子进程处理，防止 shell 交互
        if auth == "password" {
            if let Some(p) = password {
                if !p.is_empty() {
                    drop(child);
                    return self.connect_expect(host, port, user, p);
                }
            }
        }
        *self.child.lock().unwrap() = Some(child);
        *self.stdin.lock().unwrap() = Some(stdin);
        *self.stdout.lock().unwrap() = Some(stdout);
        let sess = SshSession { id: format!("ssh-{}", host), host, port, user, status: "connected".into() };
        *self.session.lock().unwrap() = Some(sess.clone());
        Ok(sess)
    }

    fn connect_expect(&self, host: String, port: u16, user: String, password: String) -> Result<SshSession, String> {
        let mut cmd = Command::new("/usr/bin/expect");
        cmd.arg("-c").arg(format!(
            "spawn ssh -tt -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=15 {} -l {} -p {}; expect \"*assword:*\"; send \"{}\\r\"; interact",
            host, user, port, password
        ));
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("启动 expect 失败: {e}"))?;
        let stdin = child.stdin.take().ok_or("无法获取 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取 stdout")?;
        *self.child.lock().unwrap() = Some(child);
        *self.stdin.lock().unwrap() = Some(stdin);
        *self.stdout.lock().unwrap() = Some(stdout);
        let sess = SshSession { id: format!("ssh-{}", host), host, port, user, status: "connected".into() };
        *self.session.lock().unwrap() = Some(sess.clone());
        Ok(sess)
    }

    pub fn disconnect(&self) {
        let mut child = self.child.lock().unwrap();
        if let Some(mut c) = child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        *self.stdin.lock().unwrap() = None;
        *self.stdout.lock().unwrap() = None;
        *self.session.lock().unwrap() = None;
    }

    pub fn write(&self, data: Vec<u8>) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap();
        let s = stdin.as_mut().ok_or("未连接")?;
        s.write_all(&data).map_err(|e| e.to_string())
    }

    pub fn read(&self) -> Result<Vec<u8>, String> {
        let mut stdout = self.stdout.lock().unwrap();
        let s = stdout.as_mut().ok_or("未连接")?;
        let mut buf = [0u8; 4096];
        match s.read(&mut buf) {
            Ok(n) if n > 0 => Ok(buf[..n].to_vec()),
            Ok(_) => Ok(vec![]),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn status(&self) -> Option<SshSession> {
        let g = self.session.lock().unwrap();
        g.clone()
    }
}
