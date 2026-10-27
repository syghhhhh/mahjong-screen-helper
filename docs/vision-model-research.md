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

## 小分类器的 Rust 像素预处理

检查 FAQ 说明图后发现，右上角的小红色数字或字母也进入了字形包围盒。保留原混合字体权重，只在本机 Python 探针中去掉符合位置、大小和连通条件的角标，结果先为 32/34；放宽角标宽度以覆盖西牌的 W 后为 33/34，仍把八筒误认一索，分数约 0.269。两次摘要分别为 `.local-test-data/runs/local-corner-hint-probe-8419edd5ce1f4ad79cd6390455dc11d9/summary.json` 和 `.local-test-data/runs/local-corner-hint-probe-cceb8e906f5748f7b266d8dc20bcf5b9/summary.json`。原来的 24/34 失败结果继续保留。这组图已经参与预处理调整，之后属于开发压力测试集，不能再宣称为未见的独立最终验收集。

新增跨平台 `vision-core::neural_input::prepare_tile`，预处理标识为 `rgb-inverse-glyph-box-v2`。它接收 8–1024 像素宽高的直立牌面 RGB，复制后处理，不读写文件；alpha 须由调用方先合成到白色。步骤为：

1. 检测红色四连通块；只去掉位于最右侧、上方且面积和宽高均较小的块，以浅色背景中位数填充。中央红色、左侧红色和大的红色字形保留。该启发式仍须用不同赤五牌面验证。
2. 忽略外圈 5% 及橙色边缘对包围盒的影响，取墨色包围盒并加一像素边距。
3. 用分离的 BOX 重采样、22 位定点系数和逐遍 8 位舍入得到 32×48 RGB，再生成 CHW `[3,48,32]` 的 `1 - RGB/255`。不能用一次二维平均截断来替代，否则输入与训练时不同。
4. 无墨色时输出零 tensor，同时返回 `glyph_bounds: None`；该结果不能证明识别到了白板。模型映射、空白验证、未知输入拒识和置信度校准仍需调用层完成。

七组合成几何图片和 Pillow 12.2.0 参考字节保存在 `tests/fixtures/synthetic/neural-input/`；无游戏牌图、字体和模型权重。四个新增 Rust 测试检查每个像素/通道的数值一致性、边界尺寸、输入不被修改、角标去除及红色信息保留，随版本和回归测试运行。初始模块缺失的失败日志保存在 `.local-test-data/runs/neural-input-tdd-a9f96b9/initial-failure.log`。开发时可用 `tools/generate-neural-input-fixtures.py` 再生成；运行程序和 CI 不依赖 Python。

诊断命令 `cargo run -p vision-core --example inspect_neural_faces --release --locked -j 1 -- <ONNX> <新tensor文件> <牌面PNG> ...` 接受 1–256 张本机 RGB/RGBA PNG，保存实际 Rust 输入并输出原生 logits。目标 tensor 文件必须不存在，旧证据不得被覆盖。它没有生产类别映射、不输出麻将推荐，也未接入浮窗；模型清单必须绑定预处理版本后才能用于后续集成。

## 预处理固定提交与颜色增强

干净提交 `e0d48aa475818b09848938e62932f348c160ebcd` 通过本机版本测试 8 项、全项目回归 9 项，均无失败或跳过；摘要为 `.local-test-data/runs/version-v0.0.1-2c97931c46dd4e9bb76c2b342c674cde/version-tests.json` 和 `.local-test-data/runs/all-9f81b8fc1e6b4e789c0ff38f41f65200/all-tests.json`。该提交的 [CI 40522023234](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40522023234) 五项检查均通过，包含新增像素对照及原生生命周期测试。RGB/tensor/ONNX fixture 在 `.gitattributes` 标为二进制，避免 Windows 换行规范化破坏参考字节。

同一提交对旧混合字体权重进行 112 张输入的双实现对照：三张已知手牌 41 张、开发 FAQ 图 34 张、训练源牌图 37 张。Rust tensor 与 Pillow 参考逐值相同，原生输出与 PyTorch 最大 logits 误差约 `4.97e-6`，分别为 41/41、33/34、37/37。摘要为 `.local-test-data/runs/local-neural-input-v2-3e7fef7fb30d45558beb01d3bf9d190d/summary.json`；唯一移除角标的已知手牌为四筒边缘的一小块红色，实图赤五索和训练源三种赤五均未移除红色块。该检查不能证明所有皮肤的赤五标记均安全。

随后只增加蓝色墨迹向黑色变化的合成增强，保留红色和绿色信息；没有加入游戏截图训练。训练记录为 `.local-test-data/runs/local-pip-color-cnn-79cd5b2f31ea42f582d4475957d31bb2/`，权重 SHA-256 为 `9987d02875520a2327c74b32c796a7e78f06bfd0773020f3c473000504a57ec8`。在同一 112 张开发输入中，结果为 41/41、34/34、37/37；已知手牌最低分约 0.766，FAQ 最低分仍仅约 0.500。原生 tensor 完全一致、最大 logits 误差约 `4.24e-6`，摘要为 `.local-test-data/runs/local-neural-input-v2-e27aa13e6e65406b81c027a079e9a14d/summary.json`。ONNX 为 247,696 字节，SHA-256 为 `c37b86d95b3ff580b4f6c254804d8d3c61444c10fc15d22e83d51a9c4a6ae931`，仍仅在本机实验目录，未打包、未启用。

## 新公开手牌与真值复核

在颜色增强权重冻结后，从另一位作者的[公开四麻牌谱画面](https://note.com/okuohouko/n/nfd1e9a38abb3)取得四张未用于上述训练及调参的画面，先保存人工标签再推理。Rust 自动定位每张图的 14 个牌槽，共 56 张。初始结果为 52/56，四个不一致均为重复出现的同一种筒子；放大原图可数出“上方两黑、下方四红”六个圆，原文第三巡说明的三/四/六筒也与此一致。初始人工标签把六筒误记五筒，属于标注错误，不能作为模型高分误识的证据。

旧标签及初次输出保存在 `.local-test-data/unseen-hands-preinference-truth.json` 和 `.local-test-data/runs/local-unseen-hands-0ae2a1cce9594ceabe35082c9068b1de/`，未覆盖。更正真值、ROI/原图哈希及理由保存在 `tests/fixtures/manifests/public-hands-v2.json`；复算摘要为 `.local-test-data/runs/unseen-truth-reassessment-974c5a8dfb474093924458cee973bb1b/summary.json`，包含旧摘要、新真值哈希，按更正标签为 56/56，权重、输入、预测均未改变。四张来自同一局的相关画面不等于四个独立对局，更不等于完整 37 类、赤五、行动按钮或现场验收。

这次 56 次原生执行约 6.5 ms，读 PNG 与预处理合计约 11.2 ms、加载约 29 ms；这些是开发机离线探针时间，不含捕获、三帧等待、牌效与显示，不能证明 250 ms 端到端目标。

## 未知输入拒识尚未通过

冻结颜色增强权重，另生成 15 个非麻将字形和 4 个纯色区域作本机压力测试，记录在 `.local-test-data/runs/local-unknown-neural-probe-06e950043a4c4ed3bd1145a3f0448e8b/summary.json`。在假设的 `softmax ≥ 0.9` 且前两名差值 `≥ 0.2` 门槛下，仍有 6/19 个非牌面会通过，包括 Y→四万约 0.9998、暗区域→八筒约 0.9924；空白浅色区域也会得到白板约 0.983。该门槛仅用于暴露问题，没有写入生产配置。

因此不能只凭已知与新手牌命中率启用模型。后续须补上未知输入/无效牌面拒识、白板上下文确认、模型清单及类别/预处理绑定，再进行集成和现场验收。视觉任务继续 `in_progress`；旧高分错误和本次真实的未知输入失败均保留，不能通过调低门槛或忽略失败将其标记成功。

## 拒识训练与本机分类器接口

参照 [Outlier Exposure 原论文](https://arxiv.org/abs/1812.04606)中对异常样本学习均匀分布的思路，新增本机实验使用自行绘制线条和 OFL Noto 字形作辅助负样本，不复制其代码。498 个合成输入明确排除之前 15 个压力字形的字符；不将空白零 tensor 当负样本，以免与白板冲突。负样本来源与脚本哈希在 `.local-test-data/models/original-outliers-v1/`，这些实验不是完整独立未知分布验收。

首轮 `local-outlier-cnn-6bb48a5e2f2f4107bc6383a577276df5` 保持 37 类、61,517 参数，已知手牌最高标签仍为 41/41，FAQ 降为 33/34，真实赤五索分数降至约 0.125。19 个原压力样本在假设 0.9 / 0.2 门槛下只剩空白区域高分；输入和原生数值对照为 `local-neural-input-v2-b0c9e15e30c34c4f8278329c14dfdd6a`，压力摘要为 `local-unknown-neural-probe-3c004b600dc34187bc419992ef19961d`。这说明训练改善部分拒识，却损害有效牌面的置信度，不能只报告拒识数字。

随后自行用椭圆、杆和圆角几何图形绘制普通/赤五索各 40 个变化，没有从游戏图片裁取纹理。来源为 `.local-test-data/models/permissive-bamboo-v2/`。第二轮 `local-bamboo-outlier-cnn-096672c8e3154ad79b647443a41bb529` 仍为已知手牌 41/41、FAQ 33/34；赤五索改善但最低分仍约 0.318。原生 tensor 逐值一致，最大 logits 误差约 `4.29e-6`，摘要为 `local-neural-input-v2-3c7578c4d1734e9ba67898c8d2f2ea27`；压力结果 `local-unknown-neural-probe-4bb5928a76d1468d99eb11b9898a913b` 仍只有无上下文空白高分。两轮模型均未打包、未启用，历史旧模型和所有错误继续保留。

新增 `ModelManifest`、`FaceObservation` 和 Windows `LocalTileClassifier` 实现明确的本机模型契约：限定文件名、实际 SHA-256、类别顺序、预处理版本和显式阈值；读取时保持模型文件写入/替换锁，使用稳定数值运算并明确拒识；空白须有同一手牌观察中的四边牌槽证据。详见 [清单与拒识接口](model-manifest.md)。这仍不替代目标游戏白板、遮挡、未知输入与动作识别验收，且还没有接入浮窗。

新增八个测试覆盖清单/决定/白板几何、系统哈希和本机加载。初始缺模块的 TDD 日志在 `.local-test-data/runs/classifier-contract-tdd-a1a0dd9/initial-failure.log`。另保留 `first-behavior-failure.log`：最初的并列分数用例因其最高概率略小于 0.5，先触发低概率拒识，未隔离到预期的差值检查；把第二名从 20 改为 19.98，使最高概率略高于 0.5 而差值不足，分别验证两条规则，没有放宽实现阈值。

## 清单与拒识的固定提交验算

干净提交 `905546eed6461086f615d104ce32a34b20cdbed5` 通过本机版本 8 项、完整回归 9 项，无失败或跳过。摘要为 `.local-test-data/runs/version-v0.0.1-0d43f3e9a2d747508bd24e3d0a9986e7/version-tests.json` 与 `.local-test-data/runs/all-a1920d19c13f4466894fa33245068882/all-tests.json`；[CI 40527423010](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40527423010) 也已完成，五项检查均通过。新功能复用已有锁定的 serde 和 serde_json，没有增加注册表依赖种类；许可证收集仍覆盖 50 项。

同一提交用第二轮辅助负样本模型，通过新清单加载器及实际 Rust 决定规则检查七张公开手牌，共 97 张。保持 `min_probability: 0.8 / min_margin: 0.1 / max_normalized_entropy: 0.4`，接受 81 张且已接受标签无误、拒绝 16 张；8 张真实白板均通过同帧边界检查，19 个开发非牌面全部拒绝。证据为 `.local-test-data/runs/local-bound-classifier-0d7c065a168d4963ae6c4d900828f1ab/summary.json`，包含实际清单和模型哈希、源提交、所有输入哈希、命令及逐例原因。七张牌局画面均至少有一张被拒，因此不能把“没有接受错牌”解释为整手可用；这份证据也不是浮窗实时验收。

## 开发集单参数校准

另参照[温度缩放原论文](https://proceedings.mlr.press/v70/guo17a)，尝试只校准输出 logits 的单个温度，不改变网络权重、类别、输入预处理或接受门槛。本机 `calibrate-outlier-temperature.py` 在预先列出的 12 个温度中，最小化开发集真牌 NLL 加 0.5 倍未知输入均匀分布 KL 的自定义目标；这项联合目标是本项目实验，不是声称原论文证明它适合麻将。

拟合数据明确为前三张已知手牌的 41 个 logits、已参与调参的 FAQ 34 个 logits、18 个已有非牌面。空白区域由几何规则处理，不纳入未知分布拟合；四张新作者的 56 张牌未用于选择温度。虽网络权重仍仅由合成素材训练，这个温度确实使用公开画面的开发 logits 拟合，不能宣称整个模型未接触真实画面数据。

选择温度为 0.8，开发目标约 0.1354；完整候选数值、脚本和来源哈希位于 `.local-test-data/runs/local-temperature-calibration-e0272d45d2ad4a59a72ad3770a4d059a/summary.json`。新 ONNX 的 SHA-256 为 `047619fa6f677b694c59cdfa7e8b31a9f3a6449681a9468ae69122fa7ce5b5d3`，只在本机存放。112 个输入的 Windows 原生输出与 PyTorch 参考除以 0.8 的最大误差约 `5.23e-6`，最高类别均一致，数值证据为 `local-calibrated-native-parity-0615c5eb5be04e7580d63073f698f48b`。

仍在同一干净提交、保持 0.8 / 0.1 / 0.4 门槛，重新检查七张手牌：接受 94/97，已接受标签无误；赤五索及两个相关帧中的九筒共 3 张仍被拒绝。8 张白板均验证通过，19 个开发非牌面全部拒绝。证据为 `.local-test-data/runs/local-bound-classifier-bf78fe6d65694546bb1f14723d4531c2/summary.json`。原先 81/97 的摘要、两次辅助训练的低分和错误继续保留。新作者四帧中有两帧仍不能整手提交，不能以单牌 94/97 替代整手可用率。

该校准只改善有限开发样本的接受率，不能证明未见字形、其他皮肤、遮挡、公共牌、动作按钮或现场连续识别已通过；仍须额外冻结模型并检查新来源。所有训练与校准候选均未加入便携包，V001-003、005、006、007、008 继续 `in_progress`。

## 实时回退接口

`FaceClassifier` 将模型决定接口与运行时分离，Windows 实现继续使用已校验的 `LocalTileClassifier`。`TemplateBank` 为手牌及公共牌增加可选回退，保留无模型时的旧路径。只有不确定牌槽调用模型；模型接受结果还须满足调用方门槛和有限分数检查，不能漏掉失败槽位、绕过赤五区别或超过四张限制。公共牌的所有已接受方向必须同类，方向矛盾或后端失败则不提交该牌。

`RuntimeEngine` 持有可选模型并在手牌、牌河、四家副露和指示牌上统一使用；拒识仍走原有不确定状态，清除建议缓存和来牌。更换/移除模型重置三帧过滤。Windows 入口从明确安装的模型清单加载；牌图不足但模型有效时可用完整按钮模板启动，坏模型不会被静默忽略。接口、路径和限制见 [模型安装说明](model-manifest.md)。

12 个新测试覆盖快速路径不调用模型、逐槽回退、分数/执行拒绝、方向矛盾、重叠与四张上限、旋转白板、三帧确认、旧建议清除、暂停/移除，以及三种启动模式和坏模型。失败日志保存在 `.local-test-data/runs/hybrid-recognition-tdd-1f15ef9/`；包括缺接口的 TDD 失败，以及修正前的 Rust 浮点字面量和 Clippy 测试代码风格错误。没有跳过测试或放宽拒识来消除这些错误。

复核另发现模型切换后的跨源换局问题：旧换局基线仍能与新模型的稳定手牌、公共牌比较，误触发 `RoundEnd`。第 13 个回归测试用两个识别源解释的变化复现，失败日志为同目录 `cross-model-round-failure.log`。修复在 `set_tile_classifier` 清除换局基线，保留当前局规则状态与原本的三帧确认；不能用换局重置来掩盖模型解释变化导致的不一致。

打包检查另发现此前开发 exe 直接导入 `VCRUNTIME140.dll`。新增 DLL 审计在修复前拒绝它，日志为同目录 `initial-runtime-import-failure.log`。配置 Windows x64 `target-feature=+crt-static` 后，实际导入列表不再有 VC runtime DLL；依据 [Rust 链接说明](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes)切换链接方式，并按其建议检查最终二进制。审计使用[微软 DUMPBIN](https://learn.microsoft.com/en-us/cpp/build/reference/dependents?view=msvc-170)，只在构建机执行。

版本打包测试还编译一个原创的动态 CRT 负例，确认它因运行库依赖被拒，而不是因为工具缺失或无法解析而误通过。从仓库外调用整个打包测试也通过，记录为同目录 `static-runtime-package.log`；输出目录中总文件约 1.82 MB，未包含负例、Python 或其他开发运行时。这只是开发机的打包与导入检查，不是干净 Windows 或现场识牌验收。

## 回退集成后的实图链路与 CI 差异

集成提交 `76d446854b93805b95154d062fa68e183b1b3881` 的 [CI 40532258615](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40532258615) 五项通过；补充跨模型基线修复的 `e3f7f7fed8b3a24054683fda0fe107d9fc3e7333` 的 [CI 40533562248](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40533562248) 也全部通过。静态 CRT 提交 `8e446849909415b0e6417218c312a3853690bee5` 本机版本 8 项、回归 9 项无失败或跳过，摘要为 `version-v0.0.1-438e536e3fc74ec5a5670b17a61cf590/version-tests.json`、`all-f17a7e7745f04139baa10b22b20c4b47/all-tests.json`（均在 `.local-test-data/runs/`）。

同一干净静态 CRT 提交使用实际 Windows 模型后端、三帧过滤、状态机和牌效核心检查七组公开手牌：4 组在第三帧输出正确整手与合法 Top-3，3 组因先前已记录的低分牌拒识，不输出推荐；由非麻将字形组成的整手也保持不确定。证据为 `.local-test-data/runs/local-model-runtime-787285ff5b7140d182eed80fbb898e42/summary.json`，记录模型与输入哈希、源提交、命令和逐帧输出。单帧离线处理该次最大约 12.2 ms，不含捕获、真实帧间隔或绘制。该工具未输入行动按钮和公共牌，不能代表八区域现场集成或 250 ms 验收。

然而 `8e44684` 的 [CI 40535547611](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40535547611) 中，独立 `package-smoke` 失败，其他四项通过。失败发生在动态 CRT 负例的原因验证；实际便携 exe 的 DLL 审计已通过。服务器原始日志保存在 `.local-test-data/runs/ci-8e44684-package-failure/ci-8e44684-package-job.log`。本机改用与独立检查相同的 PowerShell 7 宿主后也复现，记录为 `hybrid-recognition-tdd-1f15ef9/pwsh-package-reproduction.log`；该次负例子进程的原始错误显示找不到 `Get-FileHash`，而不是发现 VC runtime，因此测试正确地拒绝假成功。

把 `Start-Process` 子调用改为与版本测试一致的直接 `powershell.exe` 调用，并检查非零退出码和明确的 VC runtime 原因后，两种 PowerShell 宿主均可工作；修复后的本机 PowerShell 7 记录为同目录 `pwsh-package-fixed.log`。CI 增加始终收集负例日志的步骤，只允许 `version-<GUID>/import-control/expected-rejection.log`，不扩大到任意包内文件；微型仓库回归同时验证有/无 runs 目录时，截图、模型、exe 及相邻同名日志都不会上传。新提交仍须重新通过全部 required checks，不能用本机成功覆盖已失败的服务器运行。

后续 `ce88a94` 的打包步骤已完成所有检查，却因 GitHub pwsh 包装器继承预期负例的退出码而失败；本机用相同包装器复现 1，全部断言之后显式成功退出后为 0。该轮回归另有一次错误维度 fixture 的非预期返回值，原始日志保留在 `ci-ce88a94-failure/`。`b8e7104658d81a2d6bd3784734d20e60f0f8c444` 增加具体 fixture/实际错误诊断而保留原断言，本机版本 8 项、回归 9 项及 [CI 40539478406](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40539478406) 五项全部通过。120 次独立本机进程也未复现；这仍不证明已经找到原生返回差异的根因，后续若再失败应使用新增诊断追查。

## 真实牌河的光晕分割

旧公开牌河图上的白板与五索被浅绿色光晕连接：原候选 `[283,65,50,40]` 混入下一张牌。公共亮色分割排除已有高亮判定的绿色像素后，白板边界为 `[283,65,42,21]`，相邻五索保持独立。新增原创合成图先复现连牌失败，再验证四种旋转、完整牌面尺寸、共享光晕歧义拒绝及单独高亮保留；原始失败记录为 `ci-ce88a94-failure/highlight-initial-failure.log`。

人工标注 `tests/fixtures/manifests/public-board-v1.json` 在首次实际模型牌河推理前复核，包括放大旋转检查九筒、九万与五索。原图与 RGB 只保存在本机，仓库仅保留来源、SHA-256、边界与牌码。`inspect_model_board` 按实际门槛逐方向输出结果，整组仍沿用现有严格判定。该来源参与过模板和分割开发，不能列为独立最终验收集。

干净提交 `e28dfac973e75114c64bf5859b8dd901d67dc533` 的实际原生模型牌河探针为 18 个边界与人工复核一致，6 张通过、12 张拒绝、已接受标签无误；整组拒绝，来牌为空。证据为 `.local-test-data/runs/local-model-public-board-fd2443efb67045c7bb6863e4a5cb1824/summary.json`，保存源 SHA、模型/清单/真值/图像哈希、命令和完整方向诊断。本机完整版本 8 项与回归 9 项通过，回归摘要为 `all-31ff7d0add5840a2bd4b2fdc265cef95/all-tests.json`。候选通过率不是准确率达标，当前公共牌模型仍不足以支撑默认自动建议。

补充反例复现整张浅绿覆盖牌被误作空区域，失败记录为 `ci-ce88a94-failure/washed-face-initial-failure.log`。公共识别现在保留原亮色候选中没有分离牌面对应的可疑区域并拒绝；测试覆盖单独覆盖牌及旁边有正常牌，防止绿色排除把未识别牌变成空桌。合并、遮挡和未定位的真实牌面仍需继续验证。

## 阴影定位与新来源基准

提交 `03a20d82a7b9ff79c6d1e1eb81a64c2046a77bf9` 的 [CI 40541524170](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40541524170) 五项通过。随后从另一位作者的[公开回放文章](https://note.com/ymdesign/n/n5e7c34e4b3a2)选第一张画面，在首次推理前冻结原图/ROI/人工标签/模型哈希。放大旋转复核上方六万与左侧北，逐个数筒索，标明手牌二索被回放标记部分遮挡。真值为 `tests/fixtures/manifests/public-scene-v1.json`；该文章的相邻帧不能算额外独立对局。

初始实际 Windows 模型探针 `.local-test-data/runs/local-independent-scene-b4c468c3922d49ba86b6772c06271990/summary.json` 显示公共牌只定位 8/10，接受 4 张，其中红中被误认一万；手牌接受 11/14、已接受标签无误，手牌加六组公共图片的静态链路未形成 Ready，也没有 Top-3。该诊断缺少按钮输入、采集、UI、真实帧间隔和连续对局，不能代表现场集成通过。

像素连通检查找到两种漏定位：密集索子背景不足以通过原亮度扫描，以及灰色遮罩下的南。独立的较暗候选扫描保留原亮牌边界，只补没有已有亮牌对应的区域；绿色遮挡、大块灰面仍拒绝，白板的中心亮度及边界证据继续独立检查。首次两个合成失败日志为 `.local-test-data/shadow-public-initial-failure.log`。五个新测试覆盖定位、原边界、未知遮挡、白板约束和运行时清除旧建议及三帧恢复。

开发探针 `local-independent-scene-e51e8936728246cb8862e2f2eec14097` 恢复到 10/10 定位，仍接受 4 张、误认 1 张，整帧保持不确定。该图片已用于定位亮度选择与一个本机凸包背景原型，故由初始跨来源基准转为开发集；原始冻结真值另存 `public-truth-reassessment-80c3a3edcb4f49ccba6d7ca6a5bed89d/initial-independent-scene-truth.json`，用途变化记录在清单中。凸包原型仅改善个别牌，没有消除误认，尚未加入生产路径。不能将这次开发后的复测作为独立准确率。

## 旋转候选失败与公共真值修订

仅用已有 CC0/OFL 合成正例、原创负例和旧合成权重进行一次预注册的四方向增强实验，固定 4000 步、种子及沿用的温度 0.8，检查最终权重，不按实图挑选中途 checkpoint。候选 `local-rotation-invariant-cnn-50854fcba0a9453b899bc7f1641f3d24` 仍为 61,517 参数、247,696 字节，模型 SHA-256 为 `f531d4efa3499a44376572714b6e36850220716db59a01728e7b271356149745`。训练和导出完成后写摘要时发生 `NameError: Path`；原失败脚本及日志保留，通过单独元数据恢复脚本核验已落盘文件，没有重新训练或覆盖失败记录。

原生决定探针 `local-bound-classifier-88f6ad07c50d4b6fb82a860f9272bc02` 仅接受 76/97 开发手牌、无已接受标签错误，19 个开发非牌面均拒绝；旧候选为 94/97。公共探针 `local-model-public-board-f2f5fe546939484a8e38b5abf6216623` 接受 12/18 却误认 2 张：西与一筒均被报南。该候选不进入默认程序，分数升高不能替代识别正确。最先遇到误认而非零退出的运行 `local-model-public-board-5c5c7cbd81cc4d1c8e561255669fe41b` 的原始日志也保留。

放大公共牌预处理图时发现 `river-12` 的数字与萬字均为红色，应为 `0m`，此前标为 `5m`。同图其他万牌及官方普通五万图的数字为黑色，可供对照；修订未取自模型输出。清单保留旧标签、旧真值哈希及 Git/运行引用；`public-truth-reassessment-80c3a3edcb4f49ccba6d7ca6a5bed89d/summary.json` 保留放大图、旧摘要哈希和单独复算，两轮旧模型已接受标签错误数仍为 0，修订牌原来均被拒。真实截图、候选和所有实验日志仍只在本机。

## 外形方向与较大合成候选

提交 `8599cdd689949eaf92254912d1bffa4b8f92cf2f` 的 [CI 40546252433](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40546252433) 五项通过。后续逐方向诊断发现，旧候选在新来源的红中误认，以及部分新候选在旧牌河的西误认，发生在把横向长条作为直立牌面解释的方向。公共模板及模型现在只采用旋转后高宽比 0.9–2.5 的解释；一般为互相倒置的两个方向，近方形仍保留四个解释，符合外形的类别冲突继续拒绝。没有可用方向的区域不能因诊断零分门槛而通过。

两个新增像素/接口回归先复现错误，再检查四种屏幕朝向的有效方向恢复、仅错误长短边高分的拒绝、正方形歧义和细长区域。原失败日志为 `.local-test-data/public-axis-initial-failure.log`、`public-strip-initial-failure.log`。旧模型开发复测 `local-independent-scene-ccba0da9078a4503b57d0fbfc88e2158` 接受 3/10 公共牌、已接受标签无误，旧牌河 `local-model-public-board-6632de06d1aa4083b09268b681302dc8` 仍为 6/18、已接受标签无误；整组均不确定。这些探针记录 `source_dirty: true`，不作任务成功证据，方向筛选也不能证明新输入一定正确。

新增训练视图使用 CC0 牌图、原有 Noto 两种字体及 OFL 的 Ma Shan Zheng、LXGW WenKai。先查 CMap，再生成实际存在的字形、自绘竹形、四方向透视、低分辨率和灰色遮罩；不含游戏截图像素。最初 `public-synthetic-v3` 的字体序号与方向、验证序号存在排程相关性，在训练前弃用，原数据和 `quality-note.json` 保留。修正的 `public-synthetic-v4` 有 37×128 个视图，每类固定 112 个训练、16 个合成验证，方向与字体解耦；空白训练脸还须通过零字形检查。数据 SHA-256 为 `b4e668add5ced544aaca33c7d9aaa81bb05b033a214fa1fc99eccad43fae3519`。验证图仍来自相同素材/字体家族，不能当独立实图准确率。

一次固定 8000 步的较大 CNN 实验为 240,309 参数、962,873 字节，候选 `local-public-v4-cnn-745e01b259424795b595cf18120baa61` 的 SHA-256 为 `06f4e66c827c1b468dae7849c6f0e1c24f7b158342e642a3bc5cfe67dcc2adeb`。只检查最终权重，温度沿用 0.8；原生手牌探针 `local-bound-classifier-b52cd6fca9c844d98335a4279dc7eec3` 接受 77/97、已接受标签无误，19 个开发非牌面均拒绝。方向筛选前，旧牌河接受 14/18、误认 1；筛选后 `local-model-public-board-1b1f292dab9540a6853631534bc632ca` 为 11/18、无已接受错误。新来源开发图 `local-independent-scene-2e85f5b30e3a413faf833b8521795b7e` 为公共 6/10、手牌 12/14、无已接受错误，整帧仍不确定。

再用同架构混合 50% 旧直立合成视图、50% 新视图，固定 4000 步得到 `local-public-v5-cnn-841d01cac51b43068d93e83fb384ce92`，SHA-256 为 `a5927b1ea5592f8884dc0081725ace199e0f0af6d038125fb78b8e5d1894d165`。`local-bound-classifier-48a34518a8564e4694441afe97280319` 接受 80/97、已接受标签无误，19 个开发非牌面均拒绝；但 `local-model-public-board-2d5e6b08169e49a9a36599f699f8e132` 即使经过外形筛选仍把西报南，接受 13/18、误认 1。新来源开发图仍没有形成完整 Ready。两个候选都未达到部署条件，未加入便携包；后续仍需改进字体/牌形泛化并冻结后检验新来源，不能用合成验证或较高部分接受率代替验收。

## 字牌编码复核

检查训练字体映射与画面时发现，旧牌河 `river-09` 和新来源手牌第 12 张虽然都被读作“西”，数字编码却录成 `4z`。正确编码为西 `3z`、北 `4z`；合成训练生成器本来的映射就是正确顺序。这是标注录入错误，不是为模型预测修改牌意。原图放大对照及旧清单字节存于 `.local-test-data/runs/honor-code-review-e25da5f0dae84c8f9d037e2b5b3a23f3/`，清单修订记录保留原 SHA-256 和旧编码；原始模型输出、分数与日志未改动。

该目录 `summary.json` 对 16 份旧公共/手牌摘要独立复算，已接受错误数量均不变：修订的手牌未被接受，旧公共牌或拒绝或报南，后者修订后仍然错误。旧计数不变也不代表模型通过，只说明这次编码修订未改变相应接受统计。

四份真值添加从图像复核的 `observed_honor` 语义：包括旧四帧中清晰的東、南、西、北顺序，以及新图中西与北的直接对照。三个 Rust 清单测试纳入已有版本及回归命令，检查名称与牌码、缺失、重复、错位及四张上限；故意把西写为北时必须因对应关系失败。首次正向检查在修订前确实报告 `observed west requires 3z, got 4z`，日志为 `.local-test-data/honor-code-initial-failure.log`。这些测试只验证结构及语义录入一致性，图片观察本身仍需人工复核。

## 固定版本 ViT 教师的本机评估

为判断能否借助现成模型改善小分类器，评估 [pjura/mahjong_soul_vision](https://huggingface.co/pjura/mahjong_soul_vision) 的固定提交 `ea01ca0bcb9be0fc10a5fa09ee8356770c25adc5`。343,367,594 字节的权重 SHA-256 为 `a3cc566c8011b4496239a5228de3985e37ba2c2dfceefd52f67bb34c5f7e49c0`；模型卡声明 Apache-2.0，训练与评估图像来源没有具体说明。上游自报准确率不作为本项目证据。参数只在本机通过显式 ViT 架构和 `torch.load(weights_only=True)` 严格载入，没有执行远程代码、加载训练参数 pickle 或修改全局依赖；模型、模型卡、预处理配置与哈希保存在被忽略的 `.local-test-data/models/pjura-vit-teacher-ea01ca0/`。

实际源代码为干净提交 `ac923f8315745bf017e48a7767af75710fb645a0`，独立本机摘要为 `.local-test-data/runs/local-pinned-vit-teacher-930a7b1cebad4ca18b7f7c8af5382b61/summary.json`。同目录保存执行脚本、手牌定位日志、所有方向的概率与标签、真值/输入/模型哈希、命令和环境。输入为此前已经用于本项目开发的八组手牌、两组公共牌、一个宝牌指示牌及 19 个非牌面；没有新建独立验收集，也没有把教师输出用于训练。

使用原模型配置的 224×224 RGB、双线性缩放、均值和标准差均为 0.5；数字类别按配置的数值索引映射到标准牌码，并检查恰好覆盖 34 类。手牌使用当前原生槽位检测，旧牌河使用已复核的精确牌面边界，新牌河复用同提交探针的实际定位边界；公共牌保留外形允许的所有方向，不按真值挑方向。研究门槛在推理前固定为概率 0.8、前两名差 0.1、归一化熵不超过 0.4；通过门槛的方向若相互矛盾仍拒绝。该门槛未按此教师校准。

| 开发输入 | 总数 | 接受 | 34 类正确 | 37 类精确正确 | 拒绝 | 已接受的 34 类错误 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 八组手牌 | 111 | 111 | 111 | 109 | 0 | 0 |
| 旧牌河 | 18 | 16 | 16 | 15 | 2 | 0 |
| 新牌河与指示牌 | 11 | 11 | 11 | 11 | 0 | 0 |
| 已知非牌面 | 19 | 1 | 不适用 | 不适用 | 18 | 1 |

三个赤五都被当成对应普通五，因此 34 类成功不能视为 37 类成功。非牌面错误是空白浅色区域被报白板；此探针直接运行教师，没有套用应用现有的白板封闭边界与中心亮度检查，不能由此推断集成路径也会接受该区域。其他非牌面拒绝也只代表这 19 个已知开发负例。

本机使用 PyTorch 2.6.0+cu124、Transformers 4.51.3、RTX 4080，预热后的每候选耗时中位数约 5.17 ms、最大约 31.68 ms，包含图像处理、传输及 GPU 同步，不含模型载入、捕获、真实帧间隔或绘制。该结果不能代表普通 Windows CPU 或 250 ms 端到端目标。343 MB 教师也不符合当前小模型的 16 MiB 文件上限，未转换、集成或打包。它说明压缩或蒸馏值得继续验证，尚不能证明压缩后的效果；若后续使用其权重或教师输出，必须保留来源与许可记录，不能再声称该候选完全从零训练。真实默认资源、行动按钮、独立场景与现场验收仍未完成，任务状态不变。

## 结构压缩与字形比例对照

随后在干净提交 `fc7eeb6be9e7afe5a65a6d8d97ae3aedb9fbd8b9` 上做了两个固定最终步的教师派生实验。输入仍是 v2 的 `[1,3,48,32]`，输出改为标准 37 类；训练只使用已有 CC0/OFL/原创合成视图及原创负例。教师在 7,696 个合成视图上的基础类别最高分正确率约 77.4%，所以蒸馏仅采用基础类别正确且概率至少 0.7 的训练视图，3,167/7,104 个训练视图符合；所有训练视图仍有监督牌码。592 个同素材家族验证视图不参与训练，也不是独立实图。目标缓存 SHA-256 为 `74814e50a89cad2ad4a0969ee048c4d54fadab31ffc98966e80a06c3e9a0bd80`，教师权重来源和参数复用都记入每个候选清单，不将其标成 MIT 或完全从零训练。

| 候选 | 固定训练与结构 | ONNX 字节 | 原生旧手牌 | 原生旧牌河 | 新开发场景 |
| --- | --- | ---: | --- | --- | --- |
| `local-pruned-vit-v1-4b3cc61729f441a0a21b13b125d9d83e` | 2,500 步；4 层、192 通道 | 7,293,151 | 47/97 接受，无已接受错误 | 1/18 接受，无已接受错误 | 公共 3/10、手牌 4/14；不确定 |
| `local-pruned-vit-v2-427f105a6cb44c62a66530e62a1bbd26` | 6,000 步；保留 768 残差通道，每层 2 个注意力头、256 中间通道 | 13,270,751 | 64/97 接受，无已接受错误 | 7/18 接受，1 个错误（西报东） | 公共 6/10、手牌 8/14；不确定 |

参数分别为 1,815,205 / 3,309,605，ONNX SHA-256 分别为 `ff99d88b0332da73d2d0908f1cda2e470d14299edaa108aa48f6a82ead558584` / `d42f77a16d5e94fe4e530601e62c2410a9d76333ac7307c7e175e8c2e08522fa`。实际 Windows CPU 后端可加载，但两者均劣于此前候选的手牌完整性，没有安装或打包。两者的 19 个已知非牌面均拒绝；这种局部安全性不能抵消拒识和公共牌错误。阈值保持 0.8/0.1/0.4，未依据实图降低门槛或挑选中间训练权重。

原生摘要均在 `.local-test-data/runs/`：v1 为 `local-bound-classifier-506d00ff42d24c53af794fccb8e9f118`、`local-model-public-board-6263db003edc4ca5b11308a4836471ab`、`local-independent-scene-662312e0144441c9bb18ea74ea2c108f`；v2 为 `local-bound-classifier-0bd6fd6115a546b88a41dd676646da2f`、`local-model-public-board-279e362b1f9f401a8d7a09481b674a58`、`local-independent-scene-3bc295c66a8642aca5a56b5554490dc2`。两轮完整静态场景探针均退出 1，没有 Ready/Top-3。最初复用的场景脚本因不属于原先冻结的两个模型而拒绝运行，保存在 `local-independent-scene-6d150890830d40c49b7206cd7a64a862`；后续明确按开发集运行新模型，仍校验实际权重和图像哈希，没有冒充原始冻结基准。

为区分预处理损失和压缩损失，同一个未压缩教师对同 111 张手牌逐一比较：原始牌面基础类别最高分为 111/111，v2 紧字形拉伸为 104/111，7 个错误都是二索；原记录为 `local-teacher-glyph-input-186f39e913e54f9598009c4410a05fa0/summary.json`。预先固定的保持比例、28×44 内居中白底原型在 `local-teacher-letterbox-fc022e86d45047d8b7e22d07260a5a78/summary.json` 恢复为 111/111。这是已知开发图的基础类别最高分诊断，没有白板/置信度拒识，也不包含赤五区分；不能作为新小模型或现场准确率。

因此原生接口新增独立的 `rgb-inverse-glyph-letterbox-v3`，按清单选择，保留原 v2 模式。14 个原创几何/颜色 fixture 逐像素对照独立 Pillow BOX 结果，包括窄/宽字形、两种恰半舍入、奇数留白、极端尺寸、红字和角标；原 v2 的 7 个 fixture 字节未变。新增清单及实际系统后端测试证明选用了不同输入，并保留白板拒识。最初两个编译失败日志为 `.local-test-data/letterbox-initial-failure.log` 和 `letterbox-routing-initial-failure.log`；Clippy 的测试范围写法失败及修复后日志也分别保留。该实现只是下一轮权重实验的输入契约，尚无已验收的 v3 权重，不能直接改旧权重 ID 或把两个失败候选作为默认资源。

## 公开牌图种子与小型 CNN

干净代码提交 `3197b31b01b91601c8fda430260406f5b92106b5` 的版本 8 项、回归 9 项以及 [CI 40559222750](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40559222750) 五项通过。实际 v3 输入在 112 张已有游戏/FAQ/素材图上与独立 Pillow 参考逐值一致；该数值检查不是识牌验收。

后续 v3 教师压缩实验使用重新生成的保持比例数据、完整 768 维 CLS 目标及 4×2 小块，固定 6,000 步得到 `local-pruned-vit-v3-1eececd693b7412d87fff82b2d7c97dc`，13,491,936 字节，SHA-256 `468c506ffb835e6a344238049ba796cf256555c3b8567a7aedb070340e80aaaa`。实际后端手牌为 67 张正确接受、1 张误认、29 张拒绝；深色非牌面被误作八筒。旧牌河接受 7/18、4 个错误，新开发图公共牌也有误认；相关失败摘要为 `local-bound-classifier-41be49a92c004571990ce911859c7d39`、`local-model-public-board-b915e4ba158f414aafb866805027ce0e`、`local-independent-scene-3bdbeba3ed8d4404b478d1f404326470`。该候选未采用。合成数据审计为 `letterbox-data-audit-69b3f925fe4a49cc90b2eac0ebca37f8`：没有矛盾牌码，验证/训练的精确重复只有 16 张预期零张量白板，非白板没有精确重复；仍是同素材家族诊断。

为改善实际牌面差异，改用 [pjura 公开数据集](https://huggingface.co/datasets/pjura/mahjong_souls_tiles/blob/8c0f22e7c6b64be55bb1d2767fe1a63981788de7/README.md)的 34 张原始图做本机训练，固定提交 `8c0f22e7c6b64be55bb1d2767fe1a63981788de7`。数据集卡声明 Apache-2.0，并说明来自雀魂。API 列出 1,979 个文件，但原始牌图在 train/test 重复，其余主要是增强，不能宣称有近两千个独立观察。34 张图的 LFS SHA-256 均校验，共 615,786 字节；原始图、模型卡和下载脚本在被忽略的 `.local-test-data/models/pjura-seeds-8c0f22e/`。所有原图继续只在本机。

清理原始图的角上说明文字后，从普通五字形合成 3 个赤五种子。白板的 Wh 和红中的黑色 R 另有已复核遮罩，不改变生产预处理规则。每类生成 160 个随机变换视图，144 训练、16 仅作同种子变换诊断；赤五种子不是真实赤五截图。首轮仅用正向公共牌，第二轮覆盖四个方向并变化红色墨迹；检查全部 37 类的训练/验证均覆盖所需方向，避免方向与验证索引绑定。第二轮输入 SHA-256 为 `123b419bb3e8a7dbed78b505bdc69aee5d54caddd407882db7e8d27d8a2c14d0`。

两个 CNN 都从零训练，不使用教师参数或本项目公开手牌截图像素。固定 8,000 步，85% 公开种子视图、15% 既有 CC0/OFL 正向合成图，另有原创负例；阈值保持 0.8/0.1/0.4、温度 1.0，未根据实图降低门槛。模型均为 240,309 参数、962,873 字节，v3 输入和标准 37 类。

| 候选 | 已知 97 张手牌 | 旧牌河 | 旧新来源开发图 |
| --- | --- | --- | --- |
| `local-public-seed-cnn-v1-3d7ae953953d495aa20fb6863ca56c76` | 96 正确接受、1 拒识 | 16/18 接受、无已接受错误 | 公共 6/10、手牌 12/14，整帧不确定 |
| `local-public-seed-cnn-v2-9bde2d5a4439478f944ba338faa8c25a` | 97 正确接受、无拒识 | 17/18 接受、无已接受错误 | 公共 6/10、手牌 12/14，整帧不确定 |

二者均拒绝 19 个已知非牌面。模型 SHA-256 分别为 `6c409a554c16ef9102588d4a0332cffdb495a12a6e0ec112608b28690e1e0e06` 和 `ac422eb2358edfdb96e6c00f2dcec4d5098cf8d0bf369067f9b389dc822a2bee`。v2 本机摘要为 `local-bound-classifier-14f7e022cc9e4076a23d85c2718ec5cd`、`local-model-public-board-4c9d32b8754a4cb1aa15d27fa75c0c02`、`local-independent-scene-bbceb60dc8d04e8ebcaa00cad47a6be4`。公共牌未全部接受时仍没有完整 Ready/Top-3，不能把部分接受率当作准确率达标。

`local-cnn-native-parity-ef70c61d67994aaebee84f2f7f946eff` 对 112 张已有图比较 PyTorch 与实际系统后端，输入误差为 0，最大 logits 误差约 3.85e-6，最高类别一致。`local-model-runtime-44bd18b587f441988160732244f8f5c3` 的七组已有手牌都在第三帧 Ready：13 张时等待摸牌，14 张时给出手牌中的合法切牌；未知整手保持不确定。离线单帧处理最高约 12.4 ms，不含捕获、真实帧间隔或绘制，也不包含公共牌/按钮，不能作为 250 ms 验收。

## 冻结后的两来源手牌检查

v2 的权重、清单、阈值和已有报告哈希先写入本机 `frozen-evaluation-manifest.json`，随后选择[来源一](https://note.com/nagomi_258/n/n4677898917bd)和[来源二](https://note.com/amutan_mj/n/n8d3f8e0286e9)各自文章的前两张正文图片，未按模型结果挑图。原始 PNG/JPEG、文章快照、裁剪和标注在 `.local-test-data/screenshots/frozen-source-review-eac2ec53bcfc480f96d5498ac6db871e/`；仓库仅保存结构化清单 `tests/fixtures/manifests/public-hands-v3.json`。下载器最初只接受 PNG 而拒绝第二篇的 JPEG，原脚本、两个 PNG 和失败说明均保留；恢复时校验真实格式并保存原始字节，没有重新编码或改选图片。

四个手牌共 55 张，三个有已记录的回放标记遮挡，只有一个手牌没有已知字形遮挡；同文章两个帧不算额外独立对局。先看原图和放大裁剪记录真值，再运行冻结模型。首次摘要 `local-frozen-public-hands-v3-36ae29e669b44c14a9eb60d0d30f74c1` 为 39 正确接受、1 个表面标签错误、15 拒识，1/4 手牌 Ready，整组退出 1。该次工作树只新增真值清单，标记 `source_dirty: true`，不作任务成功证据。

随后放大复核发现来源二第二图第 12 张是“三个深色筒斜排＋四个红筒方阵”的七筒，初次误录为九筒；旁边第 11 张也是七筒，之后两张才是九筒。初始真值 SHA-256 `3104b60ee5ded7dd0f2358ee2d1d679a056d8888ccb8504fbe14795ecc14527b` 的字节仍在 `initial-hand-truth.json`，原生预测和初始摘要均未改写。独立复算 `frozen-hand-pip-reassessment-a5dcf691018a4fcfb643cfe7f41e9d6b` 为 40 正确接受、0 已接受错误、15 拒识，Ready 仍为 1/4，不是另一次独立验证或完整通过。标注修订不能隐藏这 15 张拒识。

牌槽诊断还显示，部分回放标记被纳入候选上沿，而普通牌上沿更低；这给后续边界与遮挡处理提供了线索，但当前没有借此修改冻结模型或阈值。如果用这些图调整实现，须将其转为开发数据并保留这次初始报告。所有候选仍未默认安装或打包，五种按钮资源、公共牌完整性、新来源稳定性和真实对局验收继续未完成。
