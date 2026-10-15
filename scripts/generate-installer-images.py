"""Generate the NSIS installer bitmaps from the app icon and the light wallpaper.

Usage (needs Pillow: pip install pillow):
    python scripts/generate-installer-images.py

Outputs src-tauri/installer/sidebar.bmp and src-tauri/installer/header.bmp.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ICON = ROOT / "src-tauri/icons/icon.png"
WALLPAPER = ROOT / "src/assets/wallpapers/light.png"
OUT_DIR = ROOT / "src-tauri/installer"

# NSIS 按系统缩放比例拉伸位图，而且是近邻采样，放大后发糊发锯齿。
# 推荐尺寸是 164x314 / 150x57（100% 缩放），按 1.5 倍出图，让最常见的 150% 缩放接近原尺寸显示。
SCALE = 1.5
SIDEBAR_SIZE = (round(164 * SCALE), round(314 * SCALE))
HEADER_SIZE = (round(150 * SCALE), round(57 * SCALE))

# 和应用浅色模式的 --xin-ink-rgb 保持一致
INK = (28, 28, 32)
# 系统没有 Inter，安装器本身的界面字体就是 Segoe UI，品牌字样跟它走更协调
FONT = "C:/Windows/Fonts/seguisb.ttf"


def load_icon(size):
    return Image.open(ICON).convert("RGBA").resize((size, size), Image.LANCZOS)


def paste_with_shadow(canvas, icon, x, y):
    # 很淡的落影只为让图标从壁纸上浮起来，不做发光
    alpha = icon.getchannel("A").point(lambda a: a * 0.16)
    shadow = Image.new("RGBA", icon.size, (40, 40, 60, 0))
    shadow.putalpha(alpha)
    blur = round(icon.width * 0.06)
    padded = Image.new("RGBA", (icon.width + blur * 4, icon.height + blur * 4), (0, 0, 0, 0))
    padded.paste(shadow, (blur * 2, blur * 2))
    padded = padded.filter(ImageFilter.GaussianBlur(blur))
    canvas.alpha_composite(padded, (x - blur * 2, y - blur * 2 + round(blur * 0.8)))
    canvas.alpha_composite(icon, (x, y))


def draw_centered_text(draw, text, font, center_x, top):
    left, _, right, _ = draw.textbbox((0, 0), text, font=font)
    draw.text((center_x - (right - left) / 2 - left, top), text, font=font, fill=INK)


def remove_bubble(wallpaper):
    # 壁纸左下那颗玻璃气泡缩到侧栏里正好夹在 Xin 和 SSH 之间，像多了个符号。
    # 周围是柔和渐变，从圆的边缘往里反复模糊扩散，就能自然地补上
    center_x, center_y, radius = 211, 584, 28
    keep = Image.new("L", wallpaper.size, 255)
    ImageDraw.Draw(keep).ellipse(
        (center_x - radius, center_y - radius, center_x + radius, center_y + radius), fill=0
    )
    filled = wallpaper
    for blur in (16, 8, 4, 2):
        for _ in range(6):
            filled = Image.composite(wallpaper, filled.filter(ImageFilter.GaussianBlur(blur)), keep)
    return filled


def build_sidebar():
    width, height = SIDEBAR_SIZE
    wallpaper = remove_bubble(Image.open(WALLPAPER).convert("RGB"))
    # 取壁纸左侧一条竖幅，正好带上 >_ 符号和左下的玻璃波浪
    crop_width = round(wallpaper.height * width / height)
    canvas = wallpaper.crop((0, 0, crop_width, wallpaper.height)).resize(SIDEBAR_SIZE, Image.LANCZOS)
    canvas = canvas.convert("RGBA")

    icon_size = round(88 * SCALE)
    icon = load_icon(icon_size)
    icon_top = round(height * 0.27)
    paste_with_shadow(canvas, icon, (width - icon_size) // 2, icon_top)

    draw = ImageDraw.Draw(canvas)
    font = ImageFont.truetype(FONT, round(18 * SCALE))
    draw_centered_text(draw, "Xin SSH", font, width / 2, icon_top + icon_size + round(10 * SCALE))
    return canvas.convert("RGB")


def build_header():
    width, height = HEADER_SIZE
    # MUI 顶部横幅的底色就是窗口白，图片背景必须同色才不会露出一个色块
    canvas = Image.new("RGBA", HEADER_SIZE, (255, 255, 255, 255))
    icon_size = round(40 * SCALE)
    left = round(10 * SCALE)
    canvas.alpha_composite(load_icon(icon_size), (left, (height - icon_size) // 2))

    draw = ImageDraw.Draw(canvas)
    font = ImageFont.truetype(FONT, round(15 * SCALE))
    text_left = left + icon_size + round(8 * SCALE)
    _, top, _, bottom = draw.textbbox((0, 0), "Xin SSH", font=font)
    draw.text((text_left, (height - (bottom - top)) / 2 - top), "Xin SSH", font=font, fill=INK)
    return canvas.convert("RGB")


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    # NSIS 只认未压缩的 24 位 BMP，RGB 模式存出来正好是这个格式
    build_sidebar().save(OUT_DIR / "sidebar.bmp")
    build_header().save(OUT_DIR / "header.bmp")
    print(f"sidebar {SIDEBAR_SIZE}, header {HEADER_SIZE} -> {OUT_DIR}")


if __name__ == "__main__":
    main()
