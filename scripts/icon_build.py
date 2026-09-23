#!/usr/bin/env python3
"""平台规范化图标后处理。

输入：src-tauri/icons/source-icon.png（正方形，>=1024）
前置：scripts/make-icons.sh 已先跑过 `cargo tauri icon`，
      生成了 Windows/Linux 用的方形 PNG/ICO（这些平台就是要方形，保持不动）。

本脚本只修 `cargo tauri icon` 修不了的平台规范：

* macOS icon.icns —— 整张照片按 Apple 图标网格嵌进松鼠形圆角矩形
  （1024 画布、内缩 82.4pt、圆角 185.4pt、4x 超采样抗锯齿）。
* iOS —— 去掉 alpha 通道（App Store 强制要求），补齐 1024 营销图。
* Android —— 自适应图标前景把画面收进 72/108 安全区（任何遮罩都不切主体），
  背景色取母版边缘主色（深夜空蓝），legacy/round 图标同风格合成。
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageStat

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "src-tauri" / "icons"
SOURCE = ICONS / "source-icon.png"

# Apple macOS Big Sur+ 图标网格（基于 1024pt 画布）
MAC_CANVAS = 1024
MAC_INSET = 82.4          # 圆角矩形外框距画布边缘
MAC_RADIUS = 185.4        # 连续圆角近似半径
SUPERSAMPLE = 4           # 超采样倍数，缩小后边缘顺滑

# Android adaptive icon：前景 108dp，安全区 72dp
ADAPTIVE_SAFE = 72 / 108
FOREGROUND_SIZES = {
    "mipmap-mdpi": 108,
    "mipmap-hdpi": 162,
    "mipmap-xhdpi": 216,
    "mipmap-xxhdpi": 324,
    "mipmap-xxxhdpi": 432,
}
LEGACY_SIZES = {
    "mipmap-mdpi": 48,
    "mipmap-hdpi": 72,
    "mipmap-xhdpi": 96,
    "mipmap-xxhdpi": 144,
    "mipmap-xxxhdpi": 192,
}

# iOS 项目需要的全部尺寸（文件名 => 边长像素）
IOS_SIZES = {
    "AppIcon-20x20@1x.png": 20,
    "AppIcon-20x20@2x.png": 40,
    "AppIcon-20x20@2x-1.png": 40,
    "AppIcon-20x20@3x.png": 60,
    "AppIcon-29x29@1x.png": 29,
    "AppIcon-29x29@2x.png": 58,
    "AppIcon-29x29@2x-1.png": 58,
    "AppIcon-29x29@3x.png": 87,
    "AppIcon-40x40@1x.png": 40,
    "AppIcon-40x40@2x.png": 80,
    "AppIcon-40x40@2x-1.png": 80,
    "AppIcon-40x40@3x.png": 120,
    "AppIcon-60x60@2x.png": 120,
    "AppIcon-60x60@3x.png": 180,
    "AppIcon-76x76@1x.png": 76,
    "AppIcon-76x76@2x.png": 152,
    "AppIcon-83.5x83.5@2x.png": 167,
    "AppIcon-512@2x.png": 1024,
    "AppIcon-1024.png": 1024,
}


def edge_background_color(img: Image.Image) -> tuple[int, int, int]:
    """取母版最外圈环带颜色的中位数作为兜底底色。

    用中位数而非平均数：画面底部有大片高亮云雾，平均数会被拉成发灰的中间调；
    中位数代表占环带多数的深夜空色，云雾属于少数极端值不参与结果。
    """
    w = img.width
    band = max(2, int(w * 0.02))
    ring = Image.new("RGB", (w, w))
    mask = Image.new("L", (w, w), 0)
    d = ImageDraw.Draw(mask)
    d.rectangle([0, 0, w - 1, band - 1], fill=255)
    d.rectangle([0, w - band, w - 1, w - 1], fill=255)
    d.rectangle([0, 0, band - 1, w - 1], fill=255)
    d.rectangle([w - band, 0, w - 1, w - 1], fill=255)
    ring.paste(img.convert("RGB"), (0, 0), mask)
    med = ImageStat.Stat(ring, mask).median
    return tuple(int(max(0, min(255, c))) for c in med)


def blurred_background(rgba: Image.Image, size: int, bg: tuple[int, int, int]) -> Image.Image:
    """自适应图标背景层：母版铺满 + 强高斯模糊 + 压暗。

    前景安全区外露出的边距看起来就是画面夜空/云雾的自然延伸，
    而不是一块突兀的纯色补丁。底色 bg 作为模糊层的兜底色先填满。
    """
    layer = Image.new("RGB", (size, size), bg)
    art = rgba.resize((size, size), Image.LANCZOS)
    layer.paste(art, (0, 0), art)
    layer = layer.filter(ImageFilter.GaussianBlur(radius=max(4, size * 0.06)))
    # 轻微压暗，让中间清晰的前景主体更突出
    dark = Image.new("RGB", (size, size), (0, 0, 0))
    layer = Image.blend(layer, dark, 0.08)
    return layer



def build_macos_icns(src: Image.Image) -> None:
    """按 Apple 图标网格生成 1024 母版，再用 iconutil 打包多分辨率 icns。"""
    s = SUPERSAMPLE
    canvas_big = MAC_CANVAS * s
    inset_big = MAC_INSET * s
    radius_big = MAC_RADIUS * s
    art_size = int(round(canvas_big - inset_big * 2))

    art = src.convert("RGBA").resize((art_size, art_size), Image.LANCZOS)
    layer = Image.new("RGBA", (canvas_big, canvas_big), (0, 0, 0, 0))
    layer.paste(art, (int(round(inset_big)), int(round(inset_big))))
    mask = Image.new("L", (canvas_big, canvas_big), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [inset_big, inset_big, canvas_big - inset_big, canvas_big - inset_big],
        radius=radius_big,
        fill=255,
    )
    layer.putalpha(mask)
    master = layer.resize((MAC_CANVAS, MAC_CANVAS), Image.LANCZOS)

    iconset = ICONS / "icon.iconset"
    if iconset.exists():
        shutil.rmtree(iconset)
    iconset.mkdir()
    slots = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ]
    for name, px in slots:
        master.resize((px, px), Image.LANCZOS).save(iconset / name)

    icns = ICONS / "icon.icns"
    subprocess.run(
        ["iconutil", "-c", "icns", str(iconset), "-o", str(icns)],
        check=True,
        capture_output=True,
    )
    shutil.rmtree(iconset)
    master.save(ICONS / "icon-macos-preview.png")
    print(f"  macOS: {icns.name}（10 个分辨率层，松鼠形圆角）")


def build_ios(src: Image.Image) -> None:
    """iOS 图标必须是无 alpha 的不透明方形，由系统自己遮罩圆角。"""
    ios_dir = ICONS / "ios"
    ios_dir.mkdir(exist_ok=True)
    rgba = src.convert("RGBA")
    for name, px in IOS_SIZES.items():
        # 显式压到不透明底上，保证 PNG 里不带 alpha chunk
        art = rgba.resize((px, px), Image.LANCZOS)
        flat = Image.new("RGB", (px, px), (0, 0, 0))
        flat.paste(art, (0, 0), art)
        flat.save(ios_dir / name)
    print(f"  iOS: {len(IOS_SIZES)} 个文件（全部无 alpha，含 1024 营销图）")


def build_android(src: Image.Image) -> None:
    """自适应图标：前景收进安全区，背景为母版模糊延伸；legacy/round 同风格。"""
    bg = edge_background_color(src)

    # 底色资源保留作兜底；自适应图标实际引用 @mipmap 模糊背景图
    values = ICONS / "android" / "values"
    values.mkdir(parents=True, exist_ok=True)
    (values / "ic_launcher_background.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n'
        "<resources>\n"
        f'  <color name="ic_launcher_background">#{bg[0]:02x}{bg[1]:02x}{bg[2]:02x}</color>\n'
        "</resources>\n",
        encoding="utf-8",
    )

    anydpi = ICONS / "android" / "mipmap-anydpi-v26"
    anydpi.mkdir(parents=True, exist_ok=True)
    (anydpi / "ic_launcher.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
        '  <background android:drawable="@mipmap/ic_launcher_background"/>\n'
        '  <foreground android:drawable="@mipmap/ic_launcher_foreground"/>\n'
        "</adaptive-icon>\n",
        encoding="utf-8",
    )

    rgba = src.convert("RGBA")
    for density, fg_canvas in FOREGROUND_SIZES.items():
        ddir = ICONS / "android" / density
        ddir.mkdir(parents=True, exist_ok=True)

        # 背景：母版铺满 + 强模糊（与前景边缘的夜空云雾自然衔接）
        blurred_background(rgba, fg_canvas, bg).save(
            ddir / "ic_launcher_background.png"
        )

        # 前景：透明底 + 画面缩到 72/108 安全区居中
        inner = round(fg_canvas * ADAPTIVE_SAFE)
        fg = Image.new("RGBA", (fg_canvas, fg_canvas), (0, 0, 0, 0))
        art = rgba.resize((inner, inner), Image.LANCZOS)
        off = (fg_canvas - inner) // 2
        fg.paste(art, (off, off), art)
        fg.save(ddir / "ic_launcher_foreground.png")

        # legacy：模糊背景 + 居中清晰主体（与自适应版本观感一致）
        legacy_px = LEGACY_SIZES[density]
        base = blurred_background(rgba, legacy_px, bg)
        linner = round(legacy_px * ADAPTIVE_SAFE)
        lart = rgba.resize((linner, linner), Image.LANCZOS)
        loff = (legacy_px - linner) // 2
        base.paste(lart, (loff, loff), lart)
        base.save(ddir / "ic_launcher.png")

        # round：同一构图裁成正圆，圆外用底色兜底（不透明，避免某些启动器渲染异常）
        mask = Image.new("L", (legacy_px, legacy_px), 0)
        ImageDraw.Draw(mask).ellipse([0, 0, legacy_px - 1, legacy_px - 1], fill=255)
        round_img = Image.new("RGB", (legacy_px, legacy_px), bg)
        round_img.paste(base, (0, 0), mask)
        round_img.save(ddir / "ic_launcher_round.png")

    print(
        f"  Android: 5 档密度（前景安全区 72/108，模糊背景，兜底色 #{bg[0]:02x}{bg[1]:02x}{bg[2]:02x}）"
    )


def main() -> int:
    if not SOURCE.is_file():
        print(f"找不到图标母版: {SOURCE}", file=sys.stderr)
        return 1
    with Image.open(SOURCE) as im:
        im.load()
        if im.width != im.height or im.width < 1024:
            print(f"母版必须是 >=1024 的正方形，实际 {im.width}x{im.height}", file=sys.stderr)
            return 1
        src = im.copy()

    print(f"==> 平台规范化后处理（母版 {src.width}x{src.height}）")
    build_macos_icns(src)
    build_ios(src)
    build_android(src)
    print("==> 完成")
    return 0


if __name__ == "__main__":
    sys.exit(main())
