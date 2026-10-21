"""Generates the AirLisp app icon (PNG sizes + .ico) with Pillow. Run: python assets/icon/make_icon.py

Design: a JetBrains-style black tile with the white underline bar, and a luminous gradient lambda
(the Lisp symbol) whose strokes fade into "air" streaks on the left - ArtCraft-like colour, JetBrains-like structure.
"""
import math
import os

from PIL import Image, ImageChops, ImageDraw, ImageFilter

S = 2048  # supersampled canvas
OUT = os.path.dirname(os.path.abspath(__file__))


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def gradient(size, stops, angle_deg=0.0):
    """Linear gradient image; stops = [(pos, (r,g,b)), ...]."""
    w, h = size
    img = Image.new("RGB", size)
    px = img.load()
    ca, sa = math.cos(math.radians(angle_deg)), math.sin(math.radians(angle_deg))
    half = (abs(w * ca) + abs(h * sa)) / 2
    for y in range(h):
        for x in range(w):
            t = (((x - w / 2) * ca + (y - h / 2) * sa) / half + 1) / 2
            t = min(1.0, max(0.0, t))
            for i in range(len(stops) - 1):
                p0, c0 = stops[i]
                p1, c1 = stops[i + 1]
                if t <= p1 or i == len(stops) - 2:
                    u = 0 if p1 == p0 else min(1.0, max(0.0, (t - p0) / (p1 - p0)))
                    px[x, y] = lerp(c0, c1, u)
                    break
    return img


def rounded_mask(size, radius):
    m = Image.new("L", size, 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, size[0] - 1, size[1] - 1], radius=radius, fill=255)
    return m


def stroke_mask(points_list, width):
    m = Image.new("L", (S, S), 0)
    d = ImageDraw.Draw(m)
    for (x0, y0), (x1, y1) in points_list:
        d.line([(x0, y0), (x1, y1)], fill=255, width=width)
        r = width // 2
        for (x, y) in ((x0, y0), (x1, y1)):
            d.ellipse([x - r, y - r, x + r, y + r], fill=255)
    return m


def build():
    # ---- tile: near-black with a faint cool vertical gradient
    tile_bg = gradient((S, S), [(0.0, (22, 24, 33)), (1.0, (6, 7, 10))], angle_deg=90)
    tile = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    tile.paste(tile_bg, (0, 0), rounded_mask((S, S), int(S * 0.225)))

    # ---- lambda geometry (in 2048 space)
    w = int(S * 0.125)
    long_stroke = ((int(S * 0.355), int(S * 0.215)), (int(S * 0.700), int(S * 0.775)))
    short_stroke = ((int(S * 0.512), int(S * 0.470)), (int(S * 0.330), int(S * 0.775)))
    lam = stroke_mask([long_stroke, short_stroke], w)

    # gradient fill across the lambda: cyan -> blue -> magenta -> orange
    fill = gradient((S, S), [(0.0, (60, 220, 255)), (0.38, (80, 110, 255)), (0.70, (226, 70, 255)), (1.0, (255, 150, 60))], angle_deg=55)

    # soft glow behind the glyph
    glow = lam.filter(ImageFilter.GaussianBlur(S * 0.035))
    glow_rgba = Image.merge("RGBA", (*fill.split(), glow.point(lambda v: int(v * 0.55))))
    tile = Image.alpha_composite(tile, glow_rgba)

    # airy streaks: thin horizontal wisps fading to the left, as if the lambda moves through air
    streaks = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    sd = ImageDraw.Draw(streaks)
    wisps = [(0.15, 0.340, 0.50, 0.016, 235), (0.09, 0.430, 0.44, 0.012, 190), (0.19, 0.520, 0.47, 0.016, 220), (0.13, 0.610, 0.40, 0.011, 160)]
    for x0, y, x1, th, alpha in wisps:
        h = int(S * th)
        sd.rounded_rectangle([int(S * x0), int(S * y) - h // 2, int(S * x1), int(S * y) + h // 2], radius=h // 2, fill=(150, 225, 255, alpha))
    fade = gradient((S, S), [(0.0, (0, 0, 0)), (0.55, (255, 255, 255)), (1.0, (255, 255, 255))], angle_deg=0).convert("L")
    sa = streaks.split()[3]
    streaks.putalpha(ImageChops.multiply(sa, fade))
    tile = Image.alpha_composite(tile, streaks)

    # the glyph itself
    glyph = Image.merge("RGBA", (*fill.split(), lam))
    tile = Image.alpha_composite(tile, glyph)

    # JetBrains signature: the white bar at the bottom left
    bar = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    bd = ImageDraw.Draw(bar)
    bd.rounded_rectangle([int(S * 0.165), int(S * 0.835), int(S * 0.385), int(S * 0.870)], radius=int(S * 0.0175), fill=(255, 255, 255, 255))
    tile = Image.alpha_composite(tile, bar)

    # subtle top highlight inside the tile
    hi = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    hd = ImageDraw.Draw(hi)
    hd.rounded_rectangle([int(S * 0.02), int(S * 0.02), int(S * 0.98), int(S * 0.98)], radius=int(S * 0.21), outline=(255, 255, 255, 38), width=int(S * 0.006))
    tile = Image.alpha_composite(tile, hi)
    return tile


def main():
    big = build()
    sizes = [16, 24, 32, 48, 64, 96, 128, 256, 512]
    images = {}
    for sz in sizes:
        img = big.resize((sz, sz), Image.LANCZOS)
        img.save(os.path.join(OUT, f"airlisp-{sz}.png"))
        images[sz] = img
    big.resize((1024, 1024), Image.LANCZOS).save(os.path.join(OUT, "airlisp-1024.png"))
    images[256].save(os.path.join(OUT, "airlisp.ico"), format="ICO", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    print("icons written to", OUT)


if __name__ == "__main__":
    main()
