<template>
  <div class="page">
    <header class="page-head">
      <div><h1>软件分流</h1><p>按软件精确控制流量走向，Chrome 走代理、Safari 直连</p></div>
      <div class="head-actions">
        <button class="btn" @click="$emit('refresh')">重新扫描</button>
        <button class="btn primary" :disabled="saving" @click="save">{{ saving ? '保存中…' : '保存并应用' }}</button>
      </div>
    </header>
    <div class="toolbar">
      <input class="search" v-model="q" placeholder="搜索软件…" />
      <div class="seg">
        <button v-for="m in modes" :key="m.value" class="seg-btn" :class="{ on: filter === m.value }" @click="filter = m.value">{{ m.label }}</button>
      </div>
    </div>
    <p class="muted" style="margin-bottom: 10px;">默认策略：所有软件直连。只有你在下面明确选为「代理」的软件才会走代理。带「联网」标记的软件/脚本当前正在访问网络，是需要关注的对象；「本地」的则未联网，无需处理。</p>
    <div class="apps-table">
      <div class="apps-row head">
        <span>软件</span><span>类别</span><span>模式</span><span>节点</span><span>状态</span><span>路径</span>
      </div>
      <div class="apps-row" v-for="app in filtered" :key="app.id">
        <div class="app-name">
          <div class="app-avatar">{{ app.name.slice(0, 1) }}</div>
          <div><div>{{ app.name }}</div><div class="muted">{{ statusText(app) }}</div></div>
        </div>
        <span class="tag">{{ app.category }}</span>
        <select class="mode-select" v-model="app.mode">
          <option value="proxy">代理</option><option value="direct">直连</option>
        </select>
        <select v-if="app.mode === 'proxy'" class="mode-select" v-model="app.node">
          <option :value="null">当前节点</option>
          <option v-for="n in nodes" :key="n.name" :value="n.name">{{ n.name }}</option>
        </select>
        <span v-else class="muted">—</span>
        <span class="conn-status" :class="connClass(app)">{{ connText(app) }}</span>
        <span class="path muted">{{ app.path }}</span>
      </div>
    </div>
  </div>
</template>
<script setup>
import { ref, computed } from 'vue';
const props = defineProps({ apps: Array, nodes: Array });
const emit = defineEmits(['change', 'refresh']);
const q = ref('');
const filter = ref('all');
// 防连点：applyApps 走父组件 save_config 是异步秒级操作，连点会触发重复 IPC +
// 重复规则热更新（reload_rules 走 osascript 提权）。disabled 期间锁住按钮。
const saving = ref(false);
const modes = [
  { value: 'all', label: '全部' }, { value: 'online', label: '联网中' }, { value: 'proxy', label: '走代理' }, { value: 'direct', label: '直连' }
];
const filtered = computed(() => {
  const query = q.value.toLowerCase();
  return props.apps
    .filter(a => {
      const nameOk = !query || a.name.toLowerCase().includes(query);
      let modeOk = true;
      if (filter.value === 'online') modeOk = !!a.online;
      else if (filter.value === 'proxy') modeOk = a.mode === 'proxy';
      else if (filter.value === 'direct') modeOk = a.mode === 'direct';
      return nameOk && modeOk;
    })
    .sort((a, b) => (b.running ? 1 : 0) - (a.running ? 1 : 0));
});
function statusText(app) {
  if (!app.running) return '未运行';
  if (app.online) return '联网中';
  return '运行中·本地';
}
function connText(app) {
  if (!app.running) return '未运行';
  if (!app.confirmed) return '默认直连';
  if (app.mode === 'proxy') return app.node ? ('代理：' + app.node) : '代理';
  return '直连';
}
function connClass(app) {
  if (!app.running) return 'idle';
  if (!app.confirmed) return 'direct';
  if (app.mode === 'proxy') return 'proxy';
  return 'direct';
}
function save() {
  if (saving.value) return;
  saving.value = true;
  emit('change', props.apps);
  // 父组件 applyApps 是 await save_config + refresh，无法直接 await；
  // 给一个保守的 1.2s 锁定期，避免热更新未完成又被连点。
  setTimeout(() => { saving.value = false; }, 1200);
}
</script>
