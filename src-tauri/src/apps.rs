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
    /// 用户是否确认过该软件的规则（未确认的软件不进入规则表，由兜底直连接管）
    pub confirmed: bool,
    /// mode=proxy 时指定的节点名；None 表示用当前选中节点
    pub node: Option<String>,
    /// 规则匹配用的路径前缀列表：该 App 全部进程（主程序+Helper+子进程）都命中这些前缀
    pub rule_paths: Vec<String>,
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
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
    ];
    if let Some(home_apps) = dirs::home_dir().map(|h| h.join("Applications")) {
        roots.push(home_apps);
    }
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(&root) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("app") { continue; }
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            if !seen.insert(name.clone()) { continue; }
            // 可执行文件通常是 Contents/MacOS/<AppName>，也可能是 Info.plist 里的 CFBundleExecutable
            let exe = p.join("Contents/MacOS").join(&name);
            let path = if exe.exists() { exe.to_string_lossy().to_string() } else { p.to_string_lossy().to_string() };
            let bundle_id = read_bundle_id(&p);
            let id = format!("app-{}", name);
            apps.push(AppEntry {
                id,
                name: name.clone(),
                bundle_id: bundle_id.clone(),
                // path 指向主可执行文件（展示用）；rule_paths 是规则前缀（匹配整组进程用）
                path: Some(path),
                running: false,
                mode: "direct".to_string(),
                category: classify(&name),
                confirmed: false,
                node: None,
                rule_paths: rule_paths_for(&p, bundle_id.as_deref()),
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
        // 取到 .app 目录（形如 /Applications/xxx.app/）
        let app_dir = &comm[..pos + marker.len()];
        // 完整可执行文件路径（形如 /Applications/xxx.app/Contents/MacOS/xxx）
        let exe_path = comm.trim();
        let bundle_id = bundle_id_for_path(app_dir);
        // 用可执行文件路径去匹配 installed 列表
        let key = exe_path.to_string();
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

/// 生成该 App 的规则路径前缀列表。
/// 默认覆盖 App 包 Contents/ 下所有进程（主程序、Helper、Frameworks 里的子进程）。
/// 特例：Safari 的实际联网进程是系统级 XPC 服务 com.apple.WebKit.Networking，
/// 位于 /System/Library/Frameworks/WebKit.framework/ 下，不在 Safari.app 包内，必须单独纳入。
fn rule_paths_for(app_dir: &Path, bundle_id: Option<&str>) -> Vec<String> {
    let mut v = vec![format!("{}/Contents/", app_dir.to_string_lossy())];
    if bundle_id == Some("com.apple.Safari") {
        v.push("/System/Library/Frameworks/WebKit.framework/".to_string());
    }
    v
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
