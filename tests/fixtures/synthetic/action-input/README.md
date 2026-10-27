# Original action input geometry

Project-authored MIT rectangles, strokes and colors; no game artwork, font
glyphs or learned parameters. The declared word bounds are independent of the
production connected-component implementation. Pillow NEAREST and exact
fraction/ties-to-even dimensions generate the expected binary 128x64 masks.

Cases cover aspect ratio, both half-rounding directions, odd margins, holes,
disconnected strokes, tiny external decorations, internal marks, maximum input
size, color thresholds and empty versus unlocalized bright pixels. Preparing a
mask does not prove that a region contains a valid game action button.

Optional developer regeneration: `python tools/generate-action-input-fixtures.py`
with Pillow and NumPy. Runtime and CI use committed bytes without Python.
