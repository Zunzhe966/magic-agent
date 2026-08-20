'use strict';

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('magic', {
  getBoot: () => ipcRenderer.invoke('app:get-boot'),
  proxyStart: o => ipcRenderer.invoke('proxy:start', o),
  proxyStop: () => ipcRenderer.invoke('proxy:stop'),
  proxyRestart: () => ipcRenderer.invoke('proxy:restart'),
  proxyStatus: () => ipcRenderer.invoke('proxy:status'),
  proxyLog: () => ipcRenderer.invoke('proxy:log'),
  proxyConnections: () => ipcRenderer.invoke('proxy:connections'),
  proxySelect: n => ipcRenderer.invoke('proxy:select', n),
  settingsGet: () => ipcRenderer.invoke('settings:get'),
  settingsUpdate: p => ipcRenderer.invoke('settings:update', p),
  appsScan: () => ipcRenderer.invoke('apps:scan'),
  sshConnect: s => ipcRenderer.invoke('ssh:connect', s),
  sshDisconnect: id => ipcRenderer.invoke('ssh:disconnect', id),
  sshWrite: (id, d) => ipcRenderer.invoke('ssh:write', id, d),
  sshRun: (id, c) => ipcRenderer.invoke('ssh:run', id, c),
  sshInfo: id => ipcRenderer.invoke('ssh:info', id),
  sshReboot: id => ipcRenderer.invoke('ssh:reboot', id),
  sshPoweroff: id => ipcRenderer.invoke('ssh:poweroff', id),
  dialogOpenJson: () => ipcRenderer.invoke('dialog:open-json'),
  shellOpenPath: p => ipcRenderer.invoke('shell:open-path', p),
  onSshData: cb => ipcRenderer.on('ssh:data', (_e, d) => cb(d))
});
