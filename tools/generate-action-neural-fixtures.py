"""Original arithmetic action-backend fixtures; development-only onnx/numpy."""
from pathlib import Path
import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

dest = Path(__file__).resolve().parents[1] / 'tests/fixtures/synthetic/action-neural'
dest.mkdir(parents=True, exist_ok=True)
for name, channels, classes in [('affine-7', 1, 7), ('wrong-channels', 3, 7),
                                ('wrong-classes', 1, 6), ('nonfinite-output', 1, 7)]:
    weights = np.array([[0.1 * (i + c - 3) for i in range(classes)]
                        for c in range(channels)], dtype=np.float32)
    bias = np.arange(classes, dtype=np.float32) * 0.03
    if name == 'nonfinite-output':
        bias[0] = np.nan
    graph = helper.make_graph([
        helper.make_node('GlobalAveragePool', ['pixels'], ['means']),
        helper.make_node('Flatten', ['means'], ['flat'], axis=1),
        helper.make_node('Gemm', ['flat', 'weights', 'bias'], ['logits']),
    ], name, [helper.make_tensor_value_info('pixels', TensorProto.FLOAT, [1, channels, 64, 128])],
       [helper.make_tensor_value_info('logits', TensorProto.FLOAT, [1, classes])],
       [numpy_helper.from_array(weights, 'weights'), numpy_helper.from_array(bias, 'bias')])
    model = helper.make_model(graph, producer_name='mahjong-screen-helper-synthetic-action-tests',
                              opset_imports=[helper.make_opsetid('', 9)])
    model.ir_version = 4
    onnx.checker.check_model(model)
    onnx.save(model, dest / f'{name}.onnx')
    print(name, len(model.SerializeToString()))
