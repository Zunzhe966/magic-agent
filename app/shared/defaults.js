'use strict';

const KNOWN_PROXIES = [
  {
    name: '搬瓦工直连',
    type: 'vless',
    server: '104.160.40.35',
    port: 443,
    uuid: '268a1166-d31e-478c-a66f-7f9c06c9afaa',
    network: 'tcp',
    tls: true,
    udp: true,
    flow: 'xtls-rprx-vision',
    clientFingerprint: 'chrome',
    servername: '',
    realityOpts: {
      publicKey: 'c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0',
      shortId: 'be08e6123ddcaf32'
    }
  },
  {
    name: 'Texas住宅',
    type: 'vless',
    server: '104.160.40.35',
    port: 443,
    uuid: '7f3e9a2b-4c5d-6e8f-1a2b-3c4d5e6f7a8b',
    network: 'tcp',
    tls: true,
    udp: true,
    flow: 'xtls-rprx-vision',
    clientFingerprint: 'chrome',
    servername: '',
    realityOpts: {
      publicKey: 'c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0',
      shortId: 'bca4b7cfbcb66d57'
    }
  }
];

const DEFAULT_APPS = [
  { id: 'chrome', enabled: true, name: 'Google Chrome', icon: 'C', policy: 'proxy', regex: 'Google Chrome.*$|chrome_crashpad_handler$' },
  { id: 'safari', enabled: true, name: 'Safari', icon: 'S', policy: 'direct', regex: '^(Safari|com\\.apple\\.WebKit|com\\.apple\\.Safari|SafariPlatformSupport|SafariSafeBrowsing|SafariBookmarks|SafariLaunch|SafariNotification)[^ ]*$' },
  { id: 'doubao', enabled: true, name: '豆包浏览器', icon: 'D', policy: 'direct', regex: '^Doubao.*$|chrome_crashpad_handler$' },
  { id: 'telegram', enabled: true, name: 'Telegram', icon: 'T', policy: 'proxy', regex: '^Telegram$' },
  { id: 'wechat', enabled: true, name: '微信', icon: 'W', policy: 'direct', regex: '^WeChat.*$|^WeChatAppEx.*$|^Updater$' },
  { id: 'qq', enabled: true, name: 'QQ', icon: 'Q', policy: 'direct', regex: '^QQ.*$|^QQEXDOC.*$|^QQUpdate.*$' },
  { id: 'claude', enabled: true, name: 'Claude', icon: 'C', policy: 'proxy', regex: '^Claude.*$' },
  { id: 'chatgpt', enabled: true, name: 'ChatGPT / Codex', icon: 'A', policy: 'proxy', regex: '^(ChatGPT|Codex|codex|node_repl|bare-modifier-monitor).*$' },
  { id: 'kimi', enabled: true, name: 'Kimi', icon: 'K', policy: 'direct', regex: '^Kimi.*$' },
  { id: 'qianwen', enabled: true, name: '通义千问', icon: 'Q', policy: 'direct', regex: '^Qianwen.*$' },
  { id: 'ollama', enabled: true, name: 'Ollama', icon: 'O', policy: 'direct', regex: '^Ollama.*$|^ollama$' },
  { id: 'finalshell', enabled: true, name: 'FinalShell', icon: 'F', policy: 'direct', regex: '^FinalShell.*$' },
  { id: 'tencentmeeting', enabled: true, name: '腾讯会议', icon: 'M', policy: 'direct', regex: '^TencentMeeting.*$' },
  { id: 'todesk', enabled: true, name: 'ToDesk', icon: 'R', policy: 'direct', regex: '^ToDesk.*$' },
  { id: 'obsidian', enabled: true, name: 'Obsidian', icon: 'O', policy: 'direct', regex: '^Obsidian.*$' },
  { id: 'workbuddy', enabled: true, name: 'WorkBuddy', icon: 'B', policy: 'direct', regex: '^WorkBuddy.*$|^Electron$' },
  { id: 'qclaw', enabled: true, name: 'QClaw', icon: 'Q', policy: 'direct', regex: '^QClaw.*$' },
  { id: 'ccswitch', enabled: true, name: 'CC Switch', icon: 'C', policy: 'direct', regex: '^cc-switch$' },
  { id: 'ndm', enabled: true, name: '下载管理器', icon: 'N', policy: 'proxy', regex: '^NeatDownloadManager$' }
];

const PROXY_PORTS = { http: 0, socks: 0, controller: 0, dns: 0 };
const CONTROLLER_SECRET = 'magic-agent-local';

module.exports = { KNOWN_PROXIES, DEFAULT_APPS, PROXY_PORTS, CONTROLLER_SECRET };
