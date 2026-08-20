'use strict';

const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');
const http = require('http');
const net = require('net');
const dgram = require('dgram');
const { app } = require('electron');
const { KNOWN_PROXIES, PROXY_PORTS, CONTROLLER_SECRET } = require('../shared/defaults');

const GEO_DIR = path.join(__dirname, '..', 'core', 'geo');
const GEO_FILES = {
  geosite: 'geosite.dat',
  geoip: 'geoip.dat',
  mmdb: 'geoip.metadb',
  asn: 'asn.mmdb'
};
const BIN_DIR = path.join(__dirname, '..', 'core', 'bin');
const BIN = path.join(BIN_DIR, os.platform() === 'win32' ? 'mihomo.exe' : 'mihomo');

function quoteYaml(s) {
  const str = String(s ?? '');
  return `"${str.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

function copyGeoFiles(runtimeDir) {
  try {
    fs.mkdirSync(runtimeDir, { recursive: true });
    for (const [src, dst] of Object.entries(GEO_FILES)) {
      const from = path.join(GEO_DIR, src);
      const to = path.join(runtimeDir, dst);
      if (fs.existsSync(from)) fs.copyFileSync(from, to);
    }
  } catch (e) {
    console.error('copy geo files failed', e);
  }
}

function findFreePort(preferred, udp) {
  return new Promise(resolve => {
    if (udp) {
      const sock = dgram.createSocket('udp4');
      sock.on('error', () => resolve(null));
      sock.bind(preferred, '127.0.0.1', () => {
        const port = sock.address().port;
        sock.close(() => resolve(port));
      });
    } else {
      const srv = net.createServer();
      srv.on('error', () => resolve(null));
      srv.listen(preferred, '127.0.0.1', () => {
        const port = srv.address().port;
        srv.close(() => resolve(port));
      });
    }
  });
}

async function resolvePorts(settings) {
  const preferred = { ...PROXY_PORTS, ...(settings.ports || {}) };
  const httpPort = await findFreePort(preferred.http || 0) || await findFreePort(0);
  const socksPort = await findFreePort(preferred.socks || 0) || await findFreePort(0);
  const controllerPort = await findFreePort(preferred.controller || 0) || await findFreePort(0);
  const dnsPort = await findFreePort(preferred.dns || 0, true) || await findFreePort(0, true);
  return { http: httpPort, socks: socksPort, controller: controllerPort, dns: dnsPort };
}

function buildMihomoConfig(appRules, settings, ports) {
  const proxies = (settings.proxies && settings.proxies.length ? settings.proxies : KNOWN_PROXIES).map(p => {
    const base = {
      name: p.name,
      type: p.type,
      server: p.server,
      port: p.port,
      uuid: p.uuid,
      network: p.network || 'tcp',
      tls: p.tls !== false,
      udp: true,
      flow: p.flow || 'xtls-rprx-vision',
      'client-fingerprint': p.clientFingerprint || 'chrome',
      servername: p.servername || '',
      'skip-cert-verify': false
    };
    if (p.realityOpts) {
      base['reality-opts'] = {
        'public-key': p.realityOpts.publicKey,
        'short-id': p.realityOpts.shortId
      };
    }
    return base;
  });

  const proxyNames = proxies.map(p => p.name);
  const selectGroup = {
    name: 'MAGIC-EXIT',
    type: 'select',
    proxies: [...proxyNames, 'DIRECT']
  };

  const rules = [];
  rules.push('RULE-SET,lan,DIRECT');
  for (const r of appRules) {
    if (!r.enabled || !r.regex) continue;
    const target = r.policy === 'proxy' ? 'MAGIC-EXIT' : 'DIRECT';
    rules.push(`PROCESS-NAME-REGEX,${r.regex.replace(/,/g, '\\,')},${target}`);
  }
  rules.push('GEOSITE,cn,DIRECT');
  rules.push('GEOIP,CN,DIRECT');
  rules.push('MATCH,MAGIC-EXIT');

  const cfg = {
    'mixed-port': ports.http,
    'socks-port': ports.socks,
    'allow-lan': false,
    mode: 'rule',
    'log-level': 'info',
    'find-process-mode': 'always',
    ipv6: false,
    'external-controller': `127.0.0.1:${ports.controller}`,
    secret: CONTROLLER_SECRET,
    'geodata-mode': true,
    'geodata-loader': 'memconservative',
    'geox-url': {
      mmdb: 'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geoip.metadb',
      asn: 'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/GeoLite2-ASN.mmdb',
      geoip: 'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geoip.dat',
      geosite: 'https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/geosite.dat'
    },
    proxies,
    'proxy-groups': [selectGroup],
    'rule-providers': {
      lan: {
        type: 'inline',
        behavior: 'ipcidr',
        payload: [
          '127.0.0.0/8',
          '10.0.0.0/8',
          '172.16.0.0/12',
          '192.168.0.0/16',
          '169.254.0.0/16',
          '224.0.0.0/4'
        ]
      }
    },
    rules,
    dns: {
      enable: true,
      listen: `127.0.0.1:${ports.dns}`,
      ipv6: false,
      'enhanced-mode': 'fake-ip',
      'fake-ip-range': '198.18.0.1/16',
      'fake-ip-filter': ['*.lan', 'localhost.ptlogin2.qq.com'],
      nameserver: ['https://doh.pub/dns-query', 'https://dns.alidns.com/dns-query', 'system://'],
      fallback: ['tls://8.8.4.4', 'tls://1.1.1.1'],
      'fallback-filter': { geoip: true, 'geoip-code': 'CN' },
      'proxy-server-nameserver': ['https://doh.pub/dns-query']
    },
    tun: { enable: false },
    profile: { 'store-selected': true, 'store-fake-ip': true }
  };

  return stringifyYaml(cfg);
}

function stringifyYaml(obj, indent) {
  indent = indent || 0;
  const pad = ' '.repeat(indent);
  const out = [];
  for (const [k, v] of Object.entries(obj)) {
    if (v === null || v === undefined) continue;
    if (Array.isArray(v)) {
      if (v.length === 0) continue;
      out.push(`${pad}${k}:`);
      for (const item of v) {
        if (typeof item === 'object' && item !== null) {
          out.push(`${pad}  - ` + stringifyYaml(item, indent + 4).trimStart());
        } else {
          out.push(`${pad}  - ${yamlScalar(item)}`);
        }
      }
    } else if (typeof v === 'object') {
      out.push(`${pad}${k}:`);
      out.push(stringifyYaml(v, indent + 2));
    } else {
      out.push(`${pad}${k}: ${yamlScalar(v)}`);
    }
  }
  return out.join('\n');
}

function yamlScalar(v) {
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (typeof v === 'number') return String(v);
  return quoteYaml(v);
}

class ProxyEngine {
  constructor(settingsStore) {
    this.settingsStore = settingsStore;
    this.proc = null;
    this.startedAt = null;
    this.ports = { ...PROXY_PORTS };
    this.runtimeDir = path.join(app.getPath('userData'), 'runtime');
    this.configPath = path.join(this.runtimeDir, 'config.yaml');
    this.logPath = path.join(this.runtimeDir, 'mihomo.log');
  }

  async start() {
    if (this.proc) return { ok: true, alreadyRunning: true };
    fs.mkdirSync(this.runtimeDir, { recursive: true });
    copyGeoFiles(this.runtimeDir);
    const settings = this.settingsStore.get();
    this.ports = await resolvePorts(settings);
    const yaml = buildMihomoConfig(settings.apps || [], settings, this.ports);
    fs.writeFileSync(this.configPath, yaml, 'utf8');
    const logFd = fs.openSync(this.logPath, 'a');
    const proc = spawn(BIN, ['-d', this.runtimeDir, '-f', this.configPath], {
      stdio: ['ignore', logFd, logFd],
      detached: false
    });
    this.proc = proc;
    this.startedAt = new Date();
    proc.on('exit', code => {
      if (this.proc === proc) {
        this.proc = null;
        this.startedAt = null;
      }
    });
    await waitForController(this.ports.controller);
    return { ok: true, pid: proc.pid };
  }

  async stop() {
    if (!this.proc) return { ok: true, alreadyStopped: true };
    const proc = this.proc;
    this.proc = null;
    try {
      proc.kill('SIGTERM');
      await Promise.race([
        new Promise(r => proc.once('exit', r)),
        new Promise(r => setTimeout(r, 2500))
      ]);
    } finally {
      if (!proc.killed && !proc.exitCode) proc.kill('SIGKILL');
      this.startedAt = null;
    }
    return { ok: true };
  }

  restart() {
    return this.stop().then(() => this.start());
  }

  isRunning() {
    return Boolean(this.proc && !this.proc.exitCode);
  }

  getStatus() {
    return {
      running: this.isRunning(),
      pid: this.proc ? this.proc.pid : null,
      startedAt: this.startedAt ? this.startedAt.toISOString() : null,
      configPath: this.configPath,
      logPath: this.logPath,
      ports: this.ports
    };
  }

  async readLog() {
    try {
      const s = fs.readFileSync(this.logPath, 'utf8');
      return s.split('\n').filter(Boolean).slice(-300);
    } catch {
      return [];
    }
  }

  async connections() {
    try {
      const data = await controllerRequest('/connections', {}, this.ports.controller);
      return data.connections || [];
    } catch {
      return [];
    }
  }

  async selectProxy(name) {
    const settings = this.settingsStore.get();
    const proxies = (settings.proxies || []).map(p => p.name);
    if (proxies.includes(name) || name === 'DIRECT') {
      await controllerRequest('/proxies/MAGIC-EXIT', { method: 'PUT', body: { name } }, this.ports.controller);
      return { ok: true };
    }
    return { ok: false, error: '代理不存在' };
  }

  async getSelected() {
    try {
      const data = await controllerRequest('/proxies/MAGIC-EXIT', {}, this.ports.controller);
      return data.now || null;
    } catch {
      return null;
    }
  }
}

function waitForController(controllerPort) {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const timer = setInterval(async () => {
      try {
        await controllerRequest('/version', {}, controllerPort);
        clearInterval(timer);
        resolve();
      } catch (e) {
        if (Date.now() - started > 15000) {
          clearInterval(timer);
          reject(new Error('内核启动超时: ' + e.message));
        }
      }
    }, 150);
  });
}

function controllerRequest(pathname, { method = 'GET', body } = {}, controllerPort) {
  return new Promise((resolve, reject) => {
    const req = http.request({
      host: '127.0.0.1',
      port: controllerPort || PROXY_PORTS.controller,
      path: pathname,
      method,
      headers: {
        Authorization: `Bearer ${CONTROLLER_SECRET}`,
        'Content-Type': 'application/json'
      },
      timeout: 2500
    }, res => {
      let data = '';
      res.on('data', c => (data += c));
      res.on('end', () => {
        try {
          resolve(JSON.parse(data));
        } catch {
          resolve({});
        }
      });
    });
    req.on('error', reject);
    req.on('timeout', () => req.destroy(new Error('timeout')));
    if (body) req.write(JSON.stringify(body));
    req.end();
  });
}

module.exports = { ProxyEngine, buildMihomoConfig };
