const { execSync } = require('node:child_process');
const path = require('node:path');

const DEFAULT_APPS = [
  { key: 'Google Chrome', name: 'Google Chrome', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Chrome Helper', name: 'Chrome 辅助进程', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Safari', name: 'Safari', kind: '浏览器', policy: '直连', group: '国内浏览' },
  { key: 'WebKit', name: 'Safari 内核服务', kind: '浏览器', policy: '直连', group: '国内浏览' },
  { key: 'Doubao Browser', name: '豆包浏览器', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Doubao Browser Helper', name: '豆包浏览器辅助进程', kind: '浏览器', policy: '代理', group: '外网浏览' },
  { key: 'Telegram', name: 'Telegram', kind: '通讯', policy: '代理', group: '外网通讯' },
  { key: 'WeChat', name: '微信', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'WeChatAppEx', name: '微信小程序', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'QQ', name: 'QQ', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'QQ Helper', name: 'QQ 辅助进程', kind: '通讯', policy: '直连', group: '国内通讯' },
  { key: 'Claude', name: 'Claude', kind: 'AI', policy: '代理', group: '外网AI' },
  { key: 'ChatGPT', name: 'ChatGPT', kind: 'AI', policy: '代理', group: '外网AI' },
  { key: 'Kimi', name: 'Kimi', kind: 'AI', policy: '直连', group: '国内AI' },
  { key: 'Doubao', name: '豆包', kind: 'AI', policy: '直连', group: '国内AI' },
  { key: 'Qianwen', name: '通义千问', kind: 'AI', policy: '直连', group: '国内AI' },
  { key: 'Ollama', name: 'Ollama', kind: 'AI', policy: '直连', group: '本机AI' },
  { key: 'ToDesk', name: 'ToDesk', kind: '远程', policy: '直连', group: '远程控制' },
  { key: 'TencentMeeting', name: '腾讯会议', kind: '办公', policy: '直连', group: '国内办公' },
  { key: 'NeatDownloadManager', name: 'NDM 下载', kind: '工具', policy: '直连', group: '下载工具' },
  { key: 'WorkBuddy', name: '腾讯 WorkBuddy', kind: '办公', policy: '直连', group: '国内办公' },
  { key: 'FinalShell', name: 'FinalShell', kind: '开发', policy: '直连', group: '服务器工具' },
  { key: 'Obsidian', name: 'Obsidian', kind: '笔记', policy: '直连', group: '本机笔记' },
  { key: 'Codex', name: 'Codex', kind: 'AI', policy: '代理', group: '外网AI' }
];

class AppProfiler {
  constructor(store) {
    this.store = store;
  }

  getProfiles() {
    const policies = this.store.get('appPolicies') || {};
    const groups = this.store.get('appGroups') || {};
    const merged = DEFAULT_APPS.map((app) => ({
      ...app,
      policy: policies[app.key] || app.policy,
      group: groups[app.key] || app.group
    }));
    const known = new Set(DEFAULT_APPS.map((a) => a.key));
    for (const [key, policy] of Object.entries(policies)) {
      if (!known.has(key)) {
        merged.push({ key, name: key, kind: '自定义', policy, group: groups[key] || '自定义' });
      }
    }
    return merged;
  }

  setPolicy(appKey, policy) {
    const policies = this.store.get('appPolicies') || {};
    policies[appKey] = policy;
    this.store.set('appPolicies', policies);
    return this.getProfiles();
  }

  setGroup(appKey, groupName) {
    const groups = this.store.get('appGroups') || {};
    groups[appKey] = groupName;
    this.store.set('appGroups', groups);
    return this.getProfiles();
  }

  scanRunningApps() {
    const out = [];
    try {
      const text = execSync("ps -axo comm | sed 's#^/.*/##' | sort -u", { encoding: 'utf8', timeout: 3000 });
      const names = text.split('\n').map((s) => s.trim()).filter(Boolean);
      const profiles = this.getProfiles();
      for (const profile of profiles) {
        const hit = names.find((n) => n.toLowerCase().includes(profile.key.toLowerCase()));
        if (hit) out.push({ ...profile, running: true, processName: hit });
      }
    } catch (_) {}
    return out;
  }
}

module.exports = AppProfiler;
