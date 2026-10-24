"""Generates the Air app icon (PNG sizes + .ico). Run: python assets/icon/make_icon.py

The mark: a white A on an opaque, pure-black background. Only the lower ends
carry a soft green-to-blue tint; the rest of the letter stays white.
"""
import math
import os
import pathlib
import sys

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent.parent / "tools"))
import glass as g  # noqa: E402

S = 1024
SS = 2  # supersampling of the shapes
OUT = os.path.dirname(os.path.abspath(__file__))


def letter_mask(w, h):
    """The A as a mask: two round-capped legs meeting at the apex, and a crossbar that curves like a stream."""
    big = Image.new("L", (w * SS, h * SS), 0)
    d = ImageDraw.Draw(big)
    P = lambda x, y: (x * w * SS, y * h * SS)
    stroke = 0.118 * w * SS
    apex, left, right = P(0.50, 0.215), P(0.255, 0.80), P(0.745, 0.80)

    def thick(a, b):
        d.line([a, b], fill=255, width=int(stroke))
        for c in (a, b):
            r = stroke / 2
            d.ellipse([c[0] - r, c[1] - r, c[0] + r, c[1] + r], fill=255)

    thick(left, apex)
    thick(apex, right)
    # the crossbar: a gentle wave from leg to leg
    pts = []
    for i in range(41):
        t = i / 40
        x = 0.37 + (0.63 - 0.37) * t
        y = 0.625 - 0.06 * math.sin(t * math.pi)
        pts.append(P(x, y))
    bar = stroke * 0.5
    d.line(pts, fill=255, width=int(bar))
    # Overlapping round samples avoid thin seams at the curved crossbar joins.
    for c in pts:
        r = bar / 2
        d.ellipse([c[0] - r, c[1] - r, c[0] + r, c[1] + r], fill=255)
    return np.asarray(big.resize((w, h), Image.LANCZOS), np.float32) / 255.0


def build():
    w = h = S
    canvas = g.new(w, h, color=(0, 0, 0))
    a = letter_mask(w, h)
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    # Fade colour in only below the crossbar, keeping most of the A pure white.
    fade = np.clip((ys / h - 0.64) / 0.22, 0, 1)
    fade = (fade * fade * (3 - 2 * fade) * 0.32)[..., None]
    horizontal = np.clip((xs / w - 0.28) / 0.44, 0, 1)[..., None]
    green = np.array(g.GREEN, np.float32) / 255.0
    blue = np.array(g.BLUE, np.float32) / 255.0
    tint = green * (1 - horizontal) + blue * horizontal
    ink = g.layer(w, h, g.WHITE, a)
    ink[..., :3] = 1 - fade + tint * fade
    return g.to_image(g.over(canvas, ink))


def main():
    big = build()
    sizes = [16, 24, 32, 48, 64, 96, 128, 256, 512]
    images = {}
    for sz in sizes:
        img = big.resize((sz, sz), Image.LANCZOS)
        img.save(os.path.join(OUT, f"air-{sz}.png"))
        images[sz] = img
    big.save(os.path.join(OUT, "air-1024.png"))
    images[256].save(os.path.join(OUT, "air.ico"), format="ICO", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    print("icons written to", OUT)


if __name__ == "__main__":
    main()
