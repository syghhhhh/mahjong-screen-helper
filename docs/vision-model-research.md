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
