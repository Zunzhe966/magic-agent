const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('magicAPI', {
  getState: () => ipcRenderer.invoke('magic:get-state'),
  startProxy: () => ipcRenderer.invoke('magic:start-proxy'),
  stopProxy: () => ipcRenderer.invoke('magic:stop-proxy'),
  setProxyMode: (mode) => ipcRenderer.invoke('magic:set-proxy-mode', mode),
  setAppPolicy: (appKey, policy) => ipcRenderer.invoke('magic:set-app-policy', appKey, policy),
  setAppGroup: (appKey, groupName) => ipcRenderer.invoke('magic:set-app-group', appKey, groupName),
  addNode: (node) => ipcRenderer.invoke('magic:add-node', node),
  removeNode: (index) => ipcRenderer.invoke('magic:remove-node', index),
  importConfig: (text) => ipcRenderer.invoke('magic:import-config', text),
  exportConfig: () => ipcRenderer.invoke('magic:export-config'),
  addServer: (server) => ipcRenderer.invoke('magic:add-server', server),
  updateServer: (id, patch) => ipcRenderer.invoke('magic:update-server', id, patch),
  removeServer: (id) => ipcRenderer.invoke('magic:remove-server', id),
  connectServer: (id) => ipcRenderer.invoke('magic:connect-server', id),
  disconnectServer: (id) => ipcRenderer.invoke('magic:disconnect-server', id),
  serverWrite: (id, data) => ipcRenderer.invoke('magic:server-write', id, data),
  serverResize: (id, cols, rows) => ipcRenderer.invoke('magic:server-resize', id, cols, rows),
  setServerConfig: (cfg) => ipcRenderer.invoke('magic:set-server-config', cfg),
  getLog: () => ipcRenderer.invoke('magic:get-log'),
  selectConfigFile: () => ipcRenderer.invoke('magic:select-config-file'),
  onLog: (cb) => ipcRenderer.on('magic:log', (_e, data) => cb(data))
});
