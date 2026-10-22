"""Generates the Air app icon (PNG sizes + .ico). Run: python assets/icon/make_icon.py

The mark: a letter A made of liquid glass - two legs and a crossbar that is a stream of air - on a black tile that
glows green in one corner and blue in the other. The look is drawn with tools/glass.py.
"""
import math
import os
import pathlib
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

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
    d.line(pts, fill=255, width=int(bar), joint="curve")
    for c in (pts[0], pts[-1]):
        r = bar / 2
        d.ellipse([c[0] - r, c[1] - r, c[0] + r, c[1] + r], fill=255)
    return np.asarray(big.resize((w, h), Image.LANCZOS), np.float32) / 255.0


def build():
    w = h = S
    tile = g.rounded_mask(w, h, (S * 0.035, S * 0.035, S * 0.965, S * 0.965), S * 0.215)
    # the ground: black with two orbs of light
    ground = g.vgradient(w, h, (12, 21, 19), (6, 9, 15))
    ground = g.over(ground, g.glow(w, h, S * 0.22, S * 0.16, S * 0.85, g.GREEN, 0.78))
    ground = g.over(ground, g.glow(w, h, S * 0.86, S * 0.90, S * 0.78, g.BLUE, 0.80))
    ground = g.over(ground, g.glow(w, h, S * 0.62, S * 0.40, S * 0.36, g.TEAL, 0.16))
    canvas = ground.copy()

    a = letter_mask(w, h)
    # soft light spilling out of the letter
    spill = g.blur(g.layer(w, h, g.GREEN, a * 0.5), S * 0.045)
    canvas = g.over(canvas, spill)
    # the letter is a pane of frosted glass (the glows behind it shine through) with colour poured into it...
    canvas = g.glass(canvas, a, tint_alpha=0.07, frost=S * 0.014, rim=0.0, highlight=0.0, dark_edge=0.0)
    ink = g.diagonal(w, h, (30, 232, 112), (62, 112, 255), 50)
    ink[..., 3] = a * 0.66
    canvas = g.over(canvas, ink)
    # ...and light on its edge and across its top
    canvas = g.glass(canvas, a, tint_alpha=0.0, frost=0.6, rim=0.60, highlight=0.32, dark_edge=0.25, rim_width=max(2, int(S * 0.004)))
    # a drop of light on the apex
    canvas = g.bubble(canvas, S * 0.50, S * 0.235, S * 0.030, strength=0.9)

    # the tile's own rim
    edge = g.ring_from_mask(tile, max(2, int(S * 0.006)))
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    light = np.clip(1 - (xs / w * 0.8 + ys / h) / 1.1, 0, 1) ** 1.3
    canvas = g.over(canvas, g.layer(w, h, g.WHITE, edge * (0.10 + light * 0.34)))
    out = canvas.copy()
    out[..., 3] = tile
    return g.to_image(out)


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
