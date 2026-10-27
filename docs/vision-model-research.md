# 轻量识牌模型与 Windows 原生推理实验

## 目标和当前边界

目标是随便携包分发小型本机模型，用户无需安装 Python 或额外推理环境。当前只完成可复用的原生 ONNX 执行后端；模型准确率、类别映射、图像预处理、拒识校准和实时集成仍未验收，因此浮窗尚未默认启用此后端。

## 只用公有领域牌图训练

训练使用 [FluffyStuff/riichi-mahjong-tiles](https://github.com/FluffyStuff/riichi-mahjong-tiles) 提交 `26e127ba2117f45cdce5ea0225748cc0cfad3169` 的 37 类 PNG。作者将素材置于公有领域。网络从随机参数开始：三层卷积（12/24/32 通道）及两层全连接，共 61,517 个参数，输入为 NCHW float32 `[1,3,48,32]`，输出为 37 个原始 logits。训练框架只用于开发机；公开雀魂截图只用于本机诊断，没有进入训练。

| 实验 | 训练增强 | 三张已知手牌图，共 41 张 |
| --- | --- | --- |
| `dac0fe3e98d74322b7da10e7d18495b7` | 几何、笔画粗细、模糊和噪声 | 38/41 |
| `55ef8e8db1ec40a28012da637ba17586` | 增加灰色牌面和轻微色偏 | 39/41 |
| `841f8cc8c5f74d13af0a1902664e032d` | 增加局部形变及低分辨率增强 | 40/41 |
| `69d790c19a2b4d968039a5b18d569317` | 增加四方向训练 | 38/41 |

每个运行目录为 `.local-test-data/runs/local-cc0-cnn-<实验编号>/`，保存 `training.py`、`summary.json` 和 `weights.pt`；摘要包括种子、训练牌图 SHA-256、脚本及权重 SHA-256、训练曲线、每张探针的预测和错误。四次训练和两种失败均保留。全部实验使用同一小组已知诊断图，不能视为独立测试集准确率。

表中数值按复核后的赤五真值计算；历史摘要不覆盖。当前最好结果为 40/41，剩余一索误认成东。第三张图第 6 张实际为赤五索，之前误标普通五索，模型此处预测正确。合成增强样本接近满分不能证明真实泛化或置信度可靠，不能通过降低拒识门槛、只报最高标签或只引用最好一张截图来宣布识牌通过。该权重未进入仓库和便携包。

## Windows 系统后端

`vision-core::neural::WinMlTileSession` 使用操作系统的 `Windows.AI.MachineLearning`，CPU 执行，无外部推理 DLL。微软列出此接口从 Windows 10 1809 起提供；当前实测为本机 Windows 11 x64，其他目标环境仍须验收。[微软接口说明](https://learn.microsoft.com/en-us/uwp/api/windows.ai.machinelearning.learningmodelsession)

约定和校验：

- 本地模型文件上限 16 MiB，扩展名须为 `.onnx`，采用以 IR 版本字段开头的标准导出（IR 3–10）；明显非模型文件在调用系统解析器前拒绝。模型必须只有一个 float32 `[1,3,48,32]` 输入、一个 float32 `[1,37]` 输出，不接受动态形状或其他网络接口。
- 输入必须恰有 4,608 个有限的 `[0,1]` 浮点数，输出必须恰有 37 个有限 logits。
- 输入输出名称来自模型描述；后端不猜测类别顺序，也不把执行成功等同于识牌成功。模型清单与类别映射的验收必须由后续调用层完成。
- WinRT 初始化与释放留在创建线程；已有 STA 调用者可使用此同步接口，后端不会释放调用者的初始化。
- 该模块尚未接到浮窗的模板匹配回退，旧识别行为保持原状。

`tests/fixtures/synthetic/neural/` 中的四个原创微型 ONNX 图没有牌图或训练权重。六个原生测试检查独立算术参考、非法输入、错误维度、非有限输出、STA 生命周期及隔离进程中的连续加载/释放；版本和回归脚本通过 `vision-core --all-targets` 包含它们。可选的 fixture 再生成脚本需要开发机的 `onnx==1.19.1` 和 NumPy，CI 和便携包使用者不需要 Python。

本机诊断命令：

```powershell
cargo run -p vision-core --example inspect_neural --release --locked -j 1 -- <本机ONNX路径> <预处理tensor文件>
```

tensor 文件为 1–256 个连续的 little-endian `[1,3,48,32]` float32 输入。该工具只输出 logits 和执行时间，不生成麻将推荐。原始截图、预处理 tensor、实验模型及其诊断日志均只放在 `.local-test-data/`。

## 固定提交验证

干净提交 `36af3493d5525c1ececc062dcc1bb7bfcc463ea9` 通过版本测试 8 项和全项目回归 9 项，均无失败或跳过；本机摘要分别为 `.local-test-data/runs/version-v0.0.1-63fdf9487e274151a8c6a4f71a7f290f/version-tests.json`、`.local-test-data/runs/all-ba4ceda8743a4bb3ac908a5572290537/all-tests.json`。初始缺少后端模块时的失败日志保留在 `.local-test-data/runs/native-neural-tdd-9b9e19a/initial-failure.log`。

同一提交使用当前按修订真值计为 40/41 的实验权重导出 ONNX，在 Windows 系统 CPU 后端计算相同的 41 个预处理输入，与 PyTorch 最大 logits 绝对误差为 `2.55e-6`，最高类别全部一致，错误标签也一致。模型为 247,696 字节，SHA-256 为 `025d078f5dfb532d46a8507a93984605a693c94655c2f6a661836c06081c2e5d`。该次本机加载约 28.7 ms，41 次执行合计约 2.7 ms；不含捕获、预处理、三帧等待、规则计算或绘制，不是端到端延迟及干净 Windows 冷启动验收。

完整摘要、输入/模型/参考输出哈希、命令、日志和环境保存在 `.local-test-data/runs/local-native-onnx-d1a65549b5384ad88b0bf92cb72fb60f/`。摘要明确区分 `native_backend_passed: true` 与 `acceptance_verified: false`、`production_integrated: false`。

## 真值更正与 CI 失败记录

人工放大复核三张图中的所有五牌后，第三张图第 6 张更正为 `0s`。统一真值和 ROI 哈希保存在 `tests/fixtures/manifests/public-hands-v1.json`，不包含原始截图。旧训练、模板、34 类模型及原生对照摘要保持原样；复算记录为 `.local-test-data/runs/truth-reassessment-2ca62fea9f904283bf5e6816b02a6d7c/summary.json`，保留每个旧摘要的 SHA-256，并分别报告 34 类基础匹配和 37 类精确匹配。原生输出与 PyTorch 的数值对照不受标注更正影响，旧摘要中的 39/41 应按更正记录解释为 40/41。

提交 `f2507e5` 的 CI 运行 `40513099578` 中，版本测试通过，回归测试的非法模型用例出现一次 `0xc0000005 / STATUS_ACCESS_VIOLATION`，日志保留在 `.local-test-data/runs/ci-f2507e5-regression-failure/native.log`。不能用本机成功或其他 CI 检查通过覆盖该失败。后续增加扩展名、最小长度及标准 IR 头预检，使 README 和伪装成 `.onnx` 的文本在进入系统解析器前被拒绝；这只是格式预检，不是完整 protobuf 校验。本机五个原生用例连续并行运行 20 轮通过，但 `81b2fa4` 的 CI 运行 `40515530047` 再次失败，日志保存在 `.local-test-data/runs/ci-81b2fa4-regression-failure/native.log`，说明预检不足以解决崩溃。

将非法模型测试单独以 `--test-threads=1` 运行后，本机也复现了崩溃：连续加载、拒绝、释放模型使最后一个 MTA 被释放，投影的进程级工厂缓存随后引用过期工厂。增加的隔离子进程测试在修复前稳定失败，记录在 `.local-test-data/runs/native-lifetime-tdd-81b2fa4/initial-failure.log`；子进程有 15 秒超时，不会被其他并行用例的 MTA 意外保活。后端改用公共 `windows::core::factory` API 获取有作用域的工厂，并在所属 apartment 之前释放，避免复用跨生命周期的缓存；不泄漏进程级 MTA 使用计数。修复后的六个原生用例已在本机单线程、并行及隔离模式通过，服务器仍以新提交的 required checks 为准。

## 增加授权明确的合成画风

后续本机实验只增加自行生成的字形和鸟形变化，没有加入游戏截图训练。字体为 [Noto CJK](https://github.com/notofonts/noto-cjk) 提交 `f8d157532fbfaeda587e826d4cd5b21a49186f7c` 的简体 Sans/Serif Regular，许可证 OFL-1.1；字体文件仅用于本机生成合成图片。[OFL FAQ](https://openfontlicense.org/ofl-faq/)说明字体生成的图形不因此受字体本身的分发许可约束。

实验 `.local-test-data/runs/local-permissive-cnn-c4085215b2ce49558fcbffba9bbc4392/` 保持同一小网络，用修订后的统一真值检查三张已知手牌，最高类别为 41/41；第二张图最低分仍只有约 0.275。这不是通过真实识牌验收，权重仍未打包、未启用，尚需独立画风、未知输入、行动按钮、公共牌和置信度测试。训练输入为 `[37,40,3,48,32]`，压缩文件 SHA-256 为 `d012bcedc8fb842059c7cdcce98973830009a09370d33c41ecd5cf1b90f47468`，生成脚本、字体来源与哈希均保存在本机 `models/permissive-synthetic-v1/`。

独立检查又从官方 FAQ 取得 34 张带数字/字母提示和完整边框的说明牌图，逐张人工核对标签后才运行分类，未把它们加入训练。整图结果为 24/34；采用当前公共牌浅色连通区域定位后仍为 24/34，一筒还出现多个候选区域。一万误认二万的分数约 0.986，说明已知三张手牌的 41/41 不能证明跨画风泛化和高分可靠性。这些说明图不是现场对局，只作为独立画风压力测试；原始图片、来源 URL、哈希和失败结果仅保存在本机 `screenshots/official-card-fixtures/`、`runs/local-official-card-probe-5ae310bcb4054e9c8cd9eb2e99ffb9c7/` 与 `runs/local-official-face-probe-74481fcadf1f4a3c8a61125ceb9fcdd1/`，没有上传或删除失败样本。

## 生命周期修复后的固定提交

提交 `06c4b74f72af03226a1c99ecb3637854d78b56eb` 的版本测试 8 项和全项目回归 9 项均在本机通过，无失败或跳过，摘要为 `.local-test-data/runs/version-v0.0.1-6953ba37615541ec91ab670752680ba2/version-tests.json` 和 `.local-test-data/runs/all-3e89bff1e2b64ebf8d679b8c7fa2b284/all-tests.json`。该提交的 [CI 运行 40518212205](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40518212205) 的五项 required checks 均通过，其中包含隔离子进程生命周期测试；前两次失败记录继续保留。

同一干净提交的原生数值对照记录在 `.local-test-data/runs/local-native-onnx-a05fe85d96454938b6cdb94d2da1874a/`：最大 logits 误差仍为 `2.55e-6`，该次模型加载约 27.8 ms、41 次执行约 3.1 ms。此对照使用较早的 40/41 CC0 权重，不是新增混合字体权重的准确率或实时验收。
