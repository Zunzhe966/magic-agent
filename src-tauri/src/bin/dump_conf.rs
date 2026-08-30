/// 调试/校验工具：从 stdin 读 config.json（JSON），把 Rust 引擎生成的 mihomo 规则
/// 逐行打印到 stdout。供 scripts/check_parity.py 与 Python 引擎做一致性 diff。
use magic_agent_lib::config::AppConfig;
use magic_agent_lib::settings_to_app_rules;

fn main() {
    use std::io::Read;
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).expect("读取 stdin 失败");
    let cfg: AppConfig = serde_json::from_str(&input).expect("config JSON 解析失败");
    // bin- 前缀条目不依赖文件系统扫描，两侧引擎可确定性对比；app- 条目跳过（路径需实机扫描）
    let path_lookup = std::collections::HashMap::new();
    let valid_nodes: std::collections::HashSet<String> =
        cfg.nodes.iter().map(|n| n.name.clone()).collect();
    let app_rules = settings_to_app_rules(&cfg.apps, &path_lookup, &valid_nodes);
    let m = magic_agent_lib::mihomo::MihomoManager::new();
    for r in m.build_rules_for(&cfg, &app_rules) {
        println!("{}", r);
    }
}
