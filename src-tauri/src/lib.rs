mod apps;
mod config;
mod keychain;
mod mihomo;
mod ssh;
mod system_proxy;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::config::AppConfig;
use crate::mihomo::{MihomoManager, MihomoStatus};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub proxy_running: bool,
    pub proxy_pid: Option<u32>,
    pub proxy_port: u16,
    pub system_proxy: bool,
    pub apps_count: usize,
    pub nodes_count: usize,
    pub ssh: Option<crate::ssh::SshSession>,
}

/// 代理端口与现有 Clash/FlClash 冲突检测结果
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub has_conflict: bool,
    pub messages: Vec<String>,
}

#[tauri::command]
fn check_conflicts() -> ConflictInfo {
    let mut messages = Vec::new();
    // 本程序自己启动的 mihomo 会带 runtime 目录参数，遇到时跳过，避免误报自身
    let runtime = MihomoManager::new().runtime_dir;
    let runtime_str = runtime.to_string_lossy().to_string();
    // 1) 检测正在运行的第三方代理程序（FlClash / Clash / 外部 mihomo）
    if let Ok(ps) = std::process::Command::new("/bin/ps").args(["-axo", "args="]).output() {
        let text = String::from_utf8_lossy(&ps.stdout);
        let mut foreign = false;
        for line in text.lines() {
            let lower = line.to_lowercase();
            let hit = lower.contains("mihomo") || lower.contains("clash") || lower.contains("flclash");
            if !hit { continue; }
            // 跳过本程序 runtime 目录启动的 mihomo
            if line.contains(&runtime_str) {
                continue;
            }
            foreign = true;
        }
        if foreign {
            messages.push("检测到正在运行的第三方代理程序（FlClash/Clash/mihomo），请先关闭".to_string());
        }
    }
    // 2) 检测本程序要用的混合端口是否已被占用
    let port = MihomoManager::new().port;
    if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
        messages.push(format!("端口 {} 已被其他程序占用，请先关闭冲突程序", port));
    }
    ConflictInfo { has_conflict: !messages.is_empty(), messages }
}

#[tauri::command]
fn fetch_subscription(url: String) -> Result<Vec<crate::config::ProxyNode>, String> {
    // 用系统 curl 拉取订阅内容
    let out = std::process::Command::new("/usr/bin/curl")
        .arg("-sL")
        .arg("--max-time").arg("15")
        .arg("-A").arg("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)")
        .arg(&url)
        .output()
        .map_err(|e| format!("调用 curl 失败: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("拉取订阅失败: {}", if err.is_empty() { "HTTP 错误".to_string() } else { err }));
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    crate::config::parse_vless_subscription(&text)
}

struct AppState {
    config: AppConfig,
    mihomo: MihomoManager,
    ssh: crate::ssh::SshManager,
}

#[tauri::command]
fn get_status(state: tauri::State<Arc<Mutex<AppState>>>) -> AppStatus {
    let g = state.lock().unwrap();
    let m = g.mihomo.status();
    AppStatus {
        proxy_running: m.running,
        proxy_pid: m.pid,
        proxy_port: m.port,
        system_proxy: system_proxy::status().enabled,
        apps_count: apps::scan_macos_apps().len(),
        nodes_count: g.config.nodes.len(),
        ssh: g.ssh.status(),
    }
}

#[tauri::command]
fn get_config(state: tauri::State<Arc<Mutex<AppState>>>) -> AppConfig {
    state.lock().unwrap().config.clone()
}

#[tauri::command]
fn save_config(state: tauri::State<Arc<Mutex<AppState>>>, config: AppConfig) -> Result<AppConfig, String> {
    let mut g = state.lock().unwrap();
    g.config = config.clone();
    config::save(&config)?;
    Ok(config)
}

#[tauri::command]
fn scan_apps(state: tauri::State<Arc<Mutex<AppState>>>) -> Vec<crate::apps::AppEntry> {
    let mut list = crate::apps::scan_macos_apps();
    let g = state.lock().unwrap();
    for app in list.iter_mut() {
        if let Some(setting) = g.config.apps.iter().find(|s| s.id == app.id) {
            app.mode = setting.mode.clone();
        }
    }
    list
}

/// 生成生效的软件分流规则：只处理用户已确认（confirmed）的条目。
/// 返回 (路径前缀列表, 目标) 列表；目标 = DIRECT | NODE-<节点名> | PROXY(当前选中节点)。
fn effective_app_rules(config: &AppConfig) -> Vec<(Vec<String>, String)> {
    let apps = crate::apps::scan_macos_apps();
    let mut by_id = std::collections::HashMap::new();
    for a in apps {
        by_id.insert(a.id, a.rule_paths);
    }
    let mut out = Vec::new();
    for setting in &config.apps {
        if !setting.confirmed {
            continue; // 未确认的软件不进规则表，由 MATCH,DIRECT 兜底
        }
        let Some(paths) = by_id.get(&setting.id) else { continue };
        let target = if setting.mode == "proxy" {
            match &setting.node {
                Some(n) if !n.trim().is_empty() => format!("NODE-{}", n.trim()),
                _ => "PROXY".to_string(),
            }
        } else {
            "DIRECT".to_string()
        };
        out.push((paths.clone(), target));
    }
    out
}

#[tauri::command]
fn start_proxy(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<MihomoStatus, String> {
    let g = state.lock().unwrap();
    // 先停止自己可能还在运行的 mihomo，避免重启时端口检测误报自身
    g.mihomo.stop();
    let cfg = g.config.clone();
    let app_rules = effective_app_rules(&cfg);
    // 再检测第三方代理冲突：FlClash/mihomo 占用端口时直接提示，避免抢端口
    let conflict = check_conflicts();
    if conflict.has_conflict {
        return Err(conflict.messages.join("；"));
    }
    let status = g.mihomo.start(&cfg, &[], &app_rules)?;
    if cfg.system_proxy {
        let _ = system_proxy::set_system_proxy(true, g.mihomo.port);
    }
    Ok(status)
}

#[tauri::command]
fn stop_proxy(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<(), String> {
    let g = state.lock().unwrap();
    g.mihomo.stop();
    let _ = system_proxy::set_system_proxy(false, g.mihomo.port);
    Ok(())
}

#[tauri::command]
fn set_system_proxy(state: tauri::State<Arc<Mutex<AppState>>>, enabled: bool) -> Result<crate::system_proxy::SystemProxyStatus, String> {
    let g = state.lock().unwrap();
    system_proxy::set_system_proxy(enabled, g.mihomo.port)
}

#[tauri::command]
fn ssh_connect(state: tauri::State<Arc<Mutex<AppState>>>, host: String, port: u16, user: String, auth: String, password: Option<String>, key: Option<String>) -> Result<crate::ssh::SshSession, String> {
    let mut g = state.lock().unwrap();
    let session = g.ssh.connect(host.clone(), port, user.clone(), auth.clone(), password.clone(), key.clone())?;

    // 密码/私钥内容安全存入 Keychain，config 不落明文
    let password_saved = if auth == "password" {
        match password {
            Some(p) if !p.trim().is_empty() => {
                keychain::store(&crate::ssh::SshManager::password_account(&host, &user), &p)?;
                true
            }
            _ => keychain::exists(&crate::ssh::SshManager::password_account(&host, &user)),
        }
    } else {
        false
    };
    let private_key_saved = if auth == "key" {
        match key.as_ref() {
            Some(k) if !k.trim().is_empty() => {
                // 如果是路径，不存内容；如果是私钥内容（多行），存 Keychain
                if k.contains('\n') {
                    keychain::store(&crate::ssh::SshManager::key_account(&host, &user), k)?;
                    true
                } else {
                    false // 路径形式，直接引用路径
                }
            }
            _ => keychain::exists(&crate::ssh::SshManager::key_account(&host, &user)),
        }
    } else {
        false
    };

    // 更新 servers 列表
    let id = format!("ssh-{}@{}", user, host);
    let info = crate::config::ServerInfo {
        id: id.clone(),
        name: host.clone(),
        host: host.clone(),
        port,
        user: user.clone(),
        auth: auth.clone(),
        password_saved,
        private_key_saved,
        key_path: key.as_ref().filter(|k| !k.contains('\n')).cloned(),
    };
    if let Some(existing) = g.config.servers.iter_mut().find(|s| s.id == id) {
        *existing = info.clone();
    } else {
        g.config.servers.push(info);
    }
    g.config.active_server_id = Some(id);

    // 兼容旧字段（不再存明文密码）
    g.config.ssh_host = Some(host);
    g.config.ssh_port = Some(port);
    g.config.ssh_user = Some(user);
    g.config.ssh_auth = Some(auth);
    g.config.ssh_password = None;
    g.config.ssh_private_key = None;
    let _ = config::save(&g.config);
    Ok(session)
}

#[tauri::command]
fn select_ssh_server(state: tauri::State<Arc<Mutex<AppState>>>, server_id: String) -> Result<crate::config::ServerInfo, String> {
    let mut g = state.lock().unwrap();
    let server = g.config.servers.iter().find(|s| s.id == server_id)
        .cloned()
        .ok_or("服务器不存在")?;
    g.config.active_server_id = Some(server.id.clone());
    g.config.ssh_host = Some(server.host.clone());
    g.config.ssh_port = Some(server.port);
    g.config.ssh_user = Some(server.user.clone());
    g.config.ssh_auth = Some(server.auth.clone());
    g.config.ssh_password = None;
    g.config.ssh_private_key = server.key_path.clone();
    config::save(&g.config)?;
    Ok(server)
}

#[tauri::command]
fn delete_ssh_server(state: tauri::State<Arc<Mutex<AppState>>>, server_id: String) -> Result<(), String> {
    let mut g = state.lock().unwrap();
    let idx = g.config.servers.iter().position(|s| s.id == server_id).ok_or("服务器不存在")?;
    let server = g.config.servers.remove(idx);
    keychain::delete(&crate::ssh::SshManager::password_account(&server.host, &server.user));
    keychain::delete(&crate::ssh::SshManager::key_account(&server.host, &server.user));
    if g.config.active_server_id.as_deref() == Some(server.id.as_str()) {
        if let Some(next) = g.config.servers.first().cloned() {
            g.config.active_server_id = Some(next.id.clone());
            g.config.ssh_host = Some(next.host);
            g.config.ssh_port = Some(next.port);
            g.config.ssh_user = Some(next.user);
            g.config.ssh_auth = Some(next.auth);
            g.config.ssh_password = None;
            g.config.ssh_private_key = next.key_path;
        } else {
            g.config.active_server_id = None;
            g.config.ssh_host = None;
            g.config.ssh_port = Some(22);
            g.config.ssh_user = Some("root".to_string());
            g.config.ssh_auth = Some("password".to_string());
            g.config.ssh_password = None;
            g.config.ssh_private_key = None;
        }
    }
    config::save(&g.config)?;
    Ok(())
}

#[tauri::command]
fn ssh_write(state: tauri::State<Arc<Mutex<AppState>>>, data: Vec<u8>) -> Result<(), String> {
    let g = state.lock().unwrap();
    g.ssh.write(data)
}

#[tauri::command]
fn ssh_read(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<Vec<u8>, String> {
    let g = state.lock().unwrap();
    g.ssh.read()
}

#[tauri::command]
fn ssh_disconnect(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<(), String> {
    let g = state.lock().unwrap();
    g.ssh.disconnect();
    Ok(())
}

#[tauri::command]
fn self_test(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<String, String> {
    let g = state.lock().unwrap();
    let bin = g.mihomo.bin_path_for_test();
    let geo = g.mihomo.geo_dir_for_test();
    let mut lines = Vec::new();
    lines.push(format!("mihomo_bin: {}", bin.display()));
    lines.push(format!("mihomo_exists: {}", bin.exists()));
    lines.push(format!("geo_dir: {}", geo.display()));
    lines.push(format!("geo_dir_exists: {}", geo.exists()));
    Ok(lines.join("\n"))
}


pub fn start_proxy_standalone() -> Result<MihomoStatus, String> {
    let cfg = config::load();
    let mihomo = MihomoManager::new();
    let app_rules = effective_app_rules(&cfg);
    let status = mihomo.start(&cfg, &[], &app_rules)?;
    if cfg.system_proxy {
        let _ = system_proxy::set_system_proxy(true, mihomo.port);
    }
    Ok(status)
}

pub fn stop_proxy_standalone() -> Result<(), String> {
    // start_proxy_standalone 与 stop_proxy_standalone 各自创建实例无法共享 child，
    // 这里改为按启动参数（runtime 下的 mihomo.yaml）精确查找并结束 mihomo 进程。
    let runtime = MihomoManager::new().runtime_dir;
    let conf_path = runtime.join("mihomo.yaml");
    let conf_str = conf_path.to_string_lossy().to_string();
    if let Ok(out) = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,args="])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("mihomo") && line.contains(&conf_str) {
                if let Some(pid_str) = line.split_whitespace().next() {
                    if let Ok(pid) = pid_str.parse::<i32>() {
                        let _ = std::process::Command::new("/bin/kill")
                            .arg(pid.to_string())
                            .output();
                    }
                }
            }
        }
    }
    let _ = system_proxy::set_system_proxy(false, 7891);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(Mutex::new(AppState {
        config: config::load(),
        mihomo: MihomoManager::new(),
        ssh: crate::ssh::SshManager::new(),
    }));
    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_config,
            save_config,
            scan_apps,
            start_proxy,
            stop_proxy,
            set_system_proxy,
            ssh_connect,
            ssh_write,
            ssh_read,
            ssh_disconnect,
            select_ssh_server,
            delete_ssh_server,
            self_test,
            check_conflicts,
            fetch_subscription
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
