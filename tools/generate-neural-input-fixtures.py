"""Original geometric fixtures + Pillow BOX oracle; development-only numpy/Pillow.

No fonts, tile artwork, screenshots, or learned weights are used. CI consumes
the saved RGB input PNGs and 32x48 RGB oracle bytes without running Python.
"""
from pathlib import Path
import hashlib
import numpy as np
from PIL import Image, ImageDraw
import PIL

DEST = Path(__file__).resolve().parents[1] / "tests/fixtures/synthetic/neural-input"


def glyph_face(image):
    pixels = np.asarray(image.convert("RGB")).copy()
    height, width = pixels.shape[:2]
    r, g, b = [pixels[:, :, i].astype(np.int16) for i in range(3)]
    red = (r > g + 25) & (r > b + 25) & (g < 210) & (b < 210)
    seen = np.zeros_like(red)
    pale = (pixels.min(2) >= 175) & (pixels.max(2).astype(np.int16) - pixels.min(2) <= 70)
    background = np.median(pixels[pale], axis=0).astype(np.uint8) if pale.any() else [240, 240, 240]
    removed = 0
    for sy, sx in np.argwhere(red):
        if seen[sy, sx]:
            continue
        todo = [(int(sx), int(sy))]
        seen[sy, sx] = True
        left = right = int(sx)
        top = bottom = int(sy)
        area = 0
        while todo:
            x, y = todo.pop()
            area += 1
            left, right, top, bottom = min(left, x), max(right, x), min(top, y), max(bottom, y)
            for xx, yy in [(x-1, y), (x+1, y), (x, y-1), (x, y+1)]:
                if 0 <= xx < width and 0 <= yy < height and red[yy, xx] and not seen[yy, xx]:
                    seen[yy, xx] = True
                    todo.append((xx, yy))
        if ((left+right)/2 >= width*.8 and bottom < height*.32
                and right-left+1 <= width*.25 and bottom-top+1 <= height*.2
                and area <= width*height*.03):
            pixels[max(0, top-1):min(height, bottom+2), max(0, left-1):min(width, right+2)] = background
            removed += 1
    r, g, b = [pixels[:, :, i].astype(np.int16) for i in range(3)]
    low, high = pixels.min(2), pixels.max(2)
    ink = ((high < 170) | ((high.astype(np.int16)-low > 70) & (low < 160))) & ~((r > g+30) & (g > b+25))
    mx, my = max(1, width//20), max(1, height//20)
    ink[:my] = False
    ink[-my:] = False
    ink[:, :mx] = False
    ink[:, -mx:] = False
    ys, xs = np.where(ink)
    bounds = None
    if len(xs):
        bounds = (max(0, int(xs.min())-1), max(0, int(ys.min())-1), min(width, int(xs.max())+2), min(height, int(ys.max())+2))
        image = Image.fromarray(pixels).crop(bounds)
    else:
        image = Image.new("RGB", (32, 48), "white")
    return image, bounds, removed


def reference(image):
    image, bounds, removed = glyph_face(image)
    return image.resize((32, 48), Image.Resampling.BOX).tobytes(), bounds, removed


def main():
    DEST.mkdir(parents=True, exist_ok=True)
    cases = {"blank": Image.new("RGB", (80, 128), (239, 241, 240))}
    image = cases["blank"].copy()
    draw = ImageDraw.Draw(image)
    draw.rectangle((68, 13, 72, 24), fill=(180, 30, 30))
    cases["hint-only"] = image
    image = image.copy()
    draw = ImageDraw.Draw(image)
    draw.rectangle((25, 38, 31, 83), fill=(13, 23, 33))
    draw.rectangle((43, 50, 51, 106), fill=(8, 90, 10))
    draw.rectangle((29, 47, 51, 53), fill=(164, 13, 24))
    cases["hint-and-red-glyph"] = image
    for name, size in [("fractional-downsample", (87, 113)), ("fractional-upsample", (17, 23)), ("equal-size", (32, 48))]:
        # Deterministic nonseparable RGB samples test both resize passes, channel
        # order, half-way rounding, and nonintegral box sample boundaries.
        width, height = size
        yy, xx = np.indices((height, width))
        rgb = np.stack(((xx*31+yy*17)%256, (xx*11+yy*47)%256, (xx*53+yy*7)%256), axis=2).astype(np.uint8)
        cases[name] = Image.fromarray(rgb)
    image = Image.new("RGB", (80, 128), "white")
    draw = ImageDraw.Draw(image)
    draw.rectangle((0, 0, 79, 4), fill=(210, 130, 50))
    draw.rectangle((28, 32, 37, 94), fill=(10, 10, 10))
    # Large red area at upper right is part of the glyph, not a hint.
    draw.rectangle((57, 13, 74, 45), fill=(185, 20, 30))
    draw.rectangle((18, 14, 22, 24), fill=(185, 20, 30))
    cases["preserve-red"] = image
    rows = ["# name x y width height removed_corner_hints; -1 means no glyph"]
    hashes = []
    for name, image in cases.items():
        rgb, bounds, removed = reference(image)
        image.save(DEST / f"{name}.png")
        (DEST / f"{name}.rgb").write_bytes(rgb)
        values = [-1, -1, 0, 0] if bounds is None else [bounds[0], bounds[1], bounds[2]-bounds[0], bounds[3]-bounds[1]]
        rows.append(" ".join(map(str, [name, *values, removed])))
        for suffix in ["png", "rgb"]:
            path = DEST / f"{name}.{suffix}"
            hashes.append(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}")
    (DEST / "cases.txt").write_text("\n".join(rows)+"\n", encoding="utf-8")
    (DEST / "SHA256SUMS").write_text("\n".join(hashes)+"\n", encoding="ascii")
    print(f"Generated {len(cases)} original fixtures with Pillow {PIL.__version__}")


if __name__ == "__main__":
    main()
