//! macOS Keychain 封装（通过系统 security 命令，零外部依赖）
//!
//! 用于安全存储 SSH 密码/私钥，避免明文写入 config.json。

use std::process::Command;

const SERVICE: &str = "com.magic.agent";

/// 把 secret 存入 Keychain，返回对应的 account 名。
/// secret 通过 stdin 传给 security（-w 不带值），避免明文出现在进程 argv 被 ps 看到。
pub fn store(account: &str, secret: &str) -> Result<String, String> {
    use std::io::Write;
    let mut child = Command::new("/usr/bin/security")
        .args(["add-generic-password", "-s", SERVICE, "-a", account, "-w", "-U"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("调用 security 失败: {e}"))?;
    {
        let stdin = child.stdin.as_mut().ok_or("无法打开 security stdin")?;
        stdin.write_all(secret.as_bytes()).map_err(|e| format!("写入 secret 失败: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("等待 security 失败: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("写入 Keychain 失败: {err}"));
    }
    Ok(account.to_string())
}

/// 从 Keychain 读取 secret。
pub fn get(account: &str) -> Result<String, String> {
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", account, "-w"])
        .output()
        .map_err(|e| format!("调用 security 失败: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("从 Keychain 读取失败: {err}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end_matches(['\n', '\r']).to_string())
}

/// 从 Keychain 删除。
pub fn delete(account: &str) {
    let _ = Command::new("/usr/bin/security")
        .args(["delete-generic-password", "-s", SERVICE, "-a", account])
        .output();
}

/// 检查 Keychain 中是否存在该 account。
pub fn exists(account: &str) -> bool {
    get(account).is_ok()
}
