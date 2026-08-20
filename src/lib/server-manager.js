const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');

class ServerManager {
  constructor(store, onLog) {
    this.store = store;
    this.onLog = onLog;
    this.sessions = new Map();
    this.config = store.get('serverConfig') || {};
  }

  log(msg) {
    if (typeof this.onLog === 'function') this.onLog(msg);
  }

  _servers() {
    return this.store.get('servers') || [];
  }

  snapshot() {
    return {
      servers: this._servers().map((s) => {
        const live = this.sessions.get(s.id);
        return { ...s, connected: !!live && live.connected, lastEvent: live ? live.lastEvent : null };
      }),
      config: this.config
    };
  }

  addServer(server) {
    const servers = this._servers().slice();
    const id = server.id || `srv-${Date.now()}`;
    servers.push({ ...server, id });
    this.store.set('servers', servers);
    this.log({ level: 'success', text: `已添加服务器 ${server.name || id}` });
    return this.snapshot();
  }

  updateServer(id, patch) {
    const servers = this._servers().map((s) => (s.id === id ? { ...s, ...patch, id } : s));
    this.store.set('servers', servers);
    return this.snapshot();
  }

  removeServer(id) {
    this.disconnect(id);
    this.store.set('servers', this._servers().filter((s) => s.id !== id));
    return this.snapshot();
  }

  connect(id) {
    const server = this._servers().find((s) => s.id === id);
    if (!server) throw new Error('服务器不存在');
    const args = [
      '-p', String(server.port || 22),
      '-o', 'StrictHostKeyChecking=no',
      '-o', 'UserKnownHostsFile=/dev/null',
      '-o', 'ServerAliveInterval=15',
      '-o', 'ServerAliveCountMax=3'
    ];
    if (server.privateKey) args.push('-i', server.privateKey);
    const sshTarget = `${server.username}@${server.host}`;
    this.log({ level: 'info', text: `连接 ${server.name} (${sshTarget}:${server.port || 22})` });
    const child = spawn('ssh', args.concat([sshTarget]), { stdio: ['pipe', 'pipe', 'pipe'] });
    const session = { child, connected: false, buffer: '', lastEvent: 'connecting', type: server.type || 'ssh' };
    this.sessions.set(id, session);
    child.stdout.on('data', (d) => {
      session.buffer = (session.buffer + d.toString()).slice(-8000);
      session.lastEvent = 'output';
      this._emit(id, 'output', d.toString());
    });
    child.stderr.on('data', (d) => {
      session.buffer = (session.buffer + d.toString()).slice(-8000);
      const s = d.toString();
      if (/password:|password for/i.test(s)) session.lastEvent = 'need-password';
      if (/WARNING: REMOTE HOST IDENTIFICATION/i.test(s)) session.lastEvent = 'host-key';
      if (/Enter passphrase/i.test(s)) session.lastEvent = 'need-passphrase';
      this._emit(id, 'stderr', s);
    });
    child.on('exit', (code) => {
      session.connected = false;
      session.lastEvent = `exit-${code}`;
      this._emit(id, 'exit', `退出 code=${code}`);
      this.sessions.delete(id);
    });
    setTimeout(() => {
      if (this.sessions.has(id) && !session.connected && session.buffer.length === 0) {
        session.connected = true;
        session.lastEvent = 'connected';
        this._emit(id, 'connected', '已连接');
      }
    }, 600);
    return this.snapshot();
  }

  disconnect(id) {
    const session = this.sessions.get(id);
    if (session && session.child) {
      try { session.child.kill('SIGHUP'); } catch (_) {}
      session.connected = false;
      session.lastEvent = 'disconnected';
      this._emit(id, 'disconnected', '已断开');
    }
    this.sessions.delete(id);
    return this.snapshot();
  }

  write(id, data) {
    const session = this.sessions.get(id);
    if (session && session.child && session.child.stdin.writable) {
      session.child.stdin.write(data);
      session.lastEvent = 'input';
    }
    return true;
  }

  resize(id, cols, rows) {
    const session = this.sessions.get(id);
    if (session && session.child && session.child.stdout.setEncoding) {
      try { session.child.stdout.setEncoding('utf8'); } catch (_) {}
    }
    return true;
  }

  setConfig(cfg) {
    this.config = cfg || {};
    this.store.set('serverConfig', this.config);
    return this.snapshot();
  }

  _emit(id, type, data) {
    const cb = this.config.onEvent;
    if (typeof cb === 'function') {
      try { cb({ id, type, data }); } catch (_) {}
    }
    this.log({ level: 'info', text: `[${type}] ${String(data).slice(0, 120)}` });
  }

  dispose() {
    for (const id of Array.from(this.sessions.keys())) this.disconnect(id);
  }

  init() {
    return this.snapshot();
  }
}

module.exports = ServerManager;
