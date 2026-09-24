#!/bin/bash
# ============================================================
# uninstall-privileged-helper.sh — 卸载旧特权控制器（v0.4.1 安全整改）
#
# 背景：v0.4.0 及之前通过「sudoers 免密白名单 + root 控制脚本」以 root
# 启动 mihomo 内核。该架构 = 系统后门级风险（/etc/sudoers.d 下的白名单
# 脚本可被替换，即免密 root 任意命令）。v0.4.1 起内核已用户态化
# （TUN 关闭、全部端口为普通端口、DNS 走 127.0.0.1:1054），root 通道
# 不再需要，必须封死。
#
# 本脚本需要管理员权限执行一次：
#   sudo bash scripts/uninstall-privileged-helper.sh
#
# 删除内容：
#   1. /etc/sudoers.d/magic-agent-mihomo        （免密 sudoers 白名单）
#   2. /etc/sudoers.d/magic-agent-mihomo.tmp    （安装过程的临时文件）
#   3. /usr/local/lib/magic-agent/mihomo-ctl.sh （root 控制脚本）
#   4. /usr/local/lib/magic-agent/              （若空则一并删除）
# 说明：launchd 系统域 party.mihomo.helper（若存在）由旧安装残留，本脚本
# 一并尝试卸载；如无法卸载（SIP/权限），v0.4.1 已不再调用它，无实际影响。
# ============================================================
set -euo pipefail

echo "==> 1/4 删除 sudoers 白名单"
rm -f /etc/sudoers.d/magic-agent-mihomo /etc/sudoers.d/magic-agent-mihomo.tmp
echo "    OK"

echo "==> 2/4 删除 root 控制脚本"
rm -f /usr/local/lib/magic-agent/mihomo-ctl.sh
echo "    OK"

echo "==> 3/4 清理目录（仅空目录时删除）"
rmdir /usr/local/lib/magic-agent 2>/dev/null || true
echo "    OK（目录非空时保留，属无害残留）"

echo "==> 4/4 尝试卸载 launchd 系统域特权助手（若存在）"
if /bin/launchctl print-disabled system/party.mihomo.helper >/dev/null 2>&1; then
    /bin/launchctl bootout system/party.mihomo.helper 2>/dev/null || true
    /bin/launchctl disable system/party.mihomo.helper 2>/dev/null || true
    echo "    OK（已卸载/禁用）"
else
    echo "    （未发现 system/party.mihomo.helper，跳过）"
fi

echo ""
echo "==> 卸载完成。验证："
ls -la /etc/sudoers.d/magic-agent-mihomo 2>/dev/null && echo "  ⚠️  sudoers 仍存在！" || echo "  ✅ sudoers 白名单已删除"
ls -la /usr/local/lib/magic-agent/mihomo-ctl.sh 2>/dev/null && echo "  ⚠️  控制脚本仍存在！" || echo "  ✅ root 控制脚本已删除"
echo ""
echo "现在可打开「尊者网络管理」App 或调用 MCP 启动代理——全程用户态，不再需要任何 root 授权。"
