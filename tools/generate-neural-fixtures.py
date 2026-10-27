"""Regenerate tiny original ONNX fixtures; requires development-only onnx/numpy."""
from pathlib import Path
import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

DEST = Path(__file__).resolve().parents[1] / "tests/fixtures/synthetic/neural"
DEST.mkdir(parents=True, exist_ok=True)
(DEST/"not-a-model.onnx").write_bytes(b"This is plain text, not an ONNX protobuf model.\n")
for name, channels, classes in [("affine-37",3,37),("wrong-channels",1,37),("wrong-classes",3,34),("nonfinite-output",3,37)]:
    weights = np.asarray([[((index+channel)%5-2)*0.1 for index in range(classes)] for channel in range(channels)],dtype=np.float32)
    bias = np.arange(classes,dtype=np.float32)*0.03
    if name == "nonfinite-output": bias[0] = np.nan
    graph = helper.make_graph([
        helper.make_node("GlobalAveragePool",["pixels"],["means"]),
        helper.make_node("Flatten",["means"],["flat"],axis=1),
        helper.make_node("Gemm",["flat","weights","bias"],["logits"]),
    ],name,[helper.make_tensor_value_info("pixels",TensorProto.FLOAT,[1,channels,48,32])],
        [helper.make_tensor_value_info("logits",TensorProto.FLOAT,[1,classes])],
        [numpy_helper.from_array(weights,"weights"),numpy_helper.from_array(bias,"bias")])
    model=helper.make_model(graph,producer_name="mahjong-screen-helper-synthetic-tests",opset_imports=[helper.make_opsetid("",9)])
    model.ir_version=4
    onnx.checker.check_model(model)
    onnx.save(model,DEST/f"{name}.onnx")
    print(f"{name}.onnx: {len(model.SerializeToString())} bytes")
