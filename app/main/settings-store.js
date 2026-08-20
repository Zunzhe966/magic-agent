'use strict';

const fs = require('fs');
const path = require('path');
const { app } = require('electron');
const { KNOWN_PROXIES, DEFAULT_APPS } = require('../shared/defaults');

const DEFAULT_SETTINGS = {
  autoStart: false,
  systemProxy: true,
  exitPolicy: 'MAGIC-EXIT',
  proxies: KNOWN_PROXIES,
  apps: DEFAULT_APPS,
  servers: [],
  theme: 'dark'
};

class SettingsStore {
  constructor() {
    this.file = path.join(app.getPath('userData'), 'settings.json');
    this.settings = this.load();
  }

  load() {
    try {
      const raw = JSON.parse(fs.readFileSync(this.file, 'utf8'));
      return { ...JSON.parse(JSON.stringify(DEFAULT_SETTINGS)), ...raw };
    } catch {
      return JSON.parse(JSON.stringify(DEFAULT_SETTINGS));
    }
  }

  get() {
    return JSON.parse(JSON.stringify(this.settings));
  }

  update(patch) {
    const next = { ...this.settings, ...patch };
    if (patch.apps) {
      const ids = new Set();
      next.apps = patch.apps.map(a => ({ ...a, id: a.id || `app-${Date.now()}-${Math.random().toString(36).slice(2, 7)}` }));
      next.apps = next.apps.filter(a => !ids.has(a.id) ? (ids.add(a.id), true) : false);
    }
    if (patch.proxies) next.proxies = patch.proxies;
    if (patch.servers) next.servers = patch.servers;
    this.settings = next;
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true });
      fs.writeFileSync(this.file, JSON.stringify(this.settings, null, 2), 'utf8');
    } catch {}
    return this.get();
  }
}

module.exports = { SettingsStore };
