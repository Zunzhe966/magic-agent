<template>
  <div class="page">
    <header class="page-head">
      <div><h1>云服务器</h1><p>管理代理节点与 SSH 连接信息</p></div>
      <div class="head-actions"><button class="btn primary" @click="showAdd = !showAdd">{{ showAdd ? '收起' : '添加节点' }}</button></div>
    </header>

    <!-- 订阅 URL -->
    <section class="panel">
      <div class="panel-head"><h2>订阅</h2></div>
      <div class="sub-row">
        <input class="search" v-model="subUrl" placeholder="粘贴 VLESS 订阅链接（可选）" />
        <button class="btn" @click="saveSub">保存订阅</button>
        <button class="btn primary" @click="pullSub" :disabled="!subUrl">拉取节点</button>
      </div>
      <p class="muted">订阅支持自动拉取多个 VLESS 节点；留空则只使用下方手动节点。</p>
    </section>

    <!-- 添加节点表单 -->
    <section class="panel" v-if="showAdd">
      <div class="panel-head"><h2>添加 VLESS 节点</h2></div>
      <div class="node-form">
        <div class="field"><label>名称</label><input v-model="form.name" placeholder="例如 香港-01" /></div>
        <div class="field"><label>服务器</label><input v-model="form.server" placeholder="104.160.40.35" /></div>
        <div class="field"><label>端口</label><input v-model.number="form.port" type="number" /></div>
        <div class="field"><label>UUID</label><input v-model="form.uuid" /></div>
        <div class="field"><label>Flow</label><input v-model="form.flow" placeholder="xtls-rprx-vision" /></div>
        <div class="field"><label>SNI</label><input v-model="form.sni" /></div>
        <div class="field"><label>Public Key</label><input v-model="form.publicKey" /></div>
        <div class="field"><label>Short ID</label><input v-model="form.shortId" /></div>
        <div class="field"><label>Fingerprint</label><input v-model="form.fingerprint" placeholder="chrome" /></div>
        <div class="field-actions">
          <button class="btn primary" @click="addNode">添加</button>
          <button class="btn" @click="showAdd = false">取消</button>
        </div>
      </div>
    </section>

    <!-- 节点列表 -->
    <div class="server-grid">
      <div class="server-card" v-for="(node, i) in (config?.nodes || [])" :key="i">
        <div class="server-top">
          <div class="server-avatar">云</div>
          <div><div class="server-name">{{ node.name }}</div><div class="muted">{{ node.server }}:{{ node.port }}</div></div>
          <span class="status-pill" :class="{ ok: config?.selectedNode === node.name }">{{ config?.selectedNode === node.name ? '当前' : '备用' }}</span>
        </div>
        <div class="kv"><span>协议</span><b>VLESS + Reality</b></div>
        <div class="kv"><span>UUID</span><b class="mono">{{ node.uuid }}</b></div>
        <div class="kv"><span>Public Key</span><b class="mono">{{ node.publicKey }}</b></div>
        <div class="kv"><span>Short ID</span><b class="mono">{{ node.shortId }}</b></div>
        <div class="card-actions">
          <button class="btn small" @click="selectNode(node)">设为当前</button>
          <button class="btn small" @click="testDelay(node)">测延迟</button>
          <button class="btn small danger" @click="removeNode(i)">删除</button>
        </div>
        <div v-if="delays[node.name]" class="delay-line" :class="delayClass(delays[node.name])">{{ delays[node.name] }} ms</div>
      </div>
    </div>

    <section class="panel" v-if="config?.servers?.length">
      <div class="panel-head"><h2>已保存 SSH 服务器</h2></div>
      <div class="saved-server-list">
        <div class="saved-server" v-for="server in config.servers" :key="server.id" :class="{ active: config.activeServerId === server.id }">
          <div>
            <strong>{{ server.name }}</strong>
            <span class="muted">{{ server.user }}@{{ server.host }}:{{ server.port }}</span>
          </div>
          <div class="card-actions">
            <button class="btn small" @click="$emit('select-server', server.id)">使用</button>
            <button class="btn small danger" @click="$emit('delete-server', server.id)">删除</button>
          </div>
        </div>
      </div>
    </section>

    <section class="panel">
      <div class="panel-head"><h2>域名分流</h2></div>
      <p class="muted" style="margin-bottom: 8px;">同一个软件下载时有的走代理有的走直连？在这里按域名指定。一行一个域名，选"代理"或"直连"。</p>
      <div v-for="(rule, i) in domainRules" :key="i" class="domain-rule-row">
        <input v-model="rule.domain" placeholder="github.com" />
        <select v-model="rule.target">
          <option value="proxy">代理</option>
          <option value="direct">直连</option>
        </select>
        <button class="btn small danger" @click="removeDomainRule(i)">删</button>
      </div>
      <button class="btn small" @click="domainRules.push({ domain: '', target: 'proxy' })">+ 加域名</button>
      <button class="btn small primary" style="margin-left: 8px;" @click="saveDomainRules">保存域名规则</button>
    </section>

    <section class="panel">
      <div class="panel-head"><h2>SSH 控制</h2><button class="btn" @click="$emit('nav')">打开控制台</button></div>
      <p class="muted">通过 SSH 直接控制云服务器：执行命令、安装软件、查看日志。</p>
    </section>
  </div>
</template>
<script setup>
import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
const props = defineProps({ config: Object, status: Object });
const emit = defineEmits(['nav', 'update', 'select-server', 'delete-server']);
const showAdd = ref(false);
const subUrl = ref(props.config?.subscriptionUrl || '');
const domainRules = ref((props.config?.domainRules || []).map(r => ({ ...r })));
watch(() => props.config?.domainRules, v => { domainRules.value = (v || []).map(r => ({ ...r })); });
function removeDomainRule(i) { domainRules.value.splice(i, 1); }
function saveDomainRules() {
  const rules = domainRules.value.filter(r => r.domain.trim());
  emit('update', { domainRules: rules });
}
const form = ref({
  name: '', server: '', port: 443, uuid: '',
  flow: 'xtls-rprx-vision', sni: '', publicKey: '', shortId: '', fingerprint: 'chrome',
});
watch(() => props.config?.subscriptionUrl, v => { subUrl.value = v || ''; });
function saveSub() {
  emit('update', { subscriptionUrl: subUrl.value.trim() || null });
}
async function pullSub() {
  const url = subUrl.value.trim();
  if (!url) return;
  try {
    const nodes = await invoke('fetch_subscription', { url });
    const existing = [...(props.config?.nodes || [])];
    // 合并：按 server+port 去重，新节点追加
    const seen = new Set(existing.map(n => n.server + ':' + n.port));
    const added = [];
    for (const n of nodes) {
      const key = n.server + ':' + n.port;
      if (!seen.has(key)) { seen.add(key); added.push(n); }
    }
    const merged = [...existing, ...added];
    emit('update', { nodes: merged });
    alert('拉取成功：新增 ' + added.length + ' 个节点，共 ' + merged.length + ' 个');
  } catch (e) {
    alert('拉取失败：' + e);
  }
}
function addNode() {
  const nodes = [...(props.config?.nodes || [])];
  nodes.push({
    name: form.value.name.trim() || `节点${nodes.length + 1}`,
    server: form.value.server.trim(),
    port: form.value.port || 443,
    uuid: form.value.uuid.trim(),
    flow: form.value.flow.trim() || 'xtls-rprx-vision',
    network: 'tcp', tls: true, udp: true,
    fingerprint: form.value.fingerprint.trim() || 'chrome',
    publicKey: form.value.publicKey.trim(),
    shortId: form.value.shortId.trim(),
    sni: form.value.sni.trim(),
  });
  emit('update', { nodes });
  form.value = { name: '', server: '', port: 443, uuid: '', flow: 'xtls-rprx-vision', sni: '', publicKey: '', shortId: '', fingerprint: 'chrome' };
}
const delays = ref({});
async function testDelay(node) {
  try {
    const url = 'http://127.0.0.1:19091/proxies/' + encodeURIComponent(node.name) + '/delay?timeout=5000&url=http://www.gstatic.com/generate_204';
    const resp = await fetch(url);
    const data = await resp.json();
    delays.value[node.name] = data.delay;
  } catch (e) {
    delays.value[node.name] = -1;
  }
}
function delayClass(v) {
  if (v < 0) return 'delay-bad';
  if (v < 800) return 'delay-good';
  if (v < 2000) return 'delay-mid';
  return 'delay-bad';
}
function selectNode(node) {
  emit('update', { selectedNode: node.name });
}
function removeNode(i) {
  const nodes = [...(props.config?.nodes || [])];
  nodes.splice(i, 1);
  emit('update', { nodes });
}
</script>
