#!/usr/bin/env python3
"""Render logo.svg -> logo.png (1024x1024) and tray.svg -> tray.png (44x44).

Needs: pip install resvg-py pillow
"""
import io
import pathlib

import resvg_py
from PIL import Image

HERE = pathlib.Path(__file__).resolve().parent


def render(name, size):
    data = resvg_py.svg_to_bytes(svg_path=str(HERE / f"{name}.svg"), width=size, height=size)
    img = Image.open(io.BytesIO(bytes(data))).convert("RGBA")
    assert img.size == (size, size), img.size
    return img


logo = render("logo", 1024)
logo.save(HERE / "logo.png")

# Template image: keep only the alpha channel, force every pixel to pure black.
tray = render("tray", 44)
black = Image.new("RGBA", tray.size, (0, 0, 0, 0))
black.putalpha(tray.getchannel("A"))
black.save(HERE / "tray.png")

print("wrote logo.png", logo.size, "and tray.png", tray.size)
