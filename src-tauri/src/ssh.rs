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

    /// 以 0600 权限写入 expect 临时脚本（含明文密码），并清理历史残留文件。
    /// 已废弃：connect 改为用 stdin 喂脚本，密码不再落盘。保留此清理逻辑，
    /// 用于清除旧版本遗留的 magic-ssh-*.expect 文件。
    #[allow(dead_code)]
    fn cleanup_legacy_expect_scripts() {
        if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
            for e in entries.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("magic-ssh-") && name.ends_with(".expect") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
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
                "#!/usr/bin/expect -f\nset timeout 20\nspawn {}\nexpect {{\n  -re \"(?i)password:\\s*\" {{\n    send \"{}\\r\"\n  }}\n  -re \"Are you sure.*\" {{\n    send \"yes\\r\"\n    exp_continue\n  }}\n  eof {{ exit 1 }}\n}}\ninteract\n",
                args.join(" "), escaped
            );
            // 明文密码永不落盘：把脚本通过 stdin 喂给 expect（`expect -f -`），
            // 彻底消除"临时脚本删得太快导致 expect 读不到文件"的时序竞态，
            // 也避免任何磁盘残留风险。
            let mut s = Command::new("/usr/bin/expect");
            s.arg("-f").arg("-");
            s.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut spawned = match s.spawn() {
                Ok(c) => c,
                Err(e) => {
                    return Err(format!("启动 SSH 失败: {e}"));
                }
            };
            {
                let sin = spawned.stdin.as_mut();
                if let Some(sin) = sin {
                    use std::io::Write;
                    let _ = sin.write_all(script.as_bytes());
                }
            }
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

    /// 非交互式执行单条命令并返回 (stdout, stderr, exit_code)。
    /// 独立于交互式会话：重新起一个 ssh 进程跑一次命令，读完输出即断开。
    /// 用于「云服务器探针」——采集 CPU/内存/磁盘/带宽等，不污染交互式终端。
    pub fn exec(
        &self,
        host: String,
        port: u16,
        user: String,
        auth: String,
        command: String,
        timeout_secs: u64,
        key_path: Option<String>,
    ) -> Result<(String, String, i32), String> {
        let pw_from_chain = keychain::get(&Self::password_account(&host, &user)).ok();
        let use_password = auth == "password" && pw_from_chain.as_deref().unwrap_or("").len() > 0;

        let mut args = vec![
            "/usr/bin/ssh".to_string(),
            "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
            "-o".to_string(), "ConnectTimeout=10".to_string(),
            "-o".to_string(), "BatchMode=no".to_string(),
        ];
        if port != 22 { args.push("-p".to_string()); args.push(port.to_string()); }
        // 密钥认证：显式指定 -i（展开 ~），不依赖 ~/.ssh/config 的 Host 别名匹配。
        if auth == "key" {
            if let Some(kp) = key_path {
                let kp = kp.trim().to_string();
                if !kp.is_empty() {
                    args.push("-i".to_string());
                    args.push(expand_ssh_key(&kp).to_string_lossy().to_string());
                }
            }
        }
        args.push(format!("{}@{}", user, host));
        args.push(command);

        // 密码认证：用 expect 喂密码（密码从 Keychain 取，不落盘、不出现在命令行）
        let output = if use_password {
            let pw = pw_from_chain.unwrap_or_default();
            let escaped = pw
                .replace('\\', "\\\\").replace('"', "\\\"")
                .replace('$', "\\$").replace('`', "\\`")
                .replace('[', "\\[").replace(']', "\\]");
            // expect 脚本：等 password 提示后送密码，命令跑完自然 eof
            let script = format!(
                "#!/usr/bin/expect -f\nset timeout {}\nspawn {}\nexpect {{\n  -re \"(?i)password:\\s*\" {{ send \"{}\\r\" }}\n  -re \"Are you sure.*\" {{ send \"yes\\r\"; exp_continue }}\n  eof {{ exit 1 }}\n}}\nexpect eof\n",
                timeout_secs,
                args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" "),
                escaped
            );
            let mut s = Command::new("/usr/bin/expect");
            s.arg("-f").arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut spawned = s.spawn().map_err(|e| format!("启动 SSH 失败: {e}"))?;
            if let Some(sin) = spawned.stdin.as_mut() {
                use std::io::Write;
                let _ = sin.write_all(script.as_bytes());
            }
            // 关键：wait_with_output 不设超时会永久挂起（SSH 卡住 / 密码错误等待重试）。
            // 用 try_wait 轮询 + 超时 kill，配合 expect 脚本内 `set timeout`，避免挂死 Tauri 命令线程。
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_secs(timeout_secs + 10);
            let output: std::process::Output;
            loop {
                if let Some(status) = spawned.try_wait().map_err(|e| e.to_string())? {
                    output = std::process::Output {
                        status,
                        stdout: read_remaining(&mut spawned.stdout.take()),
                        stderr: read_remaining_stderr(&mut spawned.stderr.take()),
                    };
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    let _ = spawned.kill();
                    let _ = spawned.wait();
                    return Err(format!("SSH 执行超时（>{}s）", timeout_secs));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            output
        } else {
            // 密钥认证：直接跑，走已配置的 ssh key
            Command::new("/usr/bin/ssh")
                .args(&args[1..])
                .stdin(Stdio::null())
                .output()
                .map_err(|e| format!("启动 SSH 失败: {e}"))?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(-1);
        Ok((stdout, stderr, code))
    }
}

/// 给 SSH 参数做 shell 引号包裹，供 expect spawn 使用（简单实现：单引号包裹并转义内部单引号）
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// 从已结束的子进程句柄读尽残留的 stdout/stderr（配合 try_wait 轮询使用）。
fn read_remaining(pipe: &mut Option<std::process::ChildStdout>) -> Vec<u8> {
    use std::io::Read;
    let mut buf = Vec::new();
    if let Some(p) = pipe {
        let _ = p.read_to_end(&mut buf);
    }
    buf
}

/// 从已结束的子进程句柄读尽残留的 stderr。
fn read_remaining_stderr(pipe: &mut Option<std::process::ChildStderr>) -> Vec<u8> {
    use std::io::Read;
    let mut buf = Vec::new();
    if let Some(p) = pipe {
        let _ = p.read_to_end(&mut buf);
    }
    buf
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
