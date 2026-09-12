#!/usr/bin/env python3
"""Dev-time icon generator for Caffeine Desktop.

Draws the app mark (espresso rounded square + caramel monitor + coffee cup
with steam) with Pillow and exports Tauri bundle assets:
  src-tauri/icons/icon.png (512), 32x32.png, 128x128.png,
  128x128@2x.png (256), icon.ico (multi-size), tray-icon.png (32)

This script runs on the *development* machine only. The shipped app is a
native single .exe and needs no Python at runtime.
"""
from PIL import Image, ImageDraw
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ICON_DIR = os.path.join(HERE, "..", "src-tauri", "icons")

BG = (26, 16, 9, 255)        # deep espresso
RING = (232, 163, 61, 255)   # caramel
CREAM = (245, 235, 221, 255)
COFFEE = (107, 62, 30, 255)
LATTE = (201, 111, 43, 255)


def rounded_bg(size: int, radius_ratio: float = 0.225) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle([0, 0, size - 1, size - 1],
                        radius=int(size * radius_ratio), fill=BG)
    return img


def draw_mark(d: ImageDraw.ImageDraw, s: int) -> None:
    u = s / 256.0  # unit: design on a 256 grid
    # caramel ring
    d.rounded_rectangle([8 * u, 8 * u, 248 * u, 248 * u],
                        radius=int(44 * u), outline=RING, width=max(2, int(7 * u)))
    # monitor
    d.rounded_rectangle([44 * u, 56 * u, 212 * u, 168 * u],
                        radius=int(12 * u), outline=CREAM, width=max(2, int(9 * u)))
    # screen
    d.rounded_rectangle([58 * u, 70 * u, 198 * u, 154 * u],
                        radius=int(6 * u), fill=(42, 29, 18, 255))
    # stand
    d.rectangle([118 * u, 168 * u, 138 * u, 184 * u], fill=CREAM)
    d.rounded_rectangle([96 * u, 184 * u, 160 * u, 194 * u],
                        radius=int(5 * u), fill=CREAM)
    # cup body
    d.rounded_rectangle([96 * u, 102 * u, 146 * u, 142 * u],
                        radius=int(6 * u), fill=CREAM)
    # cup handle
    d.arc([140 * u, 106 * u, 164 * u, 130 * u], start=270, end=90,
          fill=CREAM, width=max(2, int(7 * u)))
    # coffee surface
    d.ellipse([100 * u, 98 * u, 142 * u, 110 * u], fill=LATTE)
    d.ellipse([106 * u, 100 * u, 136 * u, 108 * u], fill=COFFEE)
    # steam strokes
    for x in (110, 121, 132):
        d.line([(x * u, 92 * u), ((x - 4) * u, 84 * u),
                ((x + 2) * u, 76 * u), ((x - 2) * u, 68 * u)],
               fill=RING, width=max(2, int(5 * u)), joint="curve")


def draw_tray(d: ImageDraw.ImageDraw, s: int) -> None:
    """Simplified high-contrast glyph for the Windows tray (dark/light)."""
    u = s / 32.0
    # monitor outline
    d.rounded_rectangle([2 * u, 5 * u, 30 * u, 22 * u],
                        radius=int(2 * u), outline=CREAM, width=max(1, int(2 * u)))
    d.rectangle([13 * u, 22 * u, 19 * u, 26 * u], fill=CREAM)
    d.rounded_rectangle([9 * u, 26 * u, 23 * u, 28 * u],
                        radius=int(1 * u), fill=CREAM)
    # cup
    d.rectangle([12 * u, 11 * u, 20 * u, 18 * u], fill=RING)
    d.rectangle([20 * u, 12 * u, 22 * u, 16 * u], outline=RING,
                width=max(1, int(1 * u)))
    # steam
    for x in (13.5, 16.5):
        d.line([(x * u, 9 * u), ((x - 1) * u, 7 * u), (x * u, 5 * u)],
               fill=RING, width=max(1, int(1 * u)))


def main() -> None:
    os.makedirs(ICON_DIR, exist_ok=True)

    master = rounded_bg(512)
    draw_mark(ImageDraw.Draw(master), 512)
    master.save(os.path.join(ICON_DIR, "icon.png"))

    for name, size in (("32x32.png", 32), ("128x128.png", 128),
                       ("128x128@2x.png", 256)):
        img = rounded_bg(size)
        draw_mark(ImageDraw.Draw(img), size)
        img.save(os.path.join(ICON_DIR, name))

    master.save(os.path.join(ICON_DIR, "icon.ico"),
                sizes=[(16, 16), (24, 24), (32, 32), (48, 48),
                       (64, 64), (128, 128), (256, 256)])

    tray = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw_tray(ImageDraw.Draw(tray), 32)
    tray.save(os.path.join(ICON_DIR, "tray-icon.png"))

    print("icons written to", ICON_DIR)
    for f in sorted(os.listdir(ICON_DIR)):
        print(" -", f)


if __name__ == "__main__":
    main()
