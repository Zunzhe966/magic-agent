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
    /// 是否有非回环的活跃网络连接（联网判断：本地软件/脚本不联网，无需分流）
    #[serde(default)]
    pub online: bool,
    /// 当前连接的远端 IP（去重，供前端展示与智能体二次分析国内/国外）
    #[serde(default)]
    pub remote_ips: Vec<String>,
}

pub fn scan_macos_apps() -> Vec<AppEntry> {
    let installed = scan_installed_apps();
    let running = scan_running_processes();
    // 一次性拿到全机网络连接：进程名/PID -> 远端 IP 集合
    let net = scan_network_connections();
    let mut list = installed;
    mark_running_apps(&mut list, &running);
    // 追加：命令行/脚本类进程（Python/Node/ffmpeg 等），它们不是 .app，但也要能分流
    list.extend(scan_script_processes());

    // 回填联网状态：按「进程名 basename」匹配网络连接，标记 online + remote_ips
    for app in list.iter_mut() {
        let mut ips: Vec<String> = Vec::new();
        // 该 App 的可执行文件 basename（用于和 lsof 的 COMMAND 名匹配）
        let exe_basename = app
            .path
            .as_deref()
            .and_then(|p| p.rsplit('/').next())
            .unwrap_or("")
            .to_string();
        // 也把 rule_paths 每个前缀的 basename 纳入匹配集（覆盖 .app 主程序 + Helper 场景）
        let mut candidates: Vec<String> = vec![app.name.clone(), exe_basename];
        for p in &app.rule_paths {
            if let Some(base) = p.trim_end_matches('/').rsplit('/').next() {
                candidates.push(base.to_string());
            }
        }
        for (proc_name, proc_ips) in net.iter() {
            let hit = candidates.iter().any(|c| {
                if c.is_empty() {
                    return false;
                }
                let c = c.to_lowercase();
                let p = proc_name.to_lowercase();
                process_name_matches(&p, &c)
            });
            if hit {
                for ip in proc_ips {
                    if !ips.contains(ip) {
                        ips.push(ip.clone());
                    }
                }
            }
        }
        app.online = !ips.is_empty();
        app.remote_ips = ips;
    }

    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// 把运行中的进程归属到安装列表。先按可执行文件路径，再按 bundle id；
/// bundle id 为空时不能误按空字符串去重，否则所有无 plist 的进程会被合成一个。
fn mark_running_apps(list: &mut [AppEntry], running: &[RunningProcess]) {
    use std::collections::HashMap;

    let mut by_path: HashMap<String, usize> = HashMap::new();
    let mut by_bundle: HashMap<String, usize> = HashMap::new();
    for (i, app) in list.iter().enumerate() {
        if let Some(path) = &app.path {
            by_path.insert(path.clone(), i);
            if let Some((dir, _)) = path.split_once("/Contents/") {
                by_path.insert(dir.to_string(), i);
            }
        }
        if let Some(b) = app.bundle_id.as_deref().filter(|b| !b.is_empty()) {
            by_bundle.insert(b.to_string(), i);
        }
    }
    for proc in running {
        if let Some(&i) = by_path.get(&proc.app_dir) {
            list[i].running = true;
        } else if let Some(&i) = by_path.get(&proc.app_path) {
            list[i].running = true;
        } else if !proc.bundle_id.is_empty() {
            if let Some(&i) = by_bundle.get(&proc.bundle_id) {
                list[i].running = true;
            }
        }
    }
}

/// 进程名匹配：判断 lsof 的进程名 p 是否属于候选名 c 所代表的软件。
/// 只允许「精确相等」或「边界匹配」（c 后紧跟空格/点/括号），
/// 拒绝裸前缀匹配，避免短名误伤——例如候选 "Go" 不应匹配 "Google Chrome"，
/// 候选 "Node" 不应匹配 "NodeRunner"。
/// 典型场景：c="Google Chrome" 应匹配 "Google Chrome Helper"、"Google Chrome Helper (Renderer)"。
fn process_name_matches(p: &str, c: &str) -> bool {
    if p == c {
        return true;
    }
    // p 必须以 c 开头，且 c 之后是"边界"字符（空格/点/括号/斜杠/冒号/数字）或已结束
    if let Some(rest) = p.strip_prefix(c) {
        // c 本身太短（<2 字符）不做前缀匹配，只允许精确相等，避免单字母误伤
        if c.chars().count() < 2 {
            return false;
        }
        let next = rest.chars().next();
        match next {
            None => true, // p == c 已在上方处理，这里不会到
            Some(ch) => {
                ch == ' '
                    || ch == '.'
                    || ch == '('
                    || ch == '/'
                    || ch == ':'
                    || ch == '-'
                    || ch == '_'
            }
        }
    } else {
        false
    }
}

/// 扫描全机活跃 TCP 连接，返回 进程名 -> 远端 IP 集合 的映射。
/// 只统计 ESTABLISHED 且远端非回环（回环不算"联网出网"）。
/// 用 lsof -nP -iTCP -sTCP:ESTABLISHED 拿「进程名 PID 远端IP」。
fn scan_network_connections() -> std::collections::HashMap<String, Vec<String>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    // lsof 扫全机 ESTABLISHED 连接：连接多时可能耗时 1-3 秒。
    // 加 2.5 秒硬超时——超时就放弃联网标记（online/remote_ips 降级为空），
    // 绝不因为 lsof 卡住而阻塞整个 App 扫描和 UI 交互。
    let mut child = match std::process::Command::new("/usr/sbin/lsof")
        .args(["-nP", "-iTCP", "-sTCP:ESTABLISHED"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return map,
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2500);
    let out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break child.wait_with_output().ok(),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return map; // 超时：放弃联网标记
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            Err(_) => {
                let _ = child.kill();
                return map;
            }
        }
    };
    let Some(out) = out else { return map };
    if !out.status.success() {
        return map;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines().skip(1) {
        // 形如：Electron  40763  user  ...  TCP 192.168.x:port->119.188.11.232:443
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 2 {
            continue;
        }
        let proc_name = cols[0].to_string();
        // 找 "->" 后面的远端地址
        let Some(arrow) = line.find("->") else {
            continue;
        };
        let remote = line[arrow + 2..].trim();
        // 提取远端 IP：
        //   - IPv4/IPv6 带方括号：[2408:8207::1]:443  -> 取方括号内
        //   - IPv4 明文：119.188.11.232:443 -> 取冒号前
        //   - IPv6 明文（无方括号）：2408:8207::1:443 -> 最后一个冒号是端口分隔，
        //     需保留完整 IPv6（不能像旧实现那样 split(':') 取首个，会截成 2408）
        let ip = if remote.starts_with('[') {
            remote[1..].split(']').next().unwrap_or("").to_string()
        } else {
            // 无方括号：若冒号数量 > 1 判定为 IPv6，去掉末尾 :port
            let colon_count = remote.matches(':').count();
            if colon_count > 1 {
                remote
                    .rsplit_once(':')
                    .map(|(ip, _)| ip.to_string())
                    .unwrap_or_else(|| remote.to_string())
            } else {
                // IPv4:port 或裸 IPv4
                remote.split([':', ' ']).next().unwrap_or("").to_string()
            }
        };
        if ip.is_empty() {
            continue;
        }
        // 排除回环
        if ip == "127.0.0.1"
            || ip == "::1"
            || ip == "localhost"
            || ip == "*"
            || ip == "0.0.0.0"
            || ip == "::"
        {
            continue;
        }
        let entry = map.entry(proc_name).or_insert_with(Vec::new);
        if !entry.contains(&ip) {
            entry.push(ip);
        }
    }
    map
}

/// 扫描运行中的"命令行/脚本类"进程，纳入分流。
/// 这些进程（python、node、ffmpeg、java 等）不在 .app 里，传统扫描会漏掉。
///
/// 关键：mihomo 的 PROCESS-PATH-REGEX 匹配的是「进程可执行文件的绝对路径」，
/// 不是 argv 里的脚本路径。对 Python 脚本而言，可执行文件是解释器（如 .venv/bin/python），
/// 不是 main.py。因此这里记录「可执行文件路径」（argv[0]），并同时用脚本所在项目目录
/// 来命名/分类。虚拟环境 .venv 就在项目目录里，所以 .venv/bin/python 的完整路径天然含项目目录，
/// 用它做 PROCESS-PATH-REGEX 能精确区分不同项目，而不会误伤系统 python。
fn scan_script_processes() -> Vec<AppEntry> {
    let mut out = Vec::new();
    let Ok(ps) = std::process::Command::new("/bin/ps")
        .args(["-axo", "args="])
        .output()
    else {
        return out;
    };
    if !ps.status.success() {
        return out;
    }
    let text = String::from_utf8_lossy(&ps.stdout);

    // 需要识别为"脚本进程"的解释器/命令
    let interpreters = [
        "python", "python3", "node", "ffmpeg", "java", "ruby", "perl", "go", "deno", "bun", "php",
    ];

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // argv[0] = 可执行文件路径
        let exe_path = line.split_whitespace().next().unwrap_or("");
        if exe_path.is_empty() || !exe_path.contains('/') {
            continue;
        }
        let exe_name = exe_path.rsplit('/').next().unwrap_or(exe_path);
        // 是否属于脚本解释器进程
        let is_interp = interpreters
            .iter()
            .any(|i| exe_name == *i || exe_name.starts_with(&format!("{}.", i)));
        if !is_interp {
            continue;
        }
        // 从 argv 里推断脚本所在项目目录（用于命名/分类，不用于规则匹配）
        let proj_dir = infer_script_project_dir(line)
            .or_else(|| exe_path.rsplit_once('/').map(|(d, _)| d.to_string()));
        // 去重键：解释器路径 + 项目目录。多个项目共用同一解释器（如都直接用
        // 系统 /usr/bin/python3 而非各自 .venv）时，仅按 exe_path 去重会把第二个
        // 项目整个漏掉。用 (exe_path, proj_dir) 组合才能精确区分不同项目。
        let dedup_key = format!("{}|{}", exe_path, proj_dir.as_deref().unwrap_or(""));
        if !seen.insert(dedup_key) {
            continue; // 同一解释器跑同一项目只记一次（如多 worker 进程）
        }
        let name = proj_dir
            .as_deref()
            .and_then(|d| d.rsplit('/').next())
            .filter(|s| !s.is_empty())
            .map(|s| format!("{}·{}", s, exe_name))
            .unwrap_or_else(|| exe_name.to_string());
        // id 用 bin-<可执行文件路径>，settings_to_app_rules 里 bin- 前缀直接用该路径
        // 生成 PROCESS-PATH-REGEX,^<路径>，匹配进程可执行文件路径。
        out.push(AppEntry {
            id: format!("bin-{}", exe_path),
            name: format!("脚本·{}", name),
            bundle_id: None,
            path: Some(exe_path.to_string()),
            running: true,
            mode: "direct".to_string(),
            category: "脚本/命令行".to_string(),
            confirmed: false,
            node: None,
            rule_paths: vec![exe_path.to_string()],
            online: false,
            remote_ips: Vec::new(),
        });
    }
    out
}

/// 从命令行 argv 推断"项目目录"。
/// 策略：优先找 argv 里含 .py/.js/.ts/.mjs/.cjs 等脚本后缀的参数，取其所在目录；
/// 否则退回第一个含路径分隔符且非解释器自身的参数所在目录。
///
/// 用 shell 引号规则正确切分 argv（而非 split_whitespace），这样含空格的路径
/// （如 /Users/x/example monitor/main.py）不会被空格拆散导致识别失败。
fn infer_script_project_dir(line: &str) -> Option<String> {
    let tokens = shell_split_argv(line);
    let script_exts = [
        ".py", ".js", ".mjs", ".cjs", ".ts", ".rb", ".pl", ".php", ".go",
    ];
    // 1) 找脚本文件参数
    for t in &tokens {
        if t.starts_with('-') {
            continue;
        }
        if !t.contains('/') {
            continue;
        }
        let lower = t.to_lowercase();
        if script_exts.iter().any(|e| lower.ends_with(e)) {
            if let Some(dir) = t.rsplit_once('/').map(|(d, _)| d.to_string()) {
                if !dir.is_empty() {
                    return Some(dir);
                }
            }
        }
    }
    // 2) 退回：第一个含路径分隔符的目录参数
    for t in &tokens {
        if t.contains('/') && !t.contains('=') && !t.starts_with('-') {
            if let Some(dir) = t.rsplit_once('/').map(|(d, _)| d.to_string()) {
                if !dir.is_empty() && dir != "/" {
                    return Some(dir);
                }
            }
        }
    }
    None
}

/// 按 shell 引号规则切分命令行字符串为 argv。
/// 支持单引号（字面）、双引号（字面，简化处理不展开 $ 变量）与反斜杠转义，
/// 正确处理含空格、含引号的路径。返回的每个 token 已去掉包裹引号。
fn shell_split_argv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut cur_has_content = false;

    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                cur.push(c);
            }
            continue;
        }
        if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                // 双引号内反斜杠只对 " \ $ 等生效，这里简化：保留下一个字符
                if let Some(&n) = chars.peek() {
                    cur.push(n);
                    chars.next();
                }
            } else {
                cur.push(c);
            }
            continue;
        }
        // 非引号状态
        match c {
            '\'' => {
                in_single = true;
                cur_has_content = true;
            }
            '"' => {
                in_double = true;
                cur_has_content = true;
            }
            '\\' => {
                if let Some(&n) = chars.peek() {
                    cur.push(n);
                    chars.next();
                    cur_has_content = true;
                }
            }
            ' ' | '\t' => {
                if cur_has_content || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    cur_has_content = false;
                }
            }
            c => {
                cur.push(c);
                cur_has_content = true;
            }
        }
    }
    if cur_has_content || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn scan_installed_apps() -> Vec<AppEntry> {
    let mut candidates = Vec::new();
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
        let Ok(rd) = std::fs::read_dir(&root) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("app") {
                continue;
            }
            let name = p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let canonical_path = p.canonicalize().unwrap_or_else(|_| p.clone());
            if !seen.insert(canonical_path.clone()) {
                continue;
            }
            // 可执行文件通常是 Contents/MacOS/<AppName>，也可能是 Info.plist 里的 CFBundleExecutable
            let exe = p.join("Contents/MacOS").join(&name);
            let path = if exe.exists() {
                exe.to_string_lossy().to_string()
            } else {
                p.to_string_lossy().to_string()
            };
            let bundle_id = read_bundle_id(&p);
            candidates.push((canonical_path, name, path, bundle_id));
        }
    }

    let ids = app_ids(&candidates);
    let mut apps = Vec::with_capacity(candidates.len());
    for ((canonical_path, name, path, bundle_id), id) in candidates.into_iter().zip(ids) {
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
            rule_paths: rule_paths_for(&canonical_path, bundle_id.as_deref()),
            online: false,
            remote_ips: Vec::new(),
        });
    }
    apps
}

/// App id 必须唯一，同时不能让已有配置全部失效。
/// 无重名时沿用历史 `app-<显示名>`；只有真正重名时，冲突项才追加稳定后缀。
fn app_ids(candidates: &[(PathBuf, String, String, Option<String>)]) -> Vec<String> {
    use std::collections::HashMap;

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (_, name, _, _) in candidates {
        *counts.entry(name.as_str()).or_default() += 1;
    }
    candidates
        .iter()
        .map(|(path, name, _, bundle)| {
            let base = format!("app-{name}");
            if counts.get(name.as_str()).copied().unwrap_or(0) <= 1 {
                return base;
            }
            if let Some(b) = bundle.as_deref().filter(|b| !b.is_empty()) {
                return format!("{base}--{b}");
            }
            let normalized = path
                .to_string_lossy()
                .trim_end_matches('/')
                .trim_start_matches('/')
                .replace('/', ":");
            format!("{base}--{normalized}")
        })
        .collect()
}

struct RunningProcess {
    app_dir: String,
    app_path: String,
    bundle_id: String,
}

fn scan_running_processes() -> Vec<RunningProcess> {
    let mut out = Vec::new();
    // 用 args= 而非 comm=：comm 是进程名（不含路径、截断），永远匹配不到 .app/ 路径，
    // 导致"运行中"状态检测失效。args 含完整可执行文件路径。
    let ps = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,comm=,args="])
        .output()
        .ok();
    let Some(ps) = ps else { return out };
    if !ps.status.success() {
        return out;
    }
    let text = String::from_utf8_lossy(&ps.stdout);
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let line = line.trim_start();
        // ps -axo pid=,comm=,args= 形如
        // "12345 /Applications/xxx.app/Contents/MacOS/xxx /Applications/xxx.app/Contents/MacOS/xxx --flag"
        let Some((pid, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if !pid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let rest = rest.trim_start();
        let Some((comm, args)) = rest.split_once(char::is_whitespace) else {
            continue;
        };
        if !args.contains(".app/") {
            continue;
        }
        let marker = ".app/";
        let Some(pos) = args.find(marker) else {
            continue;
        };
        // 取到 .app 目录（形如 /Applications/xxx.app/）
        let app_dir = &args[..pos + marker.len()];
        let bundle_id = bundle_id_for_path(app_dir);
        // comm 在 macOS 上按空格截断，不能当完整路径。app_dir 才是可靠身份；
        // app_path 只用于无 bundle id 时的兜底展示/匹配。
        // 先从完整 args 提取，comm 在 macOS 上常被截断到首个空格之前
        // （例如 "Google Chrome" 只留下 "Google"）。bundle id 缺失时
        // app_path 是唯一去重/匹配键，不能让截断值污染它。
        let app_path = exact_process_path(args, app_dir)
            .or_else(|| exact_process_path(comm, app_dir))
            .unwrap_or_else(|| comm.trim().to_string());
        // 去重键优先用 bundle_id（更稳定，不受 args 空格截断影响）；
        // bundle_id 为空（极少见，Info.plist 读不到）才退回 app_path。
        // 此前用 app_path 去重，含空格的路径被截断到首个 token，
        // 多个不同进程首 token 相同会被误并为一个，丢失真实运行进程。
        let dedup_key = if !bundle_id.is_empty() {
            bundle_id.clone()
        } else {
            app_path.clone()
        };
        if !seen.insert(dedup_key) {
            continue;
        }
        out.push(RunningProcess {
            app_dir: app_dir.trim_end_matches('/').to_string(),
            app_path,
            bundle_id,
        });
    }
    out
}

/// 从 `ps args=` 行中提取 .app 主可执行文件的精确路径。
/// ps 不引用含空格路径，但 `.app/Contents/MacOS/` 这个稳定分界能确定
/// 可执行文件在 `.app/` 后的第一段；超出分界的部分是参数，不是路径。
fn exact_process_path(args: &str, app_dir: &str) -> Option<String> {
    let (_, after_app) = args.split_once(app_dir)?;
    let executable = after_app.split_whitespace().next()?;
    if executable.is_empty() {
        return None;
    }
    Some(format!("{app_dir}{executable}"))
}

fn bundle_id_for_path(app_path: &str) -> String {
    let p = Path::new(app_path);
    let plist = p.join("Contents/Info.plist");
    let out = std::process::Command::new("/usr/bin/plutil")
        .arg("-extract")
        .arg("CFBundleIdentifier")
        .arg("raw")
        .arg("-o")
        .arg("-")
        .arg(&plist)
        .output()
        .ok();
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
    if n.contains("chrome")
        || n.contains("safari")
        || n.contains("edge")
        || n.contains("firefox")
        || n.contains("browser")
    {
        "浏览器".to_string()
    } else if n.contains("wechat")
        || n.contains("weixin")
        || n.contains("qq")
        || n.contains("telegram")
        || n.contains("discord")
        || n.contains("slack")
        || n.contains("dingtalk")
        || n.contains("lark")
        || n.contains("feishu")
        || n.contains("whatsapp")
    {
        "通讯".to_string()
    } else if n.contains("meeting")
        || n.contains("zoom")
        || n.contains("tencent")
        || n.contains("会议")
    {
        "会议".to_string()
    } else if n.contains("terminal")
        || n.contains("iterm")
        || n.contains("ssh")
        || n.contains("code")
        || n.contains("studio")
        || n.contains("xcode")
        || n.contains("docker")
        || n.contains("vim")
    {
        "开发工具".to_string()
    } else if n.contains("chatgpt")
        || n.contains("claude")
        || n.contains("gemini")
        || n.contains("kimi")
        || n.contains("qianwen")
        || n.contains("doubao")
        || n.contains("ollama")
    {
        "AI 工具".to_string()
    } else {
        "其他".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_script_dir_from_py_script() {
        // 监控脚本典型：.venv/bin/python + main.py 路径
        let line = "/Users/username/Desktop/example-monitor/.venv/bin/python /Users/username/Desktop/example-monitor/main.py";
        assert_eq!(
            infer_script_project_dir(line),
            Some("/Users/username/Desktop/example-monitor".to_string())
        );
    }

    #[test]
    fn infer_script_dir_from_node_script() {
        let line = "/opt/homebrew/bin/node /Users/username/Desktop/example-automation/platform-core/src/cli.js daemon start";
        assert_eq!(
            infer_script_project_dir(line),
            Some("/Users/username/Desktop/example-automation/platform-core/src".to_string())
        );
    }

    #[test]
    fn infer_script_dir_fallback_to_exe_dir() {
        // 没有脚本后缀时，退回 argv[0] 所在目录
        let line = "/Users/username/Desktop/example-monitor/.venv/bin/python -m fupan.pipeline";
        // 无 .py 后缀，退回 exe 所在目录
        assert_eq!(
            infer_script_project_dir(line),
            Some("/Users/username/Desktop/example-monitor/.venv/bin".to_string())
        );
    }

    #[test]
    fn infer_script_dir_none_for_plain_cmd() {
        // 纯命令无路径，返回 None
        assert_eq!(infer_script_project_dir("python -c 'print(1)'"), None);
    }

    #[test]
    fn infer_script_dir_quoted_space_path() {
        // 引号包裹的含空格路径（真实 shell 会为含空格参数加引号）
        let line = "/usr/bin/python3 \"/Users/username/My Scripts/run analysis.py\" --flag";
        assert_eq!(
            infer_script_project_dir(line),
            Some("/Users/username/My Scripts".to_string())
        );
    }

    #[test]
    fn shell_split_argv_handles_quotes() {
        let argv = shell_split_argv("python3 \"/a b/c.py\" 'x y' plain\\ z");
        assert_eq!(argv, vec!["python3", "/a b/c.py", "x y", "plain z"]);
    }

    #[test]
    fn process_name_matches_exact_and_boundary() {
        // 精确相等
        assert!(process_name_matches("google chrome", "google chrome"));
        // 边界匹配：空格（Helper）
        assert!(process_name_matches(
            "google chrome helper",
            "google chrome"
        ));
        assert!(process_name_matches(
            "google chrome helper (renderer)",
            "google chrome"
        ));
        // 边界匹配：点
        assert!(process_name_matches(
            "google chrome.helper",
            "google chrome"
        ));
    }

    #[test]
    fn process_name_matches_rejects_short_prefix_false_positive() {
        // 短名 "go" 不应误匹配 "google chrome"（Go 后面是 o，非边界字符）
        assert!(!process_name_matches("google chrome", "go"));
        // "node" 后面是空格（边界）则匹配（node 的子进程）
        assert!(process_name_matches("node runner", "node"));
        // 但 "node" 不应误匹配 "nodemon"（node 后面是 m，非边界）
        assert!(!process_name_matches("nodemon", "node"));
        // "node" 精确相等仍应匹配
        assert!(process_name_matches("node", "node"));
    }

    #[test]
    fn process_name_matches_short_candidate_no_prefix() {
        // 候选名 <2 字符时，只允许精确相等
        assert!(process_name_matches("x", "x"));
        assert!(!process_name_matches("xhelper", "x"));
    }

    fn app_entry(id: &str, name: &str, path: &str, bundle: Option<&str>) -> AppEntry {
        AppEntry {
            id: id.to_string(),
            name: name.to_string(),
            bundle_id: bundle.map(str::to_string),
            path: Some(path.to_string()),
            running: false,
            mode: "direct".to_string(),
            category: "其他".to_string(),
            confirmed: false,
            node: None,
            rule_paths: vec![],
            online: false,
            remote_ips: vec![],
        }
    }

    #[test]
    fn mark_running_apps_matches_exact_path() {
        let mut list = vec![
            app_entry(
                "app-a",
                "SameName",
                "/Applications/A.app/Contents/MacOS/A",
                Some("com.a"),
            ),
            app_entry(
                "app-b",
                "SameName",
                "/Applications/B.app/Contents/MacOS/B",
                Some("com.b"),
            ),
        ];
        let running = vec![RunningProcess {
            app_dir: "/Applications/B.app".to_string(),
            app_path: "/Applications/B.app/Contents/MacOS/B".to_string(),
            bundle_id: String::new(),
        }];

        mark_running_apps(&mut list, &running);

        assert!(!list[0].running);
        assert!(list[1].running);
    }

    #[test]
    fn mark_running_apps_matches_bundle_id_for_helper() {
        let mut list = vec![app_entry(
            "app-a",
            "Demo",
            "/Applications/Demo.app/Contents/MacOS/Demo",
            Some("com.demo"),
        )];
        let running = vec![RunningProcess {
            app_dir: "/Applications/Demo.app".to_string(),
            app_path: "/Applications/Demo.app/Contents/Frameworks/Helper.app/Contents/MacOS/Helper"
                .to_string(),
            bundle_id: "com.demo".to_string(),
        }];

        mark_running_apps(&mut list, &running);

        assert!(list[0].running);
    }

    #[test]
    fn exact_process_path_keeps_app_dir_prefix() {
        assert_eq!(
            exact_process_path(
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Google Chrome.app/",
            ),
            Some("/Applications/Google Chrome.app/Contents/MacOS/Google".to_string()),
        );
    }

    #[test]
    fn scan_running_process_prefers_full_args_path_over_truncated_comm() {
        let args = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome --flag";
        let app_dir = "/Applications/Google Chrome.app/";
        assert_eq!(
            exact_process_path(args, app_dir),
            Some("/Applications/Google Chrome.app/Contents/MacOS/Google".to_string()),
        );
        let comm = "/Applications/Google Chrome.app/Contents/MacOS/Google";
        assert_eq!(
            exact_process_path(comm, app_dir),
            Some("/Applications/Google Chrome.app/Contents/MacOS/Google".to_string()),
        );
    }

    #[test]
    fn mark_running_apps_matches_main_app_dir_when_comm_is_truncated() {
        let mut list = vec![app_entry(
            "app-Demo",
            "Google Chrome",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            Some("com.google.Chrome"),
        )];
        let running = vec![RunningProcess {
            app_dir: "/Applications/Google Chrome.app".to_string(),
            app_path: "/Applications/Google Chrome.app/Contents/MacOS/Google".to_string(),
            bundle_id: "com.google.Chrome".to_string(),
        }];

        mark_running_apps(&mut list, &running);

        assert!(list[0].running);
    }

    #[test]
    fn app_ids_keep_legacy_id_when_name_is_unique() {
        let candidates = vec![(
            PathBuf::from("/Applications/Demo.app"),
            "Demo".to_string(),
            "/Applications/Demo.app/Contents/MacOS/Demo".to_string(),
            Some("com.demo".to_string()),
        )];
        assert_eq!(app_ids(&candidates), vec!["app-Demo"]);
    }

    #[test]
    fn app_ids_disambiguate_same_display_name() {
        let candidates = vec![
            (
                PathBuf::from("/Applications/A.app"),
                "SameName".to_string(),
                "/Applications/A.app/Contents/MacOS/A".to_string(),
                Some("com.a".to_string()),
            ),
            (
                PathBuf::from("/Applications/B.app"),
                "SameName".to_string(),
                "/Applications/B.app/Contents/MacOS/B".to_string(),
                Some("com.b".to_string()),
            ),
        ];
        let ids = app_ids(&candidates);
        assert_eq!(ids, vec!["app-SameName--com.a", "app-SameName--com.b"]);
    }
}
