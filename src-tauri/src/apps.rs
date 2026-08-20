use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub bundle_id: Option<String>,
    pub path: Option<String>,
    pub running: bool,
    pub mode: String,
    pub category: String,
    pub rule_path: String,
}


pub fn scan_macos_apps() -> Vec<AppEntry> {
    let installed = scan_installed_apps();
    let running = scan_running_processes();
    let mut by_path: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, app) in installed.iter().enumerate() {
        if let Some(path) = &app.path {
            by_path.insert(path.clone(), i);
        }
        if let Some(b) = &app.bundle_id {
            by_path.insert(b.clone(), i);
        }
    }
    let mut list = installed;
    for proc in running {
        if let Some(&i) = by_path.get(&proc.app_path) {
            list[i].running = true;
        } else if let Some(&i) = by_path.get(&proc.bundle_id) {
            list[i].running = true;
        }
    }
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

fn scan_installed_apps() -> Vec<AppEntry> {
    let mut apps = Vec::new();
    let roots = [
        PathBuf::from("/Applications"),
        dirs::home_dir().map(|h| h.join("Applications")).unwrap_or_default(),
    ];
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("app") { continue; }
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            if !seen.insert(name.clone()) { continue; }
            let exe = p.join("Contents/MacOS").join(&name);
            let path = if exe.exists() { exe.to_string_lossy().to_string() } else { p.to_string_lossy().to_string() };
            let bundle_id = read_bundle_id(&p);
            let id = format!("app-{}", name);
            apps.push(AppEntry {
                id,
                name: name.clone(),
                bundle_id,
                path: Some(path.clone()),
                running: false,
                mode: "auto".to_string(),
                category: classify(&name),
                rule_path: format!("{}/", p.to_string_lossy()),
            });
        }
    }
    apps
}

struct RunningProcess {
    app_path: String,
    bundle_id: String,
}

fn scan_running_processes() -> Vec<RunningProcess> {
    let mut out = Vec::new();
    let ps = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,comm="])
        .output()
        .ok();
    let Some(ps) = ps else { return out };
    if !ps.status.success() { return out; }
    let text = String::from_utf8_lossy(&ps.stdout);
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((_, comm)) = line.split_once(' ') else { continue };
        let comm = comm.trim();
        if !comm.contains(".app/") { continue; }
        let marker = ".app/";
        let Some(pos) = comm.find(marker) else { continue };
        let app_path = &comm[..pos + marker.len()];
        let bundle_id = bundle_id_for_path(app_path);
        let key = app_path.to_string();
        if !seen.insert(key.clone()) { continue; }
        out.push(RunningProcess { app_path: key, bundle_id });
    }
    out
}

fn bundle_id_for_path(app_path: &str) -> String {
    let p = Path::new(app_path);
    let plist = p.join("Contents/Info.plist");
    let out = std::process::Command::new("/usr/bin/plutil")
        .arg("-extract").arg("CFBundleIdentifier").arg("raw").arg("-o").arg("-").arg(&plist)
        .output().ok();
    if let Some(o) = out {
        if o.status.success() {
            return String::from_utf8_lossy(&o.stdout).trim().to_string();
        }
    }
    String::new()
}
fn read_bundle_id(app: &Path) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    let out = std::process::Command::new("/usr/bin/plutil")
        .arg("-extract")
        .arg("CFBundleIdentifier")
        .arg("raw")
        .arg("-o")
        .arg("-")
        .arg(&plist)
        .output()
        .ok()?;
    if out.status.success() {
        return Some(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    None
}

pub fn classify(name: &str) -> String {
    let n = name.to_lowercase();
    if n.contains("chrome") || n.contains("safari") || n.contains("edge") || n.contains("firefox") || n.contains("browser") {
        "浏览器".to_string()
    } else if n.contains("wechat") || n.contains("weixin") || n.contains("qq") || n.contains("telegram") || n.contains("discord") || n.contains("slack") || n.contains("dingtalk") || n.contains("lark") || n.contains("feishu") || n.contains("whatsapp") {
        "通讯".to_string()
    } else if n.contains("meeting") || n.contains("zoom") || n.contains("tencent") || n.contains("会议") {
        "会议".to_string()
    } else if n.contains("terminal") || n.contains("iterm") || n.contains("ssh") || n.contains("code") || n.contains("studio") || n.contains("xcode") || n.contains("docker") || n.contains("vim") {
        "开发工具".to_string()
    } else if n.contains("chatgpt") || n.contains("claude") || n.contains("gemini") || n.contains("kimi") || n.contains("qianwen") || n.contains("doubao") || n.contains("ollama") {
        "AI 工具".to_string()
    } else {
        "其他".to_string()
    }
}
