# Small-classifier input oracle

All pictures are original geometric patterns under this project's MIT license.
They contain no game screenshots, fonts, third-party tile art, or model weights.

`cases.txt` records expected crop rectangles and the number of removed corner
annotations. A negative x/y means no ink remained. Each `*.png` is an RGB input;
its `*.rgb` is the independently generated, interleaved RGB **32 × 48** result
before inversion. `SHA256SUMS` records both files. CI reads these committed bytes;
it needs no Python. The four integration tests run in both version and regression
vision suites through `cargo test -p vision-core --all-targets --locked`.

Development regeneration:

```powershell
python tools/generate-neural-input-fixtures.py
```

The saved oracle was generated using NumPy and Pillow **12.2.0** `Resampling.BOX`.
The source generator is the reference implementation. Rust is compared at every
channel/pixel after CHW conversion, not only at the predicted class. Nonintegral
up/down sampling and varied RGB values distinguish separable rounding from a
single truncated 2D average. The BOX filter's half-open sampling and two-pass
fixed-point arithmetic can be inspected in [Pillow's primary source](https://github.com/python-pillow/Pillow/blob/12.0.0/src/libImaging/Resample.c).

`rgb-inverse-glyph-box-v2` expects a bounded, upright, tightly cropped tile face,
already composited onto white if it had alpha. It removes only small isolated red
components at the far upper right, excludes orange edge pixels from ink bounds,
crops ink plus one pixel, resizes, and returns `1 - RGB/255` in CHW order. Large
and central red marks are retained. This heuristic is not proof that every
possible skin or red-five mark is preserved.

No ink yields an all-zero tensor with `glyph_bounds: None`. That alone cannot
distinguish White Dragon from an empty region, animation, or missed glyph. This
preprocessor does not establish confidence, select a class map, or enable any
model in the overlay. A model must declare this preprocessing identifier.
