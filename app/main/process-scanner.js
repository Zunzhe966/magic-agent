'use strict';

const { execFile } = require('child_process');
const util = require('util');
const path = require('path');
const fs = require('fs');

const execFileP = util.promisify(execFile);

const IGNORE = new Set([
  'FlClash', 'FlClashCore', 'mihomo', 'launchd', 'kernel_task', 'WindowServer', 'SystemUIServer',
  'ControlCenter', 'Dock', 'Finder', 'loginwindow', 'cfprefsd', 'nsurlsessiond', 'safaridavclient',
  'com.apple.WebKit.GPU', 'tailscaled', 'node', 'npm', 'Electron', 'osascript', 'sh', 'zsh', 'bash'
]);

async function scanApps() {
  if (process.platform !== 'darwin') return [];
  const { stdout } = await execFileP('/bin/ps', ['-axo', 'pid=,comm=']);
  const lines = stdout.split('\n');
  const found = [];
  const seen = new Set();
  for (const line of lines) {
    const match = line.match(/^\s*(\d+)\s+(.+)$/);
    if (!match) continue;
    const [, pidStr, comm] = match;
    const p = comm.trim();
    if (!p || p.startsWith('/System/Library/') || p.startsWith('/usr/libexec/') || p.startsWith('/usr/sbin/')) continue;
    const name = path.basename(p);
    if (IGNORE.has(name)) continue;
    if (seen.has(name)) continue;
    seen.add(name);
    let bundle = '';
    if (p.startsWith('/Applications/')) {
      const parts = p.split('/');
      const idx = parts.findIndex(x => x.endsWith('.app'));
      if (idx >= 0) {
        const appPath = parts.slice(0, idx + 1).join('/');
        const info = path.join(appPath, 'Contents', 'Info.plist');
        if (fs.existsSync(info)) {
          try {
            const { stdout: plist } = await execFileP('/usr/bin/plutil', ['-extract', 'CFBundleName', 'raw', info]);
            bundle = plist.trim();
          } catch {}
        }
      }
    }
    found.push({ pid: Number(pidStr), name, path: p, bundle: bundle || name });
  }
  return found.sort((a, b) => a.name.localeCompare(b.name));
}

module.exports = { scanApps };
