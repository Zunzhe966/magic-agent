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

    /// 解析本次连接可用的私钥文件路径：
    /// 1) 显式路径（展开 ~ 且存在）→ 直接用；
    /// 2) 否则回退 Keychain 里保存的私钥内容 → 落成 0600 临时文件。
    /// 返回 (私钥路径, 临时文件)。临时文件用毕必须由调用方 remove_file。
    fn resolve_key_file(host: &str, user: &str, auth: &str, key: &Option<String>) -> (Option<String>, Option<PathBuf>) {
        if let Some(k) = key.as_deref().map(|k| k.trim()).filter(|k| !k.is_empty()) {
            let expanded = expand_ssh_key(k);
            if expanded.exists() {
                return (Some(expanded.to_string_lossy().to_string()), None);
            }
        }
        if auth == "key" {
            if let Ok(content) = keychain::get(&Self::key_account(host, user)) {
                if content.contains("PRIVATE KEY") {
                    if let Ok(p) = write_temp_key(&content) {
                        let path = p.to_string_lossy().to_string();
                        return (Some(path), Some(p));
                    }
                }
            }
        }
        (None, None)
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

        // 如果调用方没有给明文密码，尝试从 Keychain 读取
        let pw_from_chain = keychain::get(&Self::password_account(&host, &user)).ok();
        let password = password
            .filter(|p| !p.trim().is_empty())
            .or(pw_from_chain);

        // 私钥解析：显式路径优先；否则回退 Keychain 中保存的私钥内容
        // （落成 0600 临时文件供 -i 使用，认证通过后即删）
        let (key_file, key_tmp) = Self::resolve_key_file(&host, &user, &auth, &key);

        let mut args: Vec<String> = vec![
            "/usr/bin/ssh".into(), "-tt".into(),
            "-o".into(), "StrictHostKeyChecking=accept-new".into(),
            "-o".into(), "ServerAliveInterval=15".into(),
            "-o".into(), "ServerAliveCountMax=3".into(),
            "-o".into(), "ConnectTimeout=10".into(),
        ];
        if port != 22 { args.push("-p".into()); args.push(port.to_string()); }
        if let Some(ref k) = key_file { args.push("-i".into()); args.push(k.clone()); }
        args.push(format!("{}@{}", user, host));

        let use_password = auth == "password" && password.as_deref().unwrap_or("").len() > 0;
        let mut child = if use_password {
            let pw = password.unwrap_or_default();
            let escaped = pw
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace('`', "\\`")
                .replace('[', "\\[")
                .replace(']', "\\]");
            // spawn 参数逐个 shell_quote：主机/路径含空格或 shell 元字符时不会被 pty 拆错。
            // 密码发送只试一次：若又出现密码提示 = 密码错误，打 MAGIC_AUTH_FAILED 标记退出，
            // 让后端能确定性地识别认证失败（而不是傻傻显示"已连接"）。
            let script = format!(
                "#!/usr/bin/expect -f\nset timeout 20\nspawn {}\nexpect {{\n  -re \"(?i)password:\\s*\" {{\n    send -- \"{}\\r\"\n    expect {{\n      -re \"(?i)password:\\s*\" {{ puts \"MAGIC_AUTH_FAILED\"; exit 3 }}\n      -re \"Permission denied\" {{ puts \"MAGIC_AUTH_FAILED\"; exit 3 }}\n      -re {{[#$] *$}} {{ }}\n      timeout {{ }}\n      eof {{ exit 1 }}\n    }}\n  }}\n  -re \"Are you sure.*\" {{ send \"yes\\r\"; exp_continue }}\n  timeout {{ puts \"MAGIC_AUTH_TIMEOUT\"; exit 4 }}\n  eof {{ exit 1 }}\n}}\ninteract\n",
                args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" "), escaped
            );
            // 明文密码永不落盘：把脚本通过 stdin 喂给 expect（`expect -f -`）。
            // 实测 expect -f - 是流式执行（读到完整命令即跑，不等 stdin EOF），
            // stdin 保持打开供后续 interact 输入，正好兼任终端输入通道。
            let mut s = Command::new("/usr/bin/expect");
            s.arg("-f").arg("-");
            s.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut spawned = match s.spawn() {
                Ok(c) => c,
                Err(e) => {
                    if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                    return Err(format!("启动 SSH 失败: {e}"));
                }
            };
            {
                let sin = spawned.stdin.as_mut();
                if let Some(sin) = sin {
                    let _ = sin.write_all(script.as_bytes());
                }
            }
            spawned
        } else {
            let mut c = Command::new(&args[0]);
            c.args(&args[1..]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            match c.spawn() {
                Ok(ch) => ch,
                Err(e) => {
                    if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                    return Err(format!("启动 SSH 失败: {e}"));
                }
            }
        };

        let stdin = child.stdin.take().ok_or("无法获取 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取 stdout")?;
        let stderr = child.stderr.take();

        let buf = self.buffer.clone();
        {
            let mut b = buf.lock().unwrap();
            b.clear();
        }
        // stdout 与 stderr 都汇入同一 buffer：确定性错误指纹（Permission denied 等）走 stderr
        spawn_drain(stdout, buf.clone());
        if let Some(e) = stderr {
            spawn_drain(e, buf.clone());
        }

        // ── 认证结果验证（最多 12 秒）──
        // 不再 spawn 成功即报"已连接"：轮询输出指纹，确定性失败立即报错并杀掉进程；
        // 错误密码/私钥因此不会进 Keychain（本函数 Err 时调用方不写 Keychain）。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
        loop {
            let text = String::from_utf8_lossy(&buf.lock().unwrap()).to_string();
            if let Some(msg) = ssh_failure_reason(&text) {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                return Err(msg);
            }
            if ssh_login_confirmed(&text) {
                break;
            }
            if let Ok(Some(_)) = child.try_wait() {
                let _ = child.wait();
                if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                let tail: String = text.chars().rev().take(200).collect::<String>().chars().rev().collect();
                return Err(if text.trim().is_empty() {
                    "SSH 进程已退出，未建立会话".to_string()
                } else {
                    format!("SSH 进程已退出：{}", tail.trim())
                });
            }
            if std::time::Instant::now() >= deadline {
                // 软通过：有输出且无失败指纹（自定义提示符没有特征可匹配），视为已连接；
                // 完全无输出才是真超时（所有真实失败路径——密码错/密钥拒/不可达——都有显式指纹）。
                if text.trim().is_empty() {
                    let _ = child.kill();
                    let _ = child.wait();
                    if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                    return Err("SSH 连接超时（12 秒内无任何输出）".to_string());
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // 认证通过后 ssh 已读过私钥，临时文件即可删除
        if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }

        *self.child.lock().unwrap() = Some(child);
        *self.stdin.lock().unwrap() = Some(stdin);

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
        if let Err(e) = s.write_all(&data) {
            // stdin 写失败通常意味着子进程已退出（网络中断/服务器重启）。
            // 必须清理 child/session 状态，否则前端表现为"卡住"：poll 读空 buffer，
            // 看不到任何错误，连接状态一直显示"已连接"，用户以为还在连着。
            // 先释放 stdin 锁再锁 child，避免与 disconnect 的 child→stdin 锁顺序
            // 反转导致死锁（write 失败时与前端 disconnect 并发会互等永久挂死）。
            drop(stdin);
            let mut child = self.child.lock().unwrap();
            if let Some(mut c) = child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            *self.session.lock().unwrap() = None;
            return Err(format!("SSH 写入失败（连接可能已断开）: {e}"));
        }
        Ok(())
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
        // 私钥解析：显式路径优先；否则回退 Keychain 中保存的私钥内容
        // （落成 0600 临时文件，用完即删）。修复：此前只认 key_path，
        // 粘贴私钥内容保存的服务器（路径为 None）探测/执行永远失败。
        let key_opt = key_path.filter(|k| !k.trim().is_empty());
        let (key_file, key_tmp) = Self::resolve_key_file(&host, &user, &auth, &key_opt);
        if let Some(kf) = &key_file {
            args.push("-i".to_string());
            args.push(kf.clone());
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
            // expect 脚本：等 password 提示后送密码，命令跑完自然 eof。
            // 关键修复：`expect eof` 后用 `wait` 取回 spawn 子进程的真实退出码并 `exit $rc`，
            // 否则 expect 进程恒以 0 退出，远端命令失败也报成功，server_metrics 拿空输出却 probe_ok。
            let script = format!(
                "#!/usr/bin/expect -f\nset timeout {}\nspawn {}\nexpect {{\n  -re \"(?i)password:\\s*\" {{ send \"{}\\r\" }}\n  -re \"Are you sure.*\" {{ send \"yes\\r\"; exp_continue }}\n  eof {{ exit 1 }}\n}}\nexpect eof\nset rc [lindex [wait] 3]\nexit $rc\n",
                timeout_secs,
                args.iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" "),
                escaped
            );
            let mut s = Command::new("/usr/bin/expect");
            s.arg("-f").arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut spawned = match s.spawn() {
                Ok(c) => c,
                Err(e) => {
                    if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                    return Err(format!("启动 SSH 失败: {e}"));
                }
            };
            if let Some(sin) = spawned.stdin.as_mut() {
                use std::io::Write;
                let _ = sin.write_all(script.as_bytes());
            }
            // 关键：try_wait 轮询期间必须排空 stdout/stderr 管道，
            // 否则远端输出超过 64KB（macOS 管道缓冲）后子进程写阻塞、永不退出、最终被超时 kill。
            // 用 drain 线程持续读取管道（与 connect 函数同样的模式）。
            let stdout_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
            let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
            let drain_out = spawned.stdout.take().map(|p| spawn_drain(p, stdout_buf.clone()));
            let drain_err = spawned.stderr.take().map(|p| spawn_drain(p, stderr_buf.clone()));
            // wait_with_output 不设超时会永久挂起（SSH 卡住 / 密码错误等待重试）。
            // 用 try_wait 轮询 + 超时 kill，配合 expect 脚本内 `set timeout`，避免挂死 Tauri 命令线程。
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_secs(timeout_secs + 10);
            let output: std::process::Output;
            loop {
                match spawned.try_wait() {
                    Ok(Some(status)) => {
                        // join drain 线程确保所有缓冲输出已收集完毕
                        if let Some(h) = drain_out { let _ = h.join(); }
                        if let Some(h) = drain_err { let _ = h.join(); }
                        output = std::process::Output {
                            status,
                            stdout: stdout_buf.lock().unwrap().clone(),
                            stderr: stderr_buf.lock().unwrap().clone(),
                        };
                        break;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                        return Err(e.to_string());
                    }
                }
                if std::time::Instant::now() >= deadline {
                    let _ = spawned.kill();
                    let _ = spawned.wait();
                    if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                    return Err(format!("SSH 执行超时（>{}s）", timeout_secs));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            output
        } else {
            // 密钥认证：直接跑，走已配置的 ssh key。
            // 不能用 .output()——它会永久阻塞直到进程结束，若 SSH 命令卡住
            // （网络中断但 TCP keepalive 未触发、远端命令等待输入等），
            // Tauri 命令线程会被永久挂起。改为 spawn + try_wait 轮询 + 超时 kill，
            // 与密码认证分支一致。
            let mut spawned = match Command::new("/usr/bin/ssh")
                .args(&args[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                    return Err(format!("启动 SSH 失败: {e}"));
                }
            };
            let stdout_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
            let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
            let drain_out = spawned.stdout.take().map(|p| spawn_drain(p, stdout_buf.clone()));
            let drain_err = spawned.stderr.take().map(|p| spawn_drain(p, stderr_buf.clone()));
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_secs(timeout_secs + 10);
            loop {
                match spawned.try_wait() {
                    Ok(Some(status)) => {
                        if let Some(h) = drain_out { let _ = h.join(); }
                        if let Some(h) = drain_err { let _ = h.join(); }
                        let output = std::process::Output {
                            status,
                            stdout: stdout_buf.lock().unwrap().clone(),
                            stderr: stderr_buf.lock().unwrap().clone(),
                        };
                        break output;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                        return Err(e.to_string());
                    }
                }
                if std::time::Instant::now() >= deadline {
                    let _ = spawned.kill();
                    let _ = spawned.wait();
                    if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }
                    return Err(format!("SSH 执行超时（>{}s）", timeout_secs));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        };
        // 命令已结束，临时私钥文件立即删除
        if let Some(p) = &key_tmp { let _ = std::fs::remove_file(p); }

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

/// 把 Keychain 里的私钥内容落成 0600 临时文件供 `ssh -i` 使用。
/// 文件在 $TMPDIR（macOS 上为每用户私有目录），用毕必须 remove_file。
fn write_temp_key(content: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!("magic-ssh-key-{}-{}", std::process::id(), nanos));
    std::fs::write(&p, content).map_err(|e| format!("写临时私钥失败: {e}"))?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("设置临时私钥权限失败: {e}"))?;
    Ok(p)
}

/// 后台线程：把管道输出持续汇入共享 buffer（stdout/stderr 共用）
fn spawn_drain<R: Read + Send + 'static>(mut r: R, buf: Arc<Mutex<Vec<u8>>>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut tmp = [0u8; 4096];
        loop {
            match r.read(&mut tmp) {
                Ok(n) if n > 0 => {
                    if let Ok(mut b) = buf.lock() {
                        b.extend_from_slice(&tmp[..n]);
                    }
                }
                _ => break,
            }
        }
    })
}

/// 从 SSH 输出里识别确定性失败指纹，命中即返回用户可读的失败原因
fn ssh_failure_reason(text: &str) -> Option<String> {
    let checks: &[(&str, &str)] = &[
        ("MAGIC_AUTH_FAILED", "认证失败：密码错误（服务器拒绝了密码）"),
        ("MAGIC_AUTH_TIMEOUT", "认证超时：服务器 20 秒内未出现密码提示"),
        ("Permission denied", "认证失败：密码或私钥被服务器拒绝"),
        ("Enter passphrase for key", "私钥带密码短语（passphrase），当前不支持"),
        ("Connection refused", "连接被拒：目标端口未开放 SSH 服务"),
        ("Operation timed out", "连接超时：网络不可达或端口被拦"),
        ("Connection timed out", "连接超时：网络不可达或端口被拦"),
        ("No route to host", "无路由到目标主机"),
        ("Could not resolve hostname", "无法解析主机名"),
        ("nodename nor servname provided", "无法解析主机名"),
        ("Host key verification failed", "主机密钥校验失败"),
        ("no matching host key type", "主机密钥算法不兼容"),
        ("no matching cipher", "加密算法协商失败"),
    ];
    for (pat, msg) in checks {
        if text.contains(pat) {
            return Some(msg.to_string());
        }
    }
    None
}

/// 识别登录成功：出现登录横幅或 shell 提示符
fn ssh_login_confirmed(text: &str) -> bool {
    if text.contains("Last login:") || text.contains("Welcome to") {
        return true;
    }
    // 末行以典型 shell 提示符结尾：root@host:~#  [user@host ~]$
    // 不收 `>` 和 `%`：错误信息、欢迎横幅常含箭头/百分号，会让错误密码/私钥的会话
    // 被误判为「已连接」，进而把错误密码写进 Keychain 污染后续连接。
    // 调用方在调用本函数前已先用 ssh_failure_reason 过滤失败指纹，因此此处
    // 命中 # 或 $ 即可视为登录成功。
    matches!(text.trim_end().chars().last(), Some('#') | Some('$'))
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
