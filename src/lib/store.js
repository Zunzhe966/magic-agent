const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { app } = require('electron');

class Store {
  constructor() {
    this.dir = app ? path.join(app.getPath('userData'), 'data') : path.join(os.homedir(), '.magic-agent');
    fs.mkdirSync(this.dir, { recursive: true });
    this.file = path.join(this.dir, 'state.json');
    this.data = this.load();
  }

  load() {
    try {
      return JSON.parse(fs.readFileSync(this.file, 'utf8'));
    } catch {
      return {
        proxy: { enabled: false, mode: 'auto', selectedGroup: '🚀 智能分流', nodes: [], systemProxy: true },
        appPolicies: {},
        appGroups: {},
        servers: [],
        serverConfig: {},
        importedConfig: ''
      };
    }
  }

  save() {
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true });
      fs.writeFileSync(this.file, JSON.stringify(this.data, null, 2));
    } catch (err) {
      console.error('Store save error', err);
    }
  }

  get(key) {
    return this.data[key];
  }

  set(key, value) {
    this.data[key] = value;
    this.save();
    return value;
  }
}

module.exports = Store;
