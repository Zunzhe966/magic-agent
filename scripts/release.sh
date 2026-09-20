#!/bin/bash
set -euo pipefail

# 发版脚本：构建 + 签名 + 生成更新 feed +（可选）发布到 GitHub Release
#
# 用法：
#   ./scripts/release.sh <version> [--publish]
#
#   <version>   形如 0.2.6
#   --publish   构建完成后，把更新包上传到 GitHub Release（真用户通道）
#
# ── 设计原则：一个源，两份 feed ────────────────────────────
# 包只构建一次、只签名一次；生成的 feed 也只有一份内容，
# 只是把里面的「包下载地址」换成两种写法：
#   * 本地测试 feed：http://127.0.0.1:7878/<包名>        ← 开发者自己用
#   * 公开发布 feed：https://github.com/.../releases/download/v<ver>/<包名>  ← 真用户用
# 两边 signature 完全相同（同一把私钥签的），App 两个通道也用同一把公钥验签。
# 所以「本地」和「GitHub」永远是同一份东西，不存在不同步的问题。
#
# 本地测试流程：
#   1) ./scripts/release.sh 0.2.6
#   2) cd scripts/updater-feed && python3 -m http.server 7878 --bind 127.0.0.1
#   3) App 设置页把通道切到「本地开发测试」→ 检查更新
#
# 正式发布流程：
#   ./scripts/release.sh 0.2.6 --publish
#   之后 App 默认的「GitHub 公开发布」通道即可被所有用户使用。

VER="${1:?usage: release.sh <version> [--publish]}"
PUBLISH="${2:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONF="$ROOT/src-tauri/tauri.conf.json"
CARGO="$ROOT/src-tauri/Cargo.toml"
UIPKG="$ROOT/ui/package.json"
KEY="$HOME/.tauri/magic-agent.key"

# GitHub 仓库（源码与 Release 同仓库，避免两套东西不同步）
GH_REPO="Zunzhe966/magic-agent"
GH_TAG="v$VER"

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

TARGZ_BASE="$(basename "$TARGZ")"
TARGZ_BASE="${TARGZ_BASE%.gz}.gz"   # 归一化（避免重复后缀）
SIG_CONTENT="$(cat "$SIG")"
PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SHA256="$(shasum -a 256 "$TARGZ" | awk '{print $1}')"

# ── 发布资产名必须是 ASCII ────────────────────────────────
# 踩过的坑：GitHub Release 上传中文文件名时会被静默改写（如「尊者魔法代理.app.tar.gz」
# 变成「app.tar.gz」），导致 latest.json 里写的下载地址 404。
# 对外发布的资产统一用 ASCII 名：magic-agent_<ver>_<arch>.app.tar.gz
# 注意 macOS 下 uname -m 是 arm64，而 Tauri 平台标识用 aarch64，这里统一成 aarch64
case "$(uname -m)" in
  arm64|aarch64) ARCH="aarch64" ;;
  x86_64|amd64)  ARCH="x86_64"  ;;
  *)             ARCH="$(uname -m)" ;;
esac
ASSET="magic-agent_${VER}_${ARCH}.app.tar.gz"

# 两个 feed 目录：同一份产物，只是 latest.json 里的 URL 不同
FEED_LOCAL="$ROOT/scripts/updater-feed"
FEED_GH="$ROOT/scripts/updater-feed-publish"

# 本地 feed 保留原始文件名即可（本机 http.server 不会改写文件名）
for d in "$FEED_LOCAL" "$FEED_GH"; do
  mkdir -p "$d"
  cp "$TARGZ" "$d/"
  cp "$SIG" "$d/"
done
# 发布 feed 额外放一份 ASCII 命名的副本，供 GitHub Release 使用
cp "$TARGZ" "$FEED_GH/$ASSET"
cp "$SIG"   "$FEED_GH/$ASSET.sig"

URL_LOCAL="http://127.0.0.1:7878/$TARGZ_BASE"
URL_GH="https://github.com/$GH_REPO/releases/download/$GH_TAG/$ASSET"

write_feed() {
  local path="$1"; local url="$2"
  cat > "$path" <<JSON
{
  "version": "$VER",
  "notes": "release $VER",
  "pub_date": "$PUB_DATE",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$SIG_CONTENT",
      "url": "$url"
    }
  }
}
JSON
}

write_feed "$FEED_LOCAL/latest.json" "$URL_LOCAL"
write_feed "$FEED_GH/latest.json"    "$URL_GH"

echo ""
echo "==> 双通道 feed 已生成（同一份包，同一份签名）"
echo "    包: $TARGZ_BASE"
echo "    SHA256: $SHA256"
echo ""
echo "【本地测试通道】$FEED_LOCAL"
echo "  latest.json → 版本 $VER"
echo "  包 URL: $URL_LOCAL"
echo "  启动本地服务："
echo "    cd \"$FEED_LOCAL\" && python3 -m http.server 7878 --bind 127.0.0.1"
echo ""
echo "【GitHub 发布通道】$FEED_GH"
echo "  latest.json → 版本 $VER"
echo "  包 URL: $URL_GH"

if [ "$PUBLISH" = "--publish" ]; then
  echo ""
  echo "==> 发布到 GitHub Releases（$GH_REPO @ $GH_TAG）..."
  if ! gh release view "$GH_TAG" --repo "$GH_REPO" >/dev/null 2>&1; then
    gh release create "$GH_TAG" --repo "$GH_REPO" \
      --title "尊者魔法代理 $VER" \
      --notes "release $VER"
  fi
  gh release upload "$GH_TAG" \
    "$FEED_GH/$ASSET" \
    "$FEED_GH/$ASSET.sig" \
    "$FEED_GH/latest.json" \
    --repo "$GH_REPO" --clobber
  echo "✅ 已发布：https://github.com/$GH_REPO/releases/tag/$GH_TAG"
else
  echo ""
  echo "（未加 --publish，跳过上传。要发布请执行：）"
  echo "  ./scripts/release.sh $VER --publish"
fi
