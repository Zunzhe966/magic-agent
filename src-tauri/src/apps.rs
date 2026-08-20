use serde::{Deserialize, Serialize};
use std::path::Path;

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
    let mut apps = Vec::new();
    let roots = [
        "/Applications",
        &format!("{}/Applications", std::env::var("HOME").unwrap_or_default()),
    ];
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("app") {
                continue;
            }
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            if !seen.insert(name.clone()) {
                continue;
            }
            let exe = p.join("Contents/MacOS").join(&name);
            let path = exe.to_string_lossy().to_string();
            let bundle_id = read_bundle_id(&p);
            let id = format!("app-{}", name);
            apps.push(AppEntry {
                id,
                name: name.clone(),
                bundle_id,
                path: Some(path),
                running: false,
                mode: "auto".to_string(),
                category: classify(&name),
                rule_path: format!("/Applications/{}.app/", name),
            });
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps
}

fn read_bundle_id(app: &Path) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    let data = std::fs::read(&plist).ok()?;
    // 简单文本扫描 CFBundleIdentifier，避免引入 plist 依赖
    let text = String::from_utf8_lossy(&data);
    let key = "<key>CFBundleIdentifier</key>";
    if let Some(pos) = text.find(key) {
        let rest = &text[pos + key.len()..];
        if let Some(start) = rest.find("<string>") {
            let after = &rest[start + 8..];
            if let Some(end) = after.find("</string>") {
                return Some(after[..end].to_string());
            }
        }
    }
    None
}

pub fn classify(name: &str) -> String {
    let n = name.to_lowercase();
    if n.contains("chrome") || n.contains("safari") || n.contains("edge") || n.contains("firefox") || n.contains("doubao") || n.contains("browser") {
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

