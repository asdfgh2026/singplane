#!/usr/bin/env python3
"""Resize assets/icons/app_icon_1024.png into desktop and Android icons."""

from __future__ import annotations

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
MASTER = ROOT / "assets" / "icons" / "app_icon_1024.png"


def main() -> None:
    if not MASTER.exists():
        raise SystemExit(f"Missing master icon: {MASTER}")
    master = Image.open(MASTER).convert("RGBA")
    w, h = master.size
    s = min(w, h)
    master = master.crop(((w - s) // 2, (h - s) // 2, (w + s) // 2, (h + s) // 2))
    master = master.resize((1024, 1024), Image.Resampling.LANCZOS)

    def resize(n: int) -> Image.Image:
        return master.resize((n, n), Image.Resampling.LANCZOS)

    for folder, size in {
        "mipmap-mdpi": 48,
        "mipmap-hdpi": 72,
        "mipmap-xhdpi": 96,
        "mipmap-xxhdpi": 144,
        "mipmap-xxxhdpi": 192,
    }.items():
        d = ROOT / "mobile" / "app" / "src" / "main" / "res" / folder
        d.mkdir(parents=True, exist_ok=True)
        resize(size).save(d / "ic_launcher.png")
        print("android", folder, size)

    desktop = ROOT / "desktop" / "assets"
    desktop.mkdir(parents=True, exist_ok=True)
    resize(1024).save(desktop / "app_icon.png")
    resize(64).save(desktop / "tray.png")
    print("desktop app_icon.png + tray.png")

    ico = desktop / "app_icon.ico"
    ico_sizes = [16, 24, 32, 48, 64, 128, 256]
    ico_images = [resize(n) for n in ico_sizes]
    ico_images[-1].save(
        ico,
        format="ICO",
        append_images=ico_images[:-1],
        sizes=[(n, n) for n in ico_sizes],
    )
    print("desktop", ico, ico.stat().st_size, "sizes", ico_sizes)

    icons = ROOT / "assets" / "icons"
    resize(1024).save(icons / "app_icon.png")
    resize(64).save(icons / "tray.png")
    print("assets app_icon.png + tray.png")


if __name__ == "__main__":
    main()
