<template>
  <div class="page">
    <header class="page-head">
      <div><h1>域名分流</h1><p>按域名指定流量走向：代理、直连或指定节点</p></div>
    </header>

    <section class="panel">
      <p class="muted" style="margin-bottom: 10px;">
        同一个软件里有的网站走代理、有的走直连？在这里按域名指定。规则<strong>从上到下匹配，首条命中生效</strong>；
        目标可选"代理"（自动选最优节点）、"直连"或某个具体节点。备注写清楚服务于哪个密钥/软件，防止日久失忆误删。
      </p>
      <div class="domain-rule-head">
        <span>域名</span><span>目标</span><span>备注</span><span></span>
      </div>
      <div v-for="(rule, i) in rules" :key="i" class="domain-rule-row">
        <span class="domain-idx">{{ i + 1 }}</span>
        <input v-model="rule.domain" placeholder="github.com" />
        <select v-model="rule.target">
          <option value="proxy">代理</option>
          <option value="direct">直连</option>
          <option v-for="n in (config?.nodes || [])" :key="n.name" :value="n.name">{{ n.name }}</option>
        </select>
        <input v-model="rule.reason" placeholder="服务于哪个密钥/软件（可选）" style="flex: 1; min-width: 140px;" />
        <button class="btn small danger" @click="rules.splice(i, 1)">删</button>
      </div>
      <div style="margin-top: 12px;">
        <button class="btn small" @click="rules.push({ domain: '', target: 'proxy', reason: '' })">+ 加域名</button>
        <button class="btn small primary" style="margin-left: 8px;" @click="save">保存域名规则</button>
        <span v-if="savedTip" class="muted" style="margin-left: 10px;">{{ savedTip }}</span>
      </div>
    </section>
  </div>
</template>
<script setup>
import { ref, watch } from 'vue';
const props = defineProps({ config: Object });
const emit = defineEmits(['update']);
const rules = ref((props.config?.domainRules || []).map(r => ({ ...r })));
const savedTip = ref('');
watch(() => props.config?.domainRules, v => {
  rules.value = (v || []).map(r => ({ ...r }));
  savedTip.value = '';
});
function normalizeDomain(d) {
  let s = (d || '').trim().toLowerCase();
  // 去掉协议前缀与路径，只保留纯域名
  s = s.replace(/^https?:\/\//, '');
  s = s.split('/')[0];
  s = s.split(':')[0];
  // 去掉首尾的点和通配符
  s = s.replace(/^\.+/, '').replace(/\.+$/, '').replace(/^\*\./, '');
  return s.trim();
}
function save() {
  const seen = new Set();
  const list = [];
  for (const r of rules.value) {
    const domain = normalizeDomain(r.domain);
    if (!domain) continue;
    if (seen.has(domain)) continue; // 同域名去重，避免首条命中后其余成死规则
    seen.add(domain);
    list.push({ domain, target: r.target, reason: (r.reason || '').trim() });
  }
  emit('update', { domainRules: list });
  savedTip.value = '已保存 ' + list.length + ' 条规则';
}
</script>
