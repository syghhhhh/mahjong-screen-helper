# Original native-inference fixtures

These tiny ONNX graphs are project-authored synthetic test data under MIT.
They contain no Mahjong artwork, screenshots, trained weights, or third-party model.

`affine-37.onnx` averages each input channel, then returns:

```text
logits[i] = 0.03*i + sum(mean[channel] * 0.1*((i+channel)%5-2))
```

Its input is float32 `[1,3,48,32]` and its output float32 `[1,37]`.
Two graphs deliberately have one wrong channel/class dimension and must
be rejected before execution. `nonfinite-output.onnx` has a NaN bias and must
never return usable logits. ONNX opset 9 / IR 4 keeps these fixtures small
and compatible with the original Windows ML operator set.

Regeneration is optional developer tooling: `python tools/generate-neural-fixtures.py`
with `onnx==1.19.1` and NumPy installed. CI and portable users read the committed
files and do not need Python. Successful fixture inference does not establish
real Mahjong recognition accuracy.
