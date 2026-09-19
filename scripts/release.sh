#!/bin/bash
set -euo pipefail

VER="${1:?usage: release.sh <version>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONF="$ROOT/src-tauri/tauri.conf.json"
CARGO="$ROOT/src-tauri/Cargo.toml"
UIPKG="$ROOT/ui/package.json"
KEY="$HOME/.tauri/magic-agent.key"

if [ ! -f "$KEY" ]; then
  echo "私钥不存在: $KEY（先用 tauri signer generate -w $KEY 生成）" >&2
  exit 1
fi

echo "==> 设置版本号 $VER"
# 用 jq 改 tauri.conf.json（避免 mv 跨设备权限问题，先写临时再 cat 覆盖）
jq --arg v "$VER" '.version=$v' "$CONF" > "$CONF.tmp"
cat "$CONF.tmp" > "$CONF"
rm -f "$CONF.tmp"
# 改 Cargo.toml 和 ui/package.json 的 version
sed -i '' -e "s/^version = \".*\"/version = \"$VER\"/" "$CARGO"
sed -i '' -e "s/\"version\": \".*\"/\"version\": \"$VER\"/" "$UIPKG"

echo "==> 构建 + 签名（私钥: ${KEY}）"
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
cd "$ROOT/src-tauri" && cargo tauri build

BUNDLE_DIR="$ROOT/src-tauri/target/release/bundle/macos"
TARGZ="$(ls "$BUNDLE_DIR"/*.app.tar.gz | head -1)"
SIG="$TARGZ.sig"
if [ ! -f "$SIG" ]; then
  echo "签名文件未生成，检查 TAURI_SIGNING_PRIVATE_KEY 是否正确" >&2
  exit 1
fi

FEED="$ROOT/scripts/updater-feed"
mkdir -p "$FEED"
cp "$TARGZ" "$FEED/"
cp "$SIG" "$FEED/"

TARGZ_BASE="$(basename "$TARGZ")"
SIG_CONTENT="$(cat "$FEED/$TARGZ_BASE.sig")"
PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat > "$FEED/latest.json" <<JSON
{
  "version": "$VER",
  "notes": "release $VER",
  "pub_date": "$PUB_DATE",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$SIG_CONTENT",
      "url": "http://127.0.0.1:7878/$TARGZ_BASE"
    }
  }
}
JSON

echo ""
echo "feed 已生成于: $FEED"
echo "  latest.json 指向版本: $VER"
echo "  启动本地 server:"
echo "    cd \"$FEED\" && python3 -m http.server 7878 --bind 127.0.0.1"
