mod apps;
mod config;
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
fn scan_apps() -> Vec<crate::apps::AppEntry> {
    crate::apps::scan_macos_apps()
}

#[tauri::command]
fn start_proxy(state: tauri::State<Arc<Mutex<AppState>>>, rules: Vec<String>, direct_apps: Vec<String>, proxy_apps: Vec<String>) -> Result<MihomoStatus, String> {
    let g = state.lock().unwrap();
    g.mihomo.start(&g.config, &rules, &direct_apps, &proxy_apps)
}

#[tauri::command]
fn stop_proxy(state: tauri::State<Arc<Mutex<AppState>>>) -> Result<(), String> {
    let g = state.lock().unwrap();
    g.mihomo.stop();
    system_proxy::set_system_proxy(false, 7890).map(|_| ())
}

#[tauri::command]
fn set_system_proxy(state: tauri::State<Arc<Mutex<AppState>>>, enabled: bool) -> Result<crate::system_proxy::SystemProxyStatus, String> {
    let g = state.lock().unwrap();
    system_proxy::set_system_proxy(enabled, g.mihomo.port)
}

#[tauri::command]
fn ssh_connect(state: tauri::State<Arc<Mutex<AppState>>>, host: String, port: u16, user: String, auth: String, password: Option<String>, key: Option<String>) -> Result<crate::ssh::SshSession, String> {
    let g = state.lock().unwrap();
    g.ssh.connect(host, port, user, auth, password, key)
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
            ssh_disconnect
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
