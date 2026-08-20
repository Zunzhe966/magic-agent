'use strict';

const { Client } = require('ssh2');
const fs = require('fs');
const os = require('os');
const path = require('path');

class SshManager {
  constructor() {
    this.clients = new Map();
    this.pty = new Map();
  }

  async connect(server, sendLine) {
    const id = server.id;
    if (this.clients.has(id)) return { ok: true, connected: true, id };
    return new Promise((resolve) => {
      const conn = new Client();
      const config = {
        host: server.host,
        port: server.port || 22,
        username: server.username,
        readyTimeout: 8000,
        keepaliveInterval: 15000,
        keepaliveCountMax: 3
      };
      if (server.auth === 'key') {
        try {
          config.privateKey = fs.readFileSync(server.privateKeyPath || path.join(os.homedir(), '.ssh', 'id_ed25519'), 'utf8');
        } catch (e) {
          return resolve({ ok: false, error: '私钥读取失败: ' + e.message });
        }
      } else {
        config.password = server.password || '';
      }
      conn.on('ready', () => {
        this.clients.set(id, conn);
        resolve({ ok: true, connected: true, id });
      });
      conn.on('error', e => resolve({ ok: false, error: e.message }));
      conn.on('close', () => this.clients.delete(id));
      conn.connect(config);
    });
  }

  disconnect(id) {
    const conn = this.clients.get(id);
    if (conn) conn.end();
    this.clients.delete(id);
    return { ok: true };
  }

  async runCommand(id, command) {
    const conn = this.clients.get(id);
    if (!conn) return { ok: false, error: '未连接' };
    return new Promise((resolve) => {
      conn.exec(command, (err, stream) => {
        if (err) return resolve({ ok: false, error: err.message });
        let out = '';
        let errOut = '';
        stream.on('close', (code) => resolve({ ok: true, code, stdout: out, stderr: errOut }));
        stream.on('data', d => (out += d.toString()));
        stream.stderr.on('data', d => (errOut += d.toString()));
      });
    });
  }

  openShell(id, sendLine) {
    const conn = this.clients.get(id);
    if (!conn) return { ok: false, error: '未连接' };
    if (this.pty.has(id)) return { ok: true, alreadyOpen: true };
    conn.shell({ term: 'xterm-256color', rows: 30, cols: 110 }, (err, stream) => {
      if (err) {
        sendLine({ type: 'ssh:output', id, text: `shell error: ${err.message}\r\n` });
        return;
      }
      this.pty.set(id, stream);
      stream.on('data', d => sendLine({ type: 'ssh:output', id, text: d.toString('utf8') }));
      stream.on('close', () => this.pty.delete(id));
    });
    return { ok: true };
  }

  writeShell(id, data) {
    const stream = this.pty.get(id);
    if (!stream) return { ok: false, error: '终端未打开' };
    stream.write(data);
    return { ok: true };
  }
}

module.exports = { SshManager };
