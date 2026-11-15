#!/usr/bin/env python3
"""Package the approved gen-bot-face PNG for Flutter, tray and Windows.

Format/size conversion only; artwork comes from imagegen.
Run: python tools/make_icons.py [path/to/square-transparent-source.png]
Requires Pillow on the development machine, never at app runtime.
"""
from pathlib import Path
import argparse
import shutil
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
ICON_DIR = ROOT / "assets" / "icons"
SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", nargs="?", type=Path,
                        default=ICON_DIR / "icon-source.png")
    args = parser.parse_args()
    with Image.open(args.source) as source:
        if source.width != source.height:
            raise ValueError("The approved source must be square")
        rgba = source.convert("RGBA")
    if rgba.getchannel("A").getextrema() != (0, 255):
        raise ValueError("The source must contain real transparency and opaque artwork")
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    master = rgba.resize((512, 512), Image.Resampling.LANCZOS)
    master.save(ICON_DIR / "icon.png", optimize=True)
    for name, size in (("32x32.png", 32), ("128x128.png", 128),
                       ("128x128@2x.png", 256), ("tray-icon.png", 32)):
        rgba.resize((size, size), Image.Resampling.LANCZOS).save(ICON_DIR / name)
    master.save(ICON_DIR / "icon.ico", sizes=[(s, s) for s in SIZES])
    master.save(ICON_DIR / "tray-icon.ico", sizes=[(s, s) for s in SIZES[:6]])
    runner = ROOT / "windows" / "runner" / "resources" / "app_icon.ico"
    shutil.copyfile(ICON_DIR / "icon.ico", runner)
    print(f"Packaged transparent icons in {ICON_DIR} and {runner}")


if __name__ == "__main__":
    main()
