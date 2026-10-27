# Original action-backend arithmetic fixtures

Project-authored MIT graphs, with no game artwork, font glyphs or learned weights.
The valid graph accepts float32 `[1,1,64,128]` and returns `[1,7]`:

```text
logits[i] = 0.03*i + mean(input) * 0.1*(i-3)
```

Wrong-channel and wrong-class graphs must be rejected before inference; a NaN
bias must never return usable logits. The action and tile tensor contracts must
not accept each other's models. Success here validates numerical execution only.

Optional development regeneration: `python tools/generate-action-neural-fixtures.py`
with `onnx` and NumPy installed. CI and portable users consume the committed
fixture bytes without Python.
