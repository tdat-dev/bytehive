"""Bake the tray icon: the app's 32px logo as raw RGBA, so the native app
needs no image decoder for it. Run from native/: python tools/bake_tray.py"""
from PIL import Image

img = Image.open("../src-tauri/icons/32x32.png").convert("RGBA")
assert img.size == (32, 32)
open("assets/tray32.rgba", "wb").write(img.tobytes())
