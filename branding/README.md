# hams branding

Concept: a lowercase "h" drawn from two rounded waveform bars joined by an arch, followed by a shorter bar and a dot — a voice dying away into a whisper (همس).

Colours:
- Tile gradient (top to bottom): `#5E5CE0` -> `#3F3BB0`
- Mark: `#F6F4FF` (trailing bar at 72% opacity, dot at 45%)
- Tray glyph: `#000` on transparent (macOS template image; only alpha matters)

Regenerate the PNGs (`logo.png` 1024x1024, `tray.png` 44x44) from the SVGs:

```sh
python3 -m venv /tmp/hams-venv && /tmp/hams-venv/bin/pip install resvg-py pillow
/tmp/hams-venv/bin/python render.py
```

`social-preview.png` (1280x640) is the image GitHub shows when the repo link is shared. Upload it under
the repo's Settings → General → Social preview.
