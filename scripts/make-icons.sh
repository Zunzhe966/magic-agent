#!/bin/bash
set -euo pipefail

# 图标唯一源：src-tauri/icons/source-icon.png（正方形 PNG，建议 1024px 以上）。
# 所有平台资源都由该母版派生，禁止手工维护各尺寸文件。
#
# 两步走：
#   1. cargo tauri icon —— Windows/Linux 用的方形 PNG/ICO（这些平台要方形）
#   2. icon_build.py    —— macOS 松鼠形圆角 icns、iOS 去 alpha、
#                          Android 自适应安全区+底色（Tauri CLI 做不了这些）
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="${1:-$ROOT/src-tauri/icons/source-icon.png}"
OUTPUT="$ROOT/src-tauri/icons"

if [ ! -f "$SOURCE" ]; then
  echo "找不到图标母版: $SOURCE" >&2
  exit 1
fi

WIDTH="$(sips -g pixelWidth "$SOURCE" 2>/dev/null | awk '/pixelWidth/{print $2}')"
HEIGHT="$(sips -g pixelHeight "$SOURCE" 2>/dev/null | awk '/pixelHeight/{print $2}')"
if [ -z "$WIDTH" ] || [ -z "$HEIGHT" ] || [ "$WIDTH" != "$HEIGHT" ]; then
  echo "图标母版必须是正方形 PNG: ${WIDTH:-?}x${HEIGHT:-?}" >&2
  exit 1
fi
if [ "$WIDTH" -lt 1024 ]; then
  echo "图标母版分辨率不足: ${WIDTH}x${HEIGHT}，至少需要 1024x1024" >&2
  exit 1
fi

echo "==> 从图标母版生成全平台资源: $SOURCE"
cd "$ROOT/src-tauri"
cargo tauri icon "$SOURCE" --output "$OUTPUT"

echo "==> 平台规范化处理（macOS 圆角 / iOS 去 alpha / Android 安全区）"
python3 "$ROOT/scripts/icon_build.py"

echo "==> 图标已生成"
shasum -a 256 "$SOURCE" "$OUTPUT/icon.icns"
