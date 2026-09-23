// P1-2 接管账本 ledger.rs（docs/重构计划.md）。
//
// 立场：接管必须"先记账、后动手"——每次改变系统状态前，把【将被改变的每一项
// 原值】快照进账本；异常退出留下未结账本时，下次启动如实提示。
// 这是"网络管理员"区别于"又一个会抢端口的代理软件"的核心凭证。
//
// 设计约束：
// 1. 原子写 + 0600（复用 config.rs::save 模式：临时文件 → chmod → rename；
//    chmod 必须在 rename 前，防泄露窗口）。
// 2. 跨进程：本文件被 Rust App 与 Python MCP 两个进程写。写入走
//    读→合并→原子写，且 begin/settle 对"已存在同状态会话"幂等——
//    并发最后写者赢，但不会写出重复/半截会话。彻底收敛在 P2-1。
// 3. 损坏健壮性（复用 config.rs 思路）：解析失败 → 原文件改存
//    ledger.json.corrupt（不覆盖已有 corrupt，加时间戳）→ 从空账本继续，
//    绝不因账本坏了就拒绝启动或丢现场证据。
// 4. 语义红线：已有未结账本时再次接管【不更新 before】——账本保的是
//    最初的原值，不是"上一次接管后的状态"。
//
// JSON schema 与 mcp/server.py 的 ledger 段一一对应（camelCase），
// 改动必须两侧同步（CONTRACT 双入口一致性红线）。

use serde::{Deserialize, Serialize};

use crate::system_proxy::ServiceSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// system_proxy / process / route（route 目前无人改变，留位）
    pub kind: String,
    /// 对象键：服务名（system_proxy）或 "标签 (PID n)"（process）
    pub key: String,
    /// 原值：system_proxy = ServiceSnapshot 对象；process = "running"
    pub before: serde_json::Value,
    /// 接管后状态（begin 时多为 null，settle/kill 时回填）
    pub after: serde_json::Value,
    /// process 被杀不可逆——账本必须如实说"这项回滚不了"
    pub reversible: bool,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    /// open = 接管生效中未结账；settled = 正常停止已结账
    pub status: String,
    pub reason: String,
    pub started_ts: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_ts: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_reason: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Ledger {
    pub version: u32,
    pub sessions: Vec<Session>,
}

pub fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn default_path() -> std::path::PathBuf {
    let dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.join("magic-agent").join("ledger.json")
}

/// 可注入路径的账本（单测用 tmp 目录；生产用 default_path）。
pub struct LedgerFile {
    pub path: std::path::PathBuf,
}

impl Default for LedgerFile {
    fn default() -> Self {
        Self { path: default_path() }
    }
}

impl LedgerFile {
    /// 读取；缺失 → 空账本。解析失败 → 原文件保全为 corrupt 证据后从空开始
    /// （返回的 note 供调用方日志/提示，绝不静默吞掉）。
    pub fn load(&self) -> (Ledger, Option<String>) {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(_) => return (Ledger { version: 1, sessions: vec![] }, None),
        };
        match serde_json::from_str::<Ledger>(&text) {
            Ok(l) if l.version >= 1 => (l, None),
            Ok(_) => (
                Ledger { version: 1, sessions: vec![] },
                Some("账本 version 异常，已按空账本继续".to_string()),
            ),
            Err(e) => {
                let backup = self.corrupt_backup_path();
                let note = match std::fs::rename(&self.path, &backup) {
                    Ok(_) => format!("账本损坏（{e}），原文件已保全为 {}，从空账本继续", backup.display()),
                    Err(re) => format!("账本损坏（{e}），且备份失败（{re}），从空账本继续"),
                };
                (Ledger { version: 1, sessions: vec![] }, Some(note))
            }
        }
    }

    fn corrupt_backup_path(&self) -> std::path::PathBuf {
        let base = self.path.with_extension("json.corrupt");
        if !base.exists() {
            return base;
        }
        // 已有 corrupt 证据：绝不覆盖，加时间戳另存
        self.path
            .with_extension(format!("json.corrupt.{}", now_ts()))
    }

    /// 原子写（config.rs::save 同款：tmp → chmod 600 → rename）。
    pub fn save(&self, ledger: &Ledger) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(ledger).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    /// 当前未结账本（最新一个 open 会话）。
    pub fn open_session(&self) -> Option<Session> {
        let (ledger, note) = self.load();
        if let Some(n) = note {
            eprintln!("[ledger] {n}");
        }
        ledger
            .sessions
            .iter()
            .find(|s| s.status == "open")
            .cloned()
    }

    /// 接管开始：无 open 会话时，逐服务快照系统代理原值入账。
    /// 已有 open 会话 → 不动账本（before 必须是【最初】原值），返回 false。
    /// ⚠️ 必须在任何状态变更前调用（cleanup / set_system_proxy 之前）。
    pub fn begin_takeover(&self, reason: &str, snapshots: Vec<ServiceSnapshot>) -> Result<bool, String> {
        let (mut ledger, _note) = self.load();
        if ledger.sessions.iter().any(|s| s.status == "open") {
            return Ok(false);
        }
        let ts = now_ts();
        let entries = snapshots
            .into_iter()
            .map(|snap| Entry {
                kind: "system_proxy".into(),
                key: snap.service.clone(),
                before: serde_json::to_value(&snap).unwrap_or(serde_json::Value::Null),
                after: serde_json::Value::Null,
                reversible: true,
                ts,
            })
            .collect();
        ledger.sessions.push(Session {
            id: format!("ls-{ts}-{}", std::process::id()),
            status: "open".into(),
            reason: reason.into(),
            started_ts: ts,
            settled_ts: None,
            settled_reason: None,
            entries,
        });
        self.save(&ledger)?;
        Ok(true)
    }

    /// 记录被清理的第三方代理进程（不可逆项，如实标 reversible=false）。
    pub fn record_killed_procs(&self, procs: &[String]) -> Result<(), String> {
        if procs.is_empty() {
            return Ok(());
        }
        let (mut ledger, _note) = self.load();
        let ts = now_ts();
        if let Some(session) = ledger.sessions.iter_mut().find(|s| s.status == "open") {
            for p in procs {
                session.entries.push(Entry {
                    kind: "process".into(),
                    key: p.clone(),
                    before: serde_json::Value::String("running".into()),
                    after: serde_json::Value::String("killed".into()),
                    reversible: false,
                    ts,
                });
            }
        }
        self.save(&ledger)
    }

    /// 结账：把 open 会话标记 settled（停止代理时调用）。
    /// 无 open 会话 → false（幂等：MCP/Rust 先后 settle 不互踩）。
    pub fn settle_open(&self, reason: &str) -> Result<bool, String> {
        let (mut ledger, _note) = self.load();
        let mut done = false;
        if let Some(session) = ledger.sessions.iter_mut().find(|s| s.status == "open") {
            session.status = "settled".into();
            session.settled_ts = Some(now_ts());
            session.settled_reason = Some(reason.into());
            done = true;
        }
        if done {
            self.save(&ledger)?;
        }
        Ok(done)
    }

    /// P1-3 回滚原语：把【当前 open 会话】里所有 system_proxy 条目按 before
    /// 逐服务回放回接管前的原值，然后结账。verify 不达标时 start_proxy 调用它
    /// ——宁可不启，不留半套秩序。
    /// 返回 (回放错误清单, 是否有账可回)。before 解析失败的条目跳过并点名
    /// （凭 unknown 瞎写比重置失败更危险）。
    pub fn rollback_session(&self, reason: &str) -> Result<(Vec<String>, bool), String> {
        self.rollback_session_with(reason, crate::system_proxy::restore_service_snapshot)
    }

    /// 可注入回放函数的版本（单测用 mock，绝不真动本机系统代理）。
    pub fn rollback_session_with(
        &self,
        reason: &str,
        mut restore: impl FnMut(&crate::system_proxy::ServiceSnapshot) -> Vec<String>,
    ) -> Result<(Vec<String>, bool), String> {
        let session = match self.open_session() {
            Some(s) => s,
            None => return Ok((vec![], false)),
        };
        let mut errs = Vec::new();
        for entry in &session.entries {
            if entry.kind != "system_proxy" {
                continue; // process 项不可逆，账本如实承认"回不来"
            }
            match serde_json::from_value::<crate::system_proxy::ServiceSnapshot>(entry.before.clone()) {
                Ok(snap) => errs.extend(restore(&snap)),
                Err(e) => errs.push(format!("{}: 快照解析失败（{e}），跳过还原", entry.key)),
            }
        }
        // 有账可回就结账（无论回放是否全成）：结账=接管结束，回放失败的项
        // 由调用方如实上报，绝不假装恢复成功
        let _ = self.settle_open(reason);
        Ok((errs, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_proxy::ProxyChannelRaw;
    use std::path::PathBuf;

    fn tmp_ledger(tag: &str) -> LedgerFile {
        let dir = std::env::temp_dir().join(format!(
            "magic-ledger-test-{}-{}-{tag}",
            std::process::id(),
            now_ts()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        LedgerFile { path: dir.join("ledger.json") }
    }

    fn snap(svc: &str, on: bool, port: u16) -> ServiceSnapshot {
        ServiceSnapshot {
            service: svc.into(),
            http: ProxyChannelRaw { enabled: on, server: if on { "127.0.0.1".into() } else { "".into() }, port },
            https: ProxyChannelRaw { enabled: false, server: String::new(), port: 0 },
            socks: ProxyChannelRaw { enabled: false, server: String::new(), port: 0 },
            errors: vec![],
        }
    }

    fn cleanup(p: &PathBuf) {
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn missing_file_loads_empty() {
        let l = tmp_ledger("missing");
        let (ledger, note) = l.load();
        assert_eq!(ledger.version, 1);
        assert!(ledger.sessions.is_empty());
        assert!(note.is_none());
        cleanup(&l.path);
    }

    #[test]
    fn begin_records_full_original_values() {
        let l = tmp_ledger("begin");
        let created = l.begin_takeover("start_proxy", vec![snap("Wi-Fi", true, 7890), snap("USB 10/100 LAN", false, 0)]).unwrap();
        assert!(created);
        let session = l.open_session().expect("应有未结账本");
        assert_eq!(session.entries.len(), 2);
        assert_eq!(session.entries[0].kind, "system_proxy");
        assert!(session.entries[0].reversible);
        // before 必须携带【完整原值】：开着且指向第三方端口 7890 —— 这正是还原的依据
        let before = &session.entries[0].before;
        assert_eq!(before["http"]["enabled"], serde_json::json!(true));
        assert_eq!(before["http"]["port"], serde_json::json!(7890));
        assert_eq!(before["service"], serde_json::json!("Wi-Fi"));
        cleanup(&l.path);
    }

    #[test]
    fn second_begin_does_not_clobber_original_before() {
        let l = tmp_ledger("clobber");
        l.begin_takeover("start_proxy", vec![snap("Wi-Fi", true, 7890)]).unwrap();
        // 接管后又点一次启动：账本 before 绝不能被"已被接管后的状态"覆盖
        let created = l.begin_takeover("start_proxy2", vec![snap("Wi-Fi", true, 7891)]).unwrap();
        assert!(!created);
        let session = l.open_session().unwrap();
        assert_eq!(session.reason, "start_proxy");
        assert_eq!(session.entries[0].before["http"]["port"], serde_json::json!(7890));
        cleanup(&l.path);
    }

    #[test]
    fn killed_procs_recorded_as_irreversible() {
        let l = tmp_ledger("kill");
        l.begin_takeover("start_proxy", vec![snap("Wi-Fi", false, 0)]).unwrap();
        l.record_killed_procs(&["FlClash (PID 123)".into(), "verge-mihomo 内核 (PID 456)".into()]).unwrap();
        let session = l.open_session().unwrap();
        assert_eq!(session.entries.len(), 3);
        let proc = &session.entries[1];
        assert_eq!(proc.kind, "process");
        assert!(!proc.reversible, "杀进程不可逆，账本必须如实标 false");
        assert_eq!(proc.before, serde_json::json!("running"));
        assert_eq!(proc.after, serde_json::json!("killed"));
        // 无 open 会话时记录 → 静默跳过不报错（MCP/Rust 竞争场景）
        l.settle_open("stop").unwrap();
        l.record_killed_procs(&["Ghost (PID 1)".into()]).unwrap();
        assert_eq!(l.load().0.sessions.iter().map(|s| s.entries.len()).sum::<usize>(), 3);
        cleanup(&l.path);
    }

    #[test]
    fn settle_marks_and_is_idempotent() {
        let l = tmp_ledger("settle");
        assert!(!l.settle_open("stop").unwrap(), "无 open 会话时 settle 应返回 false");
        l.begin_takeover("start_proxy", vec![snap("Wi-Fi", false, 0)]).unwrap();
        assert!(l.settle_open("stop_proxy").unwrap());
        assert!(l.open_session().is_none());
        assert!(!l.settle_open("stop_again").unwrap());
        let session = &l.load().0.sessions[0];
        assert_eq!(session.status, "settled");
        assert_eq!(session.settled_reason.as_deref(), Some("stop_proxy"));
        // 结账后可再次接管，形成新会话
        assert!(l.begin_takeover("start_proxy2", vec![snap("Wi-Fi", false, 0)]).unwrap());
        assert_eq!(l.load().0.sessions.len(), 2);
        cleanup(&l.path);
    }

    #[test]
    fn corrupt_file_preserved_as_evidence_then_empty() {
        let l = tmp_ledger("corrupt");
        let base = l.path.with_extension("json.corrupt");
        std::fs::write(&l.path, "{ not json at all !!!").unwrap();
        std::fs::write(&base, "OLD EVIDENCE").unwrap(); // 预置 corrupt 证据：绝不覆盖
        let (ledger, note) = l.load();
        assert!(ledger.sessions.is_empty());
        assert!(note.unwrap().contains("损坏"));
        // 原损坏文件被改名保全为带时间戳的第二个 corrupt，预置证据原样在
        assert!(!l.path.exists());
        assert_eq!(std::fs::read_to_string(&base).unwrap(), "OLD EVIDENCE");
        let entries: Vec<_> = std::fs::read_dir(l.path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("corrupt"))
            .collect();
        assert_eq!(entries.len(), 2, "应同时存在预置证据与本次保全文件: {entries:?}");
        cleanup(&l.path);
    }

    #[test]
    fn save_atomic_and_private() {
        let l = tmp_ledger("perm");
        l.begin_takeover("start_proxy", vec![snap("Wi-Fi", false, 0)]).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&l.path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "账本含网络配置原值，必须 0600");
        }
        assert!(!l.path.with_extension("json.tmp").exists(), "rename 后不得残留 tmp");
        cleanup(&l.path);
    }

    /// 跨引擎契约锁：JSON 必须携带 snake_case→camelCase 后的键，
    /// 与 mcp/server.py 读写完全一致（改字段名 = 破坏双引擎共账，必须有测试挡）。
    #[test]
    fn schema_keys_are_camel_case_contract() {
        let l = tmp_ledger("schema");
        l.begin_takeover("start_proxy", vec![snap("Wi-Fi", false, 0)]).unwrap();
        l.settle_open("stop_proxy").unwrap();
        let text = std::fs::read_to_string(&l.path).unwrap();
        for key in ["\"version\"", "\"sessions\"", "\"startedTs\"", "\"settledTs\"", "\"settledReason\"", "\"system_proxy\"", "\"reversible\"", "\"before\""] {
            assert!(text.contains(key), "账本 JSON 缺少契约键 {key}:\n{text}");
        }
        for bad in ["\"started_ts\"", "\"settled_ts\"", "\"isReversible\""] {
            assert!(!text.contains(bad), "账本 JSON 出现非契约键 {bad}");
        }
        cleanup(&l.path);
    }

    /// P1-3 回滚：逐服务回放 before 原值 + 结账；回放错误如实返回不假装。
    #[test]
    fn rollback_restores_each_snapshot_then_settles() {
        let l = tmp_ledger("rollback");
        l.begin_takeover(
            "start_proxy",
            vec![snap("Wi-Fi", true, 7890), snap("USB 10/100 LAN", false, 0)],
        )
        .unwrap();
        let mut calls: Vec<(String, bool, u16)> = Vec::new();
        let (errs, had) = l
            .rollback_session_with("verify 不达标", |s| {
                calls.push((s.service.clone(), s.http.enabled, s.http.port));
                if s.service == "USB 10/100 LAN" {
                    return vec!["mock 写失败".to_string()];
                }
                vec![]
            })
            .unwrap();
        assert!(had);
        // 两个服务都按 before 原值回放：Wi-Fi 开着指 7890（不是接管后的 7891）
        assert_eq!(calls.len(), 2);
        assert!(calls.contains(&("Wi-Fi".to_string(), true, 7890)));
        // 回放失败项如实返回
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("mock 写失败"));
        // 回滚即结账（接管结束），且 reason 可追溯
        assert!(l.open_session().is_none());
        let session = &l.load().0.sessions[0];
        assert_eq!(session.status, "settled");
        assert!(session.settled_reason.as_deref().unwrap().contains("verify 不达标"));
        cleanup(&l.path);
    }

    #[test]
    fn rollback_skips_irreversible_process_entries() {
        let l = tmp_ledger("rollback-proc");
        l.begin_takeover("start_proxy", vec![snap("Wi-Fi", false, 0)]).unwrap();
        l.record_killed_procs(&["FlClash (PID 1)".into()]).unwrap();
        let mut restored_keys = Vec::new();
        let (errs, had) = l
            .rollback_session_with("stop", |s| {
                restored_keys.push(s.service.clone());
                vec![]
            })
            .unwrap();
        assert!(had);
        assert!(errs.is_empty());
        assert_eq!(restored_keys, vec!["Wi-Fi".to_string()], "process 项绝不进回放");
        cleanup(&l.path);
    }

    /// 快照 before 损坏（非对象）→ 跳过该服务并点名，绝不凭 unknown 瞎写网络配置
    #[test]
    fn rollback_tolerates_corrupt_snapshot_entry() {
        let l = tmp_ledger("rollback-corrupt");
        l.begin_takeover("start_proxy", vec![snap("Wi-Fi", false, 0)]).unwrap();
        // 手工把 before 改坏
        let (mut ledger, _) = l.load();
        ledger.sessions[0].entries[0].before = serde_json::json!({"garbage": 1});
        l.save(&ledger).unwrap();
        let mut called = 0usize;
        let (errs, had) = l
            .rollback_session_with("stop", |_| {
                called += 1;
                vec![]
            })
            .unwrap();
        assert!(had);
        assert_eq!(called, 0, "解析失败不得调用回放");
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("快照解析失败"));
        cleanup(&l.path);
    }

    #[test]
    fn rollback_without_open_session_reports_no_ledger() {
        let l = tmp_ledger("rollback-none");
        let (errs, had) = l.rollback_session_with("stop", |_| vec![]).unwrap();
        assert!(!had, "无账可回必须如实返回 false");
        assert!(errs.is_empty());
        cleanup(&l.path);
    }
}
