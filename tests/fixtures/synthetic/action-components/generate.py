"""Regenerate original geometry and independent Pillow masks (development only).

Expected rectangles are specified by hand. No game pixels, model or production
localization code is used. CI consumes the saved PNG/mask files without Python.
"""
import json
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
CASES = [
    ("transitive", [[20,12,8,40],[68,12,8,40],[40,50,8,6],[56,54,8,6],[80,58,2,2]], [20,12,56,48], 736),
    ("all-sides", [[28,12,16,32],[68,20,16,32],[24,46,8,2],[82,14,6,2],[50,10,2,4],[50,50,2,6]], [24,10,64,46], 1072),
    ("box-without-ink", [[40,20,20,24],[38,18,5,1],[38,18,1,5]], [40,20,20,24], 480),
]
rows = []
for name, shapes, bounds, foreground in CASES:
    source = Image.new("RGB", (128,64), (0,0,0))
    draw = ImageDraw.Draw(source)
    for x,y,w,h in shapes:
        draw.rectangle((x,y,x+w-1,y+h-1), fill=(240,240,240))
    source.save(ROOT / f"{name}.png")
    x,y,w,h = bounds
    crop = source.convert("L").crop((x,y,x+w,y+h)).point(lambda v: int(v != 0))
    assert sum(crop.tobytes()) == foreground
    scale = min(120/w,56/h)
    width,height = max(1,round(w*scale)),max(1,round(h*scale))
    output = Image.new("L",(128,64))
    output.paste(crop.resize((width,height),Image.Resampling.NEAREST),((128-width)//2,(64-height)//2))
    (ROOT / f"{name}.mask").write_bytes(output.tobytes())
    rows.append(dict(name=name,bounds=bounds,foreground_pixels=foreground))
(ROOT / "cases.json").write_text(json.dumps(rows,indent=2)+"\n",encoding="utf-8")
