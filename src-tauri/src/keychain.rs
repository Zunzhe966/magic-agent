//! macOS Keychain 封装（通过系统 security 命令，零外部依赖）
//!
//! 用于安全存储 SSH 密码/私钥，避免明文写入 config.json。

use std::process::Command;

const SERVICE: &str = "com.magic.agent";

/// 把 secret 存入 Keychain，返回对应的 account 名。
pub fn store(account: &str, secret: &str) -> Result<String, String> {
    let out = Command::new("/usr/bin/security")
        .args(["add-generic-password", "-s", SERVICE, "-a", account, "-w", secret, "-U"])
        .output()
        .map_err(|e| format!("调用 security 失败: {e}"))?;
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
    Ok(String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string())
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
