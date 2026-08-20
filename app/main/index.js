'use strict';

const path = require('path');
const { app, BrowserWindow, ipcMain, shell, dialog } = require('electron');
const { SettingsStore } = require('./settings-store');
const { ProxyEngine } = require('./proxy-engine');
const { SshManager } = require('./ssh-manager');
const { setSystemProxy } = require('./system-proxy');
const { scanApps } = require('./process-scanner');

let win = null;
let settingsStore;
let proxyEngine;
let sshManager;
let sysProxyEnabled = false;

function createWindow() {
  win = new BrowserWindow({
    width: 1280,
    height: 820,
    minWidth: 1024,
    minHeight: 700,
    title: '魔法代理',
    backgroundColor: '#0d1117',
    titleBarStyle: 'hiddenInset',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });
  win.loadFile(path.join(__dirname, '..', 'renderer', 'index.html'));
  win.on('closed', () => (win = null));
}

function sendToRenderer(channel, payload) {
  if (win && !win.isDestroyed()) win.webContents.send(channel, payload);
}

function registerIpc() {
  ipcMain.handle('app:get-boot', async () => {
    const settings = settingsStore.get();
    const status = proxyEngine.getStatus();
    const selected = await proxyEngine.getSelected();
    return {
      settings,
      proxy: { ...status, selected },
      systemProxy: sysProxyEnabled
    };
  });

  ipcMain.handle('proxy:start', async (event, opts) => {
    const result = await proxyEngine.start();
    if (result.ok && opts?.systemProxy !== false && settingsStore.get().systemProxy) {
      const sp = await setSystemProxy(true);
      sysProxyEnabled = sp.ok;
    }
    return { ...result, proxy: proxyEngine.getStatus() };
  });

  ipcMain.handle('proxy:stop', async () => {
    if (sysProxyEnabled) await setSystemProxy(false);
    sysProxyEnabled = false;
    await proxyEngine.stop();
    return { ok: true, proxy: proxyEngine.getStatus() };
  });

  ipcMain.handle('proxy:restart', async () => {
    await proxyEngine.restart();
    return { ok: true, proxy: proxyEngine.getStatus() };
  });

  ipcMain.handle('proxy:status', () => proxyEngine.getStatus());
  ipcMain.handle('proxy:log', () => proxyEngine.readLog());
  ipcMain.handle('proxy:connections', () => proxyEngine.connections());
  ipcMain.handle('proxy:select', (e, name) => proxyEngine.selectProxy(name));

  ipcMain.handle('settings:get', () => settingsStore.get());
  ipcMain.handle('settings:update', (e, patch) => {
    const settings = settingsStore.update(patch);
    return settings;
  });

  ipcMain.handle('apps:scan', () => scanApps());

  ipcMain.handle('ssh:connect', async (e, server) => {
    const sendLine = l => sendToRenderer('ssh:data', l);
    const result = await sshManager.connect(server, sendLine);
    if (result.ok) sshManager.openShell(server.id, sendLine);
    return result;
  });
  ipcMain.handle('ssh:disconnect', (e, id) => sshManager.disconnect(id));
  ipcMain.handle('ssh:write', (e, id, data) => sshManager.writeShell(id, data));
  ipcMain.handle('ssh:run', (e, id, command) => sshManager.runCommand(id, command));
  ipcMain.handle('ssh:info', async (e, id) => sshManager.runCommand(id, 'printf "===INFO===\\nhostname=$(hostname)\\nuptime=$(uptime)\\nmem=$(vm_stat | awk \'/Pages free/ {printf \\"%.1f GB\\", $3*4096/1073741824}\')\\nload=$(cat /proc/loadavg 2>/dev/null || echo n/a)\\ndisk=$(df -h / | awk \'NR==2{print $4 \\" free\\"}\')\\nkernel=$(uname -r)\\n"'));
  ipcMain.handle('ssh:reboot', async (e, id) => sshManager.runCommand(id, 'sudo -n reboot 2>/dev/null || reboot 2>/dev/null || echo "需要手动执行 sudo reboot"'));
  ipcMain.handle('ssh:poweroff', async (e, id) => sshManager.runCommand(id, 'sudo -n poweroff 2>/dev/null || poweroff 2>/dev/null || echo "需要手动执行 sudo poweroff"'));

  ipcMain.handle('dialog:open-json', async () => {
    const r = await dialog.showOpenDialog(win, {
      properties: ['openFile'],
      filters: [{ name: '配置', extensions: ['json'] }]
    });
    if (r.canceled || !r.filePaths[0]) return { canceled: true };
    const fs = require('fs');
    const data = JSON.parse(fs.readFileSync(r.filePaths[0], 'utf8'));
    return { canceled: false, data };
  });

  ipcMain.handle('shell:open-path', (e, p) => {
    if (p) shell.openPath(p);
  });
}

app.whenReady().then(async () => {
  settingsStore = new SettingsStore();
  proxyEngine = new ProxyEngine(settingsStore);
  sshManager = new SshManager();
  registerIpc();
  createWindow();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', async (e) => {
  if (sysProxyEnabled) {
    e.preventDefault();
    await setSystemProxy(false);
    sysProxyEnabled = false;
    app.quit();
  }
});
