"""Independent Pillow BOX oracle for aspect-preserving glyph inputs.

Only original geometric/color fixtures. Python/Pillow/numpy are development
dependencies; CI and the portable program consume the committed bytes.
"""
import hashlib
import importlib.util
from fractions import Fraction
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'tests/fixtures/synthetic/neural-letterbox'
spec = importlib.util.spec_from_file_location('glyph_oracle', ROOT / 'tools/generate-neural-input-fixtures.py')
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def reference(image):
    glyph, bounds, removed = oracle.glyph_face(image)
    scale = min(Fraction(28, glyph.width), Fraction(44, glyph.height))
    width = max(1, round(glyph.width * scale))
    height = max(1, round(glyph.height * scale))
    glyph = glyph.resize((width, height), Image.Resampling.BOX)
    canvas = Image.new('RGB', (32, 48), 'white')
    canvas.paste(glyph, ((32-width)//2, (48-height)//2))
    return canvas.tobytes(), bounds, removed


def main():
    DEST.mkdir(parents=True, exist_ok=True)
    cases = {}
    for name in ['blank', 'hint-only', 'hint-and-red-glyph', 'fractional-downsample', 'fractional-upsample', 'equal-size', 'preserve-red']:
        cases[name] = Image.open(ROOT / f'tests/fixtures/synthetic/neural-input/{name}.png').convert('RGB')
    for name, rect in [('narrow', (37, 17, 40, 110)), ('wide', (9, 57, 69, 59)), ('odd-center', (19, 20, 57, 71)), ('half-even', (17, 30, 70, 54)), ('half-even-down', (17, 30, 70, 52))]:
        image = Image.new('RGB', (80, 128), 'white')
        ImageDraw.Draw(image).rectangle(rect, fill=(13, 31, 67))
        cases[name] = image
    # Maximum supported dimensions and a one-pixel glyph exercise the fit clamp.
    for name, size in [('extreme-tall', (8, 1024)), ('extreme-wide', (1024, 8))]:
        image = Image.new('RGB', size, 'white')
        ImageDraw.Draw(image).line([(size[0]//2, size[1]//4), (size[0]//2, size[1]*3//4)], fill='black')
        cases[name] = image
    rows = ['# name x y width height removed_corner_hints; -1 means no glyph']
    hashes = []
    for name, image in cases.items():
        data, bounds, removed = reference(image)
        image.save(DEST / f'{name}.png')
        (DEST / f'{name}.rgb').write_bytes(data)
        values = [-1, -1, 0, 0] if bounds is None else [bounds[0], bounds[1], bounds[2]-bounds[0], bounds[3]-bounds[1]]
        rows.append(' '.join(map(str, [name, *values, removed])))
        for suffix in ['png', 'rgb']:
            p = DEST / f'{name}.{suffix}'
            hashes.append(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}')
    (DEST/'cases.txt').write_text('\n'.join(rows)+'\n', encoding='utf-8')
    (DEST/'SHA256SUMS').write_text('\n'.join(hashes)+'\n', encoding='ascii')
    print(f'Generated {len(cases)} original letterbox fixtures')


if __name__ == '__main__':
    main()
