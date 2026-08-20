const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const net = require('node:net');

const APP_NAME = '魔法代理';
const MIXED_PORT = 7897;
const API_PORT = 9098;

function defaultNodes() {
  return [
    {
      id: 'node-1',
      name: '搬瓦工直连',
      type: 'vless',
      server: '104.160.40.35',
      port: 443,
      uuid: '268a1166-d31e-478c-a66f-7f9c06c9afaa',
      flow: 'xtls-rprx-vision',
      servername: '',
      clientFingerprint: 'chrome',
      tls: true,
      udp: true,
      publicKey: 'c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0',
      shortId: 'be08e6123ddcaf32'
    },
    {
      id: 'node-2',
      name: 'Texas住宅',
      type: 'vless',
      server: '104.160.40.35',
      port: 443,
      uuid: '7f3e9a2b-4c5d-6e8f-1a2b-3c4d5e6f7a8b',
      flow: 'xtls-rprx-vision',
      servername: '',
      clientFingerprint: 'chrome',
      tls: true,
      udp: true,
      publicKey: 'c7wQJ08b7byOCBzPejQJSwTe8gVN5H4gZxY9vE-k1X0',
      shortId: 'bca4b7cfbcb66d57'
    }
  ];
}

class ProxyEngine {
  constructor(store, appProfiler, onLog) {
    this.store = store;
    this.appProfiler = appProfiler;
    this.onLog = onLog;
    this.child = null;
    this.ready = false;
    this.profileDir = path.join(os.homedir(), '.magic-agent', 'profiles');
    fs.mkdirSync(this.profileDir, { recursive: true });
  }

  log(message) {
    if (typeof this.onLog === 'function') this.onLog(message);
  }

  ensureBundledFiles() {
    const kernel = path.join(process.resourcesPath || '', 'bin', 'mihomo');
    if (fs.existsSync(kernel)) return kernel;
    const local = path.join(__dirname, '..', '..', 'bin', 'mihomo');
    if (fs.existsSync(local)) return local;
    this.log({ level: 'error', text: '未找到 mihomo 内核，请把 mihomo 放到 bin/mihomo' });
    return null;
  }

  _toNodeYaml(node) {
    const base = {
      name: node.name,
      type: node.type || 'vless',
      server: node.server,
      port: Number(node.port || 443),
      uuid: node.uuid || '',
      network: 'tcp',
      tls: true,
      udp: true,
      flow: node.flow || 'xtls-rprx-vision',
      servername: node.servername || '',
      'client-fingerprint': node.clientFingerprint || 'chrome'
    };
    if (node.publicKey && node.shortId) {
      base['reality-opts'] = {
        'public-key': node.publicKey,
        'short-id': node.shortId
      };
    }
    return base;
  }

  _buildConfig() {
    const state = this.store.get('proxy') || {};
    const nodes = (state.nodes && state.nodes.length ? state.nodes : defaultNodes()).map((n) => this._toNodeYaml(n));
    const groups = {
      name: 'GLOBAL',
      type: 'select',
      proxies: nodes.map((n) => n.name)
    };
    const policies = this.appProfiler ? this.appProfiler.getProfiles() : {};
    const rules = [];
    rules.push('IP-CIDR,127.0.0.0/8,DIRECT');
    rules.push('IP-CIDR,192.168.0.0/16,DIRECT');
    rules.push('IP-CIDR,10.0.0.0/8,DIRECT');
    rules.push('IP-CIDR,172.16.0.0/12,DIRECT');
    for (const [appKey, policy] of Object.entries(policies)) {
      if (!policy || policy.policy === '直连' || policy.policy === 'DIRECT') {
        rules.push(`PROCESS-NAME-REGEX,${escapeRegExp(appKey)},DIRECT`);
      }
    }
    if (state.mode === 'global') {
      rules.push('MATCH,GLOBAL');
    } else {
      rules.push('GEOSITE,cn,DIRECT');
      rules.push('GEOIP,CN,DIRECT');
      rules.push('MATCH,GLOBAL');
    }
    const config = {
      'mixed-port': MIXED_PORT,
      'allow-lan': false,
      bind: '127.0.0.1',
      mode: state.mode === 'global' ? 'global' : 'rule',
      'log-level': 'info',
      ipv6: false,
      'external-controller': `127.0.0.1:${API_PORT}`,
      'find-process-mode': 'always',
      proxies: nodes,
      'proxy-groups': [groups],
      rules
    };
    return config;
  }

  writeConfig() {
    const file = path.join(this.profileDir, 'config.yaml');
    fs.writeFileSync(file, yamlStringify(this._buildConfig()), 'utf8');
    return file;
  }

  snapshot() {
    const state = this.store.get('proxy') || {};
    return {
      enabled: this.ready,
      pid: this.child ? this.child.pid : null,
      mode: state.mode || 'auto',
      systemProxy: !!state.systemProxy,
      nodes: state.nodes && state.nodes.length ? state.nodes : defaultNodes(),
      selectedGroup: state.selectedGroup || 'GLOBAL',
      logs: this.getLogs().slice(-60)
    };
  }

  start() {
    if (this.ready) return this.snapshot();
    const kernel = this.ensureBundledFiles();
    if (!kernel) return this.snapshot();
    const file = this.writeConfig();
    this.log({ level: 'info', text: `启动 mihomo: ${file}` });
    this.child = spawn(kernel, ['-f', file], { stdio: ['ignore', 'pipe', 'pipe'] });
    this.child.stdout.on('data', (d) => this.log({ level: 'info', text: d.toString().trim() }));
    this.child.stderr.on('data', (d) => this.log({ level: 'error', text: d.toString().trim() }));
    this.child.on('exit', (code) => {
      this.ready = false;
      this.log({ level: 'warn', text: `mihomo 退出 code=${code}` });
      this.child = null;
    });
    return new Promise((resolve) => {
      const started = Date.now();
      const iv = setInterval(() => {
        const ok = net.connect({ host: '127.0.0.1', port: MIXED_PORT });
        ok.once('connect', () => {
          clearInterval(iv);
          ok.destroy();
          this.ready = true;
          this.store.set('proxy', { ...this.store.get('proxy'), enabled: true });
          this.log({ level: 'success', text: `代理已启动，端口 ${MIXED_PORT}` });
          resolve(this.snapshot());
        });
        ok.once('error', () => {
          if (Date.now() - started > 12000) {
            clearInterval(iv);
            resolve(this.snapshot());
          }
        });
      }, 400);
    });
  }

  stop() {
    if (this.child) {
      try { this.child.kill(); } catch (_) {}
      this.child = null;
    }
    this.ready = false;
    this.store.set('proxy', { ...this.store.get('proxy'), enabled: false });
    this.log({ level: 'info', text: '代理已停止' });
    return this.snapshot();
  }

  setMode(mode) {
    this.store.set('proxy', { ...this.store.get('proxy'), mode });
    this.log({ level: 'info', text: `分流模式：${mode === 'global' ? '全局' : '智能分流'}` });
    return this.snapshot();
  }

  addNode(node) {
    const state = this.store.get('proxy') || {};
    const nodes = state.nodes && state.nodes.length ? state.nodes : defaultNodes();
    nodes.push({ ...node, id: `node-${Date.now()}` });
    this.store.set('proxy', { ...state, nodes });
    return this.snapshot();
  }

  removeNode(index) {
    const state = this.store.get('proxy') || {};
    const nodes = (state.nodes || []).slice();
    nodes.splice(index, 1);
    this.store.set('proxy', { ...state, nodes });
    return this.snapshot();
  }

  importConfig(text) {
    try {
      const obj = JSON.parse(text);
      if (obj && Array.isArray(obj.nodes)) {
        this.store.set('proxy', { ...this.store.get('proxy'), nodes: obj.nodes });
        return this.snapshot();
      }
    } catch (_) {}
    this.store.set('importedConfig', text);
    return this.snapshot();
  }

  exportConfig() {
    return yamlStringify(this._buildConfig());
  }

  reloadConfig() {
    if (this.ready) {
      try { this.child && this.child.kill('SIGHUP'); } catch (_) {}
    }
    return this.snapshot();
  }

  getLogs() {
    return (this.store.get('proxyLogs') || []).map((x) => x);
  }

  init() {
    this.store.set('proxy', { ...this.store.get('proxy'), enabled: false, mode: this.store.get('proxy').mode || 'auto' });
    return this.snapshot();
  }
}

function escapeRegExp(s) {
  return String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function yamlStringify(obj) {
  return 'mixed-port: 7897\nallow-lan: false\nbind-address: 127.0.0.1\nmode: "' + (obj.mode || 'rule') + '"\nlog-level: info\nipv6: false\nexternal-controller: 127.0.0.1:9098\nfind-process-mode: always\nproxies:\n' +
    (obj.proxies || []).map((p) => {
      const lines = ['  - name: "' + p.name + '"', '    type: ' + p.type, '    server: ' + p.server, '    port: ' + p.port, '    uuid: "' + p.uuid + '"', '    network: tcp', '    tls: true', '    udp: true', '    flow: "' + (p.flow || '') + '"', '    servername: "' + (p.servername || '') + '"', '    client-fingerprint: "' + (p['client-fingerprint'] || 'chrome') + '"'];
      if (p['reality-opts']) lines.push('    reality-opts:\n      public-key: "' + p['reality-opts']['public-key'] + '"\n      short-id: "' + p['reality-opts']['short-id'] + '"');
      return lines.join('\n');
    }).join('\n') +
    '\nproxy-groups:\n  - name: GLOBAL\n    type: select\n    proxies:\n' + (obj.proxies || []).map((p) => '      - "' + p.name + '"').join('\n') +
    '\nrules:\n' + (obj.rules || []).map((r) => '  - ' + r).join('\n') + '\n';
}

module.exports = ProxyEngine;
