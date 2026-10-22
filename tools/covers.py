"""Covers of the modules (placeholder until the real artwork is drawn)."""
from PIL import Image


def draw(module, path):
    Image.new("RGB", (640, 360), (11, 13, 14)).save(path)
