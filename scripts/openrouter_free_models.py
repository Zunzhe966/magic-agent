#!/usr/bin/env python3
"""OpenRouter 免费模型台账生成器（给智能体用的管理工具）。

拉取 OpenRouter 全量模型 → 过滤免费的 → 生成两份产物：
  docs/免费模型清单.md    人看的分组台账
  docs/free_models.json   智能体读的结构化索引（MCP list_free_models 工具的数据源）

分流说明：openrouter.ai 已有域名规则固定走节点，本脚本无需任何代理配置。

用法：
  python3 scripts/openrouter_free_models.py            # 刷新台账
  python3 scripts/openrouter_free_models.py --all      # 附带付费模型清单
"""
import argparse
import datetime
import io
import json
import os
import subprocess
import sys
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = os.path.join(ROOT, 'docs')
MD_PATH = os.path.join(DOCS, '免费模型清单.md')
JSON_PATH = os.path.join(DOCS, 'free_models.json')
MODELS_URL = 'https://openrouter.ai/api/v1/models'

# 直连环境走魔法代理的 openrouter.ai 域名规则；沙箱里 TUN 兜底，无需显式代理


def fetch_models(timeout=20):
    req = urllib.request.Request(MODELS_URL, headers={'User-Agent': 'magic-agent-ledger/1.0'})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)['data']


def is_free(m):
    p = m.get('pricing', {})
    return m.get('id', '').endswith(':free') or (p.get('prompt') == '0' and p.get('completion') == '0')


def vendor_of(mid):
    return mid.split('/')[0] if '/' in mid else '其他'


def fmt_ctx(n):
    if not n:
        return '?'
    return f"{n/1024:.0f}K" if n >= 1024 else str(n)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--all', action='store_true', help='附带付费模型清单')
    args = ap.parse_args()

    models = fetch_models()
    free = [m for m in models if is_free(m)]
    free.sort(key=lambda m: vendor_of(m['id']))

    # ── JSON 索引（智能体数据源）──
    snapshot = {
        'updated_at': datetime.datetime.now().isoformat(timespec='seconds'),
        'source': MODELS_URL,
        'total_models': len(models),
        'free_count': len(free),
        'models': [
            {
                'id': m['id'],
                'name': m.get('name', ''),
                'vendor': vendor_of(m['id']),
                'context_length': m.get('context_length'),
                'modality': m.get('architecture', {}).get('modality', ''),
                'description': (m.get('description') or '')[:200],
            }
            for m in free
        ],
    }
    os.makedirs(DOCS, exist_ok=True)
    with open(JSON_PATH, 'w') as f:
        json.dump(snapshot, f, ensure_ascii=False, indent=2)

    # ── Markdown 台账（人看）──
    buf = io.StringIO()
    w = buf.write
    now = snapshot['updated_at']
    w(f"# OpenRouter 免费模型清单\n\n")
    w(f"> 自动生成于 {now}，共 {len(free)} 个免费模型 / 全站 {len(models)} 个。刷新：`python3 scripts/openrouter_free_models.py`\n")
    w(f"> 分流已由域名规则 `openrouter.ai → 示例节点` 覆盖，使用这些模型无需额外配置。\n\n")
    w("## 使用须知\n\n")
    w("- `:free` 模型有每日限额（账户未充值约 50 次/天；充值累计 $10 后提升到 1000 次/天）\n")
    w("- 同一密钥下付费与免费混列，调用时以完整模型 id 为准（如 `deepseek/deepseek-r1:free`）\n")
    w("- 免费模型上下文、质量差异大，按任务选择，长文优先选 context 大的\n\n")

    cur = None
    for m in free:
        v = vendor_of(m['id'])
        if v != cur:
            cur = v
            w(f"\n## {v}\n\n| 模型 id | 名称 | 上下文 | 模态 |\n|---|---|---|---|\n")
        modality = m.get('architecture', {}).get('modality', '').replace('->', '→')
        w(f"| `{m['id']}` | {m.get('name','')} | {fmt_ctx(m.get('context_length'))} | {modality} |\n")

    if args.all:
        paid = [m for m in models if not is_free(m)]
        paid.sort(key=lambda m: float(m.get('pricing', {}).get('prompt') or 0))
        w("\n\n## 附：付费模型（按输入价升序，前 30）\n\n| 模型 id | 输入价 $/M tok | 输出价 $/M tok | 上下文 |\n|---|---|---|---|\n")
        for m in paid[:30]:
            p = m.get('pricing', {})
            w(f"| `{m['id']}` | {float(p.get('prompt') or 0)*1e6:.2f} | {float(p.get('completion') or 0)*1e6:.2f} | {fmt_ctx(m.get('context_length'))} |\n")

    with open(MD_PATH, 'w') as f:
        f.write(buf.getvalue())

    print(f"✅ 台账已更新：{MD_PATH}")
    print(f"✅ 索引已更新：{JSON_PATH}")
    print(f"   免费 {len(free)} / 全站 {len(models)} 个模型")


if __name__ == '__main__':
    sys.exit(main())
