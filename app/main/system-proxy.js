'use strict';

const { execFile } = require('child_process');
const util = require('util');
const { PROXY_PORTS } = require('../shared/defaults');

const execFileP = util.promisify(execFile);

const SERVICES = [
  'Wi-Fi',
  'Ethernet',
  'Thunderbolt Bridge',
  'USB 10/100/1000 LAN'
];

async function getService() {
  for (const s of SERVICES) {
    try {
      const { stdout } = await execFileP('/usr/sbin/networksetup', ['-getwebproxy', s]);
      if (!/not set/.test(stdout)) return s;
    } catch {}
  }
  return 'Wi-Fi';
}

async function setSystemProxy(enable) {
  if (process.platform !== 'darwin') return { ok: true, note: '仅 macOS 支持系统代理' };
  try {
    const service = await getService();
    if (enable) {
      await execFileP('/usr/sbin/networksetup', ['-setwebproxy', service, '127.0.0.1', String(PROXY_PORTS.http), 'off']);
      await execFileP('/usr/sbin/networksetup', ['-setsecurewebproxy', service, '127.0.0.1', String(PROXY_PORTS.http), 'off']);
      await execFileP('/usr/sbin/networksetup', ['-setsocksfirewallproxy', service, '127.0.0.1', String(PROXY_PORTS.socks), 'off']);
      await execFileP('/usr/sbin/networksetup', ['-setwebproxystate', service, 'on']);
      await execFileP('/usr/sbin/networksetup', ['-setsecurewebproxystate', service, 'on']);
      await execFileP('/usr/sbin/networksetup', ['-setsocksfirewallproxystate', service, 'on']);
    } else {
      await execFileP('/usr/sbin/networksetup', ['-setwebproxystate', service, 'off']);
      await execFileP('/usr/sbin/networksetup', ['-setsecurewebproxystate', service, 'off']);
      await execFileP('/usr/sbin/networksetup', ['-setsocksfirewallproxystate', service, 'off']);
    }
    return { ok: true, service };
  } catch (e) {
    return { ok: false, error: e.message };
  }
}

module.exports = { setSystemProxy };
