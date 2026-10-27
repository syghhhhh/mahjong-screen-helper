# 轻量识牌模型与 Windows 原生推理实验

## 目标和当前边界

目标是随便携包分发小型本机模型，用户无需安装 Python 或额外推理环境。当前只完成可复用的原生 ONNX 执行后端；模型准确率、类别映射、图像预处理、拒识校准和实时集成仍未验收，因此浮窗尚未默认启用此后端。

## 只用公有领域牌图训练

训练使用 [FluffyStuff/riichi-mahjong-tiles](https://github.com/FluffyStuff/riichi-mahjong-tiles) 提交 `26e127ba2117f45cdce5ea0225748cc0cfad3169` 的 37 类 PNG。作者将素材置于公有领域。网络从随机参数开始：三层卷积（12/24/32 通道）及两层全连接，共 61,517 个参数，输入为 NCHW float32 `[1,3,48,32]`，输出为 37 个原始 logits。训练框架只用于开发机；公开雀魂截图只用于本机诊断，没有进入训练。

| 实验 | 训练增强 | 三张已知手牌图，共 41 张 |
| --- | --- | --- |
| `dac0fe3e98d74322b7da10e7d18495b7` | 几何、笔画粗细、模糊和噪声 | 37/41 |
| `55ef8e8db1ec40a28012da637ba17586` | 增加灰色牌面和轻微色偏 | 38/41 |
| `841f8cc8c5f74d13af0a1902664e032d` | 增加局部形变及低分辨率增强 | 39/41 |
| `69d790c19a2b4d968039a5b18d569317` | 增加四方向训练 | 37/41 |

每个运行目录为 `.local-test-data/runs/local-cc0-cnn-<实验编号>/`，保存 `training.py`、`summary.json` 和 `weights.pt`；摘要包括种子、训练牌图 SHA-256、脚本及权重 SHA-256、训练曲线、每张探针的预测和错误。四次训练和两种失败均保留。全部实验使用同一小组已知诊断图，不能视为独立测试集准确率。

39/41 的实验仍将一索误认成东、普通五索误认成赤五索，后者模型分数偏高。合成增强样本接近满分不能证明真实泛化或置信度可靠，不能通过降低拒识门槛、只报最高标签或只引用最好一张截图来宣布识牌通过。该权重未进入仓库和便携包。

## Windows 系统后端

`vision-core::neural::WinMlTileSession` 使用操作系统的 `Windows.AI.MachineLearning`，CPU 执行，无外部推理 DLL。微软列出此接口从 Windows 10 1809 起提供；当前实测为本机 Windows 11 x64，其他目标环境仍须验收。[微软接口说明](https://learn.microsoft.com/en-us/uwp/api/windows.ai.machinelearning.learningmodelsession)

约定和校验：

- 本地模型文件上限 16 MiB；必须只有一个 float32 `[1,3,48,32]` 输入、一个 float32 `[1,37]` 输出，不接受动态形状或其他网络接口。
- 输入必须恰有 4,608 个有限的 `[0,1]` 浮点数，输出必须恰有 37 个有限 logits。
- 输入输出名称来自模型描述；后端不猜测类别顺序，也不把执行成功等同于识牌成功。模型清单与类别映射的验收必须由后续调用层完成。
- WinRT 初始化与释放留在创建线程；已有 STA 调用者可使用此同步接口，后端不会释放调用者的初始化。
- 该模块尚未接到浮窗的模板匹配回退，旧识别行为保持原状。

`tests/fixtures/synthetic/neural/` 中的四个原创微型 ONNX 图没有牌图或训练权重。五个原生测试检查独立算术参考、非法输入、错误维度、非有限输出及 STA 生命周期；版本和回归脚本通过 `vision-core --all-targets` 包含它们。可选的 fixture 再生成脚本需要开发机的 `onnx==1.19.1` 和 NumPy，CI 和便携包使用者不需要 Python。

本机诊断命令：

```powershell
cargo run -p vision-core --example inspect_neural --release --locked -j 1 -- <本机ONNX路径> <预处理tensor文件>
```

tensor 文件为 1–256 个连续的 little-endian `[1,3,48,32]` float32 输入。该工具只输出 logits 和执行时间，不生成麻将推荐。原始截图、预处理 tensor、实验模型及其诊断日志均只放在 `.local-test-data/`。
