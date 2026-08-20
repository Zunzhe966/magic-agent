const { app, BrowserWindow, ipcMain, shell } = require('electron');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');
const { execFile } = require('node:child_process');

const ProxyEngine = require('./lib/proxy-engine');
const ServerManager = require('./lib/server-manager');
const AppProfiler = require('./lib/app-profiler');
const Store = require('./lib/store');

const tracePath = '/tmp/magic-trace.txt';
function tr(line) {
  try { fs.appendFileSync(tracePath, line + '\n'); } catch (_) {}
}
tr('main loaded');
process.on('exit', (code) => tr('process exit code=' + code));
process.on('uncaughtException', (e) => tr('uncaught ' + (e && e.stack ? e.stack : e)));

if (require('electron-squirrel-startup')) {
  app.quit();
}

let mainWindow = null;
const store = new Store();
const appProfiler = new AppProfiler(store);
const serverManager = new ServerManager(store, (log) => pushLog(log));
const proxyEngine = new ProxyEngine(store, appProfiler, (log) => pushLog(log));

function pushLog(message) {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send('magic:log', message);
  }
}

function createWindow() {
  tr('createWindow start');
  mainWindow = new BrowserWindow({
    width: 1240,
    height: 800,
    minWidth: 1080,
    minHeight: 680,
    title: '魔法代理',
    backgroundColor: '#f4f6fb',
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 18, y: 18 },
    webPreferences: {
      preload: MAIN_WINDOW_PRELOAD_WEBPACK_ENTRY,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });
  mainWindow.on('closed', () => { tr('window closed'); mainWindow = null; });
  mainWindow.webContents.once('did-finish-load', () => tr('did-finish-load'));
  mainWindow.webContents.once('did-fail-load', (_e, code, desc) => tr('did-fail-load ' + code + ' ' + desc));
  mainWindow.webContents.once('render-process-gone', (_e, det) => tr('render-process-gone ' + JSON.stringify(det)));
  mainWindow.webContents.once('destroyed', () => tr('webContents destroyed'));
  tr('about to loadURL ' + MAIN_WINDOW_WEBPACK_ENTRY);
  mainWindow.loadURL(MAIN_WINDOW_WEBPACK_ENTRY).then(() => tr('loadURL resolved')).catch((e) => tr('loadURL rejected ' + e));
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('http://') || url.startsWith('https://')) shell.openExternal(url);
    return { action: 'deny' };
  });
}

app.whenReady().then(async () => {
  tr('ready fired');
  try {
    proxyEngine.ensureBundledFiles();
    await proxyEngine.init();
    await serverManager.init();
  } catch (err) {
    pushLog({ level: 'error', text: `初始化失败: ${err.message}` });
  }
  tr('about to createWindow');
  createWindow();
  tr('after createWindow');
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) createWindow();
});

app.on('window-all-closed', () => {
  tr('window-all-closed fired');
  if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', async () => {
  tr('before-quit fired');
  try {
    await proxyEngine.stop();
    serverManager.dispose();
  } catch (_) {}
});

ipcMain.handle('magic:get-state', async () => {
  const proxy = await proxyEngine.snapshot();
  const servers = serverManager.snapshot();
  const config = store.get('appConfig') || {};
  return {
    proxy,
    servers,
    config,
    appProfiles: appProfiler.getProfiles(),
    runningApps: appProfiler.scanRunningApps()
  };
});

ipcMain.handle('magic:start-proxy', async () => {
  await proxyEngine.start();
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:stop-proxy', async () => {
  await proxyEngine.stop();
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:set-proxy-mode', async (_event, mode) => {
  await proxyEngine.setMode(mode);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:set-app-policy', async (_event, appKey, policy) => {
  await appProfiler.setPolicy(appKey, policy);
  await proxyEngine.reloadConfig();
  return { profiles: appProfiler.getProfiles(), proxy: await proxyEngine.snapshot() };
});

ipcMain.handle('magic:set-app-group', async (_event, appKey, groupName) => {
  await appProfiler.setGroup(appKey, groupName);
  await proxyEngine.reloadConfig();
  return { profiles: appProfiler.getProfiles(), proxy: await proxyEngine.snapshot() };
});

ipcMain.handle('magic:add-node', async (_event, node) => {
  await proxyEngine.addNode(node);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:remove-node', async (_event, index) => {
  await proxyEngine.removeNode(index);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:import-config', async (_event, text) => {
  await proxyEngine.importConfig(text);
  return proxyEngine.snapshot();
});

ipcMain.handle('magic:export-config', async () => {
  return { text: proxyEngine.exportConfig() };
});

ipcMain.handle('magic:add-server', async (_event, server) => {
  await serverManager.addServer(server);
  return serverManager.snapshot();
});

ipcMain.handle('magic:update-server', async (_event, id, patch) => {
  await serverManager.updateServer(id, patch);
  return serverManager.snapshot();
});

ipcMain.handle('magic:remove-server', async (_event, id) => {
  await serverManager.removeServer(id);
  return serverManager.snapshot();
});

ipcMain.handle('magic:connect-server', async (_event, id) => {
  await serverManager.connect(id);
  return serverManager.snapshot();
});

ipcMain.handle('magic:disconnect-server', async (_event, id) => {
  await serverManager.disconnect(id);
  return serverManager.snapshot();
});

ipcMain.handle('magic:server-write', async (_event, id, data) => {
  serverManager.write(id, data);
  return true;
});

ipcMain.handle('magic:server-resize', async (_event, id, cols, rows) => {
  serverManager.resize(id, cols, rows);
  return true;
});

ipcMain.handle('magic:pick-server', async () => {
  if (!mainWindow) return null;
  try {
    const result = await new Promise((resolve) => {
      execFile('/usr/bin/security', ['find-generic-password', '-s', '魔法代理SSH'], { timeout: 3000 }, (err, stdout) => {
        if (err) return resolve(null);
        resolve(stdout.trim());
      });
    });
    return result;
  } catch (_) {
    return null;
  }
});

ipcMain.handle('magic:set-server-config', async (_event, cfg) => {
  serverManager.setConfig(cfg || {});
  return serverManager.snapshot();
});

ipcMain.handle('magic:get-log', () => proxyEngine.getLogs());

ipcMain.handle('magic:select-config-file', async () => {
  const { dialog } = require('electron');
  if (!mainWindow) return null;
  const r = await dialog.showOpenDialog(mainWindow, {
    title: '选择 Clash / mihomo 配置',
    properties: ['openFile'],
    filters: [{ name: 'YAML', extensions: ['yaml', 'yml'] }]
  });
  if (r.canceled || !r.filePaths[0]) return null;
  const text = fs.readFileSync(r.filePaths[0], 'utf8');
  await proxyEngine.importConfig(text);
  return proxyEngine.snapshot();
});
