#!/usr/bin/env python3
"""Package the approved app and coffee tray PNGs for Flutter and Windows.

Transparent-canvas fitting and format/size conversion; artwork comes from imagegen.
Run: python tools/make_icons.py [path/to/square-transparent-source.png]
     python tools/make_icons.py --only tray [--tray-source path/to/source.png]
Requires Pillow on the development machine, never at app runtime.
"""
from pathlib import Path
import argparse
import shutil
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "assets" / "icons"
SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)


def load_source(path: Path) -> Image.Image:
    with Image.open(path) as source:
        if source.width != source.height:
            raise ValueError(f"The approved source must be square: {path}")
        rgba = source.convert("RGBA")
    if rgba.getchannel("A").getextrema() != (0, 255):
        raise ValueError(f"The source must contain real transparency and opaque artwork: {path}")
    return rgba


def fit_tray_canvas(rgba: Image.Image) -> Image.Image:
    # Ignore near-invisible alpha speckles when measuring the artwork. Keep
    # its shape/aspect ratio and a small safety margin for antialiased edges.
    bounds = rgba.getchannel("A").point(lambda alpha: 255 if alpha >= 16 else 0).getbbox()
    if bounds is None:
        raise ValueError("The tray source must contain visible artwork")
    left, top, right, bottom = bounds
    crop = rgba.crop((max(0, left - 2), max(0, top - 2),
                      min(rgba.width, right + 2), min(rgba.height, bottom + 2)))
    side = max(crop.size)
    padding = max(1, round(side * 0.02))
    canvas = Image.new("RGBA", (side + 2 * padding, side + 2 * padding))
    canvas.paste(crop, ((canvas.width - crop.width) // 2,
                        (canvas.height - crop.height) // 2))
    return canvas


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", type=Path,
                        default=ICON_DIR / "icon-source.png")
    parser.add_argument("--tray-source", type=Path,
                        default=ICON_DIR / "tray-icon-source.png")
    parser.add_argument("--only", choices=("all", "app", "tray"), default="all")
    args = parser.parse_args()
    app = load_source(args.source) if args.only in ("all", "app") else None
    tray = load_source(args.tray_source) if args.only in ("all", "tray") else None
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    if app is not None:
        master = app.resize((512, 512), Image.Resampling.LANCZOS)
        master.save(ICON_DIR / "icon.png", optimize=True)
        for name, size in (("32x32.png", 32), ("128x128.png", 128),
                           ("128x128@2x.png", 256)):
            app.resize((size, size), Image.Resampling.LANCZOS).save(ICON_DIR / name)
        master.save(ICON_DIR / "icon.ico", sizes=[(s, s) for s in SIZES])
        runner = ROOT / "windows" / "runner" / "resources" / "app_icon.ico"
        shutil.copyfile(ICON_DIR / "icon.ico", runner)
        print(f"Packaged app icons in {ICON_DIR} and {runner}")
    if tray is not None:
        tray = fit_tray_canvas(tray)
        tray.resize((32, 32), Image.Resampling.LANCZOS).save(ICON_DIR / "tray-icon.png")
        tray.save(ICON_DIR / "tray-icon.ico", sizes=[(s, s) for s in SIZES[:7]])
        print(f"Packaged coffee tray icons (16-64px) in {ICON_DIR}")


if __name__ == "__main__":
    main()
