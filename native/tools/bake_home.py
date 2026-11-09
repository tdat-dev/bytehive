"""Bake CSS radial gradients GPUI cannot draw into PNGs, at a quarter size.

home_haze.png: inbox.css .home, radial-gradient(900px 420px,
  rgba(255,196,120,.09), transparent 62%).
logo_glow.png: .st-brand .logo-glow::before, a 150px circle of
  rgba(245,165,36,.34), rgba(255,216,92,.1) 45%, transparent 70%, at opacity .19.
"""
import struct
import sys
import zlib
from pathlib import Path


def png(path, w, h, pixel):
    rows = []
    for y in range(h):
        row = bytearray([0])
        for x in range(w):
            row += bytes(pixel(x, y))
        rows.append(bytes(row))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b"")
    Path(path).write_bytes(data)
    print(path, len(data), "bytes")


def radius(x, y, w, h):
    dx = (x + 0.5 - w / 2) / (w / 2)
    dy = (y + 0.5 - h / 2) / (h / 2)
    return (dx * dx + dy * dy) ** 0.5


out = Path(sys.argv[1])


def home(x, y):
    t = radius(x, y, 450, 210)
    a = 0.09 * max(0.0, 1 - t / 0.62)
    return (255, 196, 120, round(a * 255))


def glow(x, y):
    t = radius(x, y, 150, 150)
    if t <= 0.45:
        f = t / 0.45
        r, g, b, a = 245 + (255 - 245) * f, 165 + (216 - 165) * f, 36 + (92 - 36) * f, 0.34 + (0.1 - 0.34) * f
    elif t <= 0.7:
        f = (t - 0.45) / 0.25
        r, g, b, a = 255, 216, 92, 0.1 * (1 - f)
    else:
        r, g, b, a = 255, 216, 92, 0.0
    return (round(r), round(g), round(b), round(a * 0.19 * 255))


png(out / "home_haze.png", 450, 210, home)
png(out / "logo_glow.png", 150, 150, glow)
