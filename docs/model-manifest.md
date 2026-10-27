# 本机小分类器清单与拒识接口

Windows 浮窗已支持显式安装的本机模型回退，运行时只读取本机文件，不联网，不需要 Python。当前没有随包训练权重，实验权重也未自动安装，默认视觉资源与真实对局验收仍未完成。

## 安装与启动

在可执行文件旁放置 `models/tile-classifier/classifier.json` 和其引用的 ONNX 文件。点击“开始”时检查清单、哈希、类别、预处理和系统后端；模型加载失败会显示原因并停止本次启动，不会悄悄忽略已安装的坏模型。没有清单时沿用完整模板模式。

`models/template-pack/` 中牌图和按钮齐全时保留模板快速路径，仅对不确定的牌槽使用模型。牌图不完整而模型有效时，忽略不完整的牌图子集，由模型负责全部手牌和公共牌分类；五种按钮 PNG 仍须齐备。浮窗会显示“模板模式”“模板＋模型回退”或“模型识牌”。

原生 session 和模型文件锁由实时引擎持有。安装、移除或更换模型需要停止并重新开始识别，不能在旧 session 存活时覆盖被锁定的权重。调用 `RuntimeEngine::set_tile_classifier` 会重置稳定帧、旧推荐、来牌和换局视觉基线，保留当前局的规则信息；不得比较新旧模型的标签来自动推断换局。

## 清单

JSON 最大 16 KiB；未知、重复、缺失字段均拒绝。示例见 `tests/fixtures/synthetic/neural/classifier.json`，其中权重是项目原创的线性算术图，不是可用的识牌网络。

必需字段：

| 字段 | 约束 |
| --- | --- |
| `schema_version` | `1.0` |
| `model_id` | 1–64 个 ASCII 字母、数字、连字符、下划线或点，以字母或数字开头，不含连续点 |
| `model_file` | 清单同目录下的 `.onnx` 文件名；上述字符限制，不接受目录、流名和 Windows 保留设备名 |
| `model_sha256` | 64 位小写十六进制，必须与实际文件一致 |
| `preprocessing` | `rgb-inverse-glyph-box-v2` |
| `class_order` | 严格为 `1m…9m, 1p…9p, 1s…9s, 1z…7z, 0m, 0p, 0s` |
| `license` | 非空的可显示许可说明，最大 120 字节；字段本身不证明素材授权 |
| `thresholds` | 必须显式指定下面三个有限数值 |

`thresholds.min_probability` 范围为 `[0.5,1]`，`min_margin` 和 `max_normalized_entropy` 范围为 `(0,1]`。示例中的 0.8 / 0.1 / 0.4 仅为契约测试参数，不是经过现场校准的生产阈值。清单只绑定所声明的类别顺序，不能证明外部网络确实按该顺序训练；必须另外审计训练与导出来源。

`LocalTileClassifier::load` 使用 Windows 系统 CNG SHA-256 校验实际模型，限制模型 16 字节至 16 MiB。文件在哈希、加载和 session 存活期间保持只读共享，防止校验后被写入或替换；原生 session 在文件锁前释放。后端继续核验一个 float32 `[1,3,48,32]` 输入、一个 `[1,37]` 输出及有限值。[CNG 哈希接口](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcrypthash)与[pseudo-handle 文档](https://learn.microsoft.com/en-us/windows/win32/seccng/cng-algorithm-pseudo-handles)说明这条系统路径从 Windows 10 提供。

## 决定与白板上下文

`ModelManifest::decide` 用 float64 稳定 softmax 计算最高分、前两名差和归一化熵。任意非有限 logits、低分、差值不足或熵过高均返回带原因的 `Rejected`。分数不是实测准确率；不能单凭高分宣布未知输入已解决。

`FaceObservation::isolated` 只准备单张牌面；无墨色时即使白板分数极高也拒绝。`FaceObservation::in_hand` 同时从当前完整手牌 ROI 获取字形和白板证据：候选须来自现有牌槽检测、符合直立牌宽高比例，并在当前 ROI 内的上、下、左、右每边均有足够的暗边对比。无墨色且满足这些条件时，才允许接受白板；有字形却预测白板也拒绝。

字形与边界证据封装在同一个观察对象中，不接受调用者传入独立布尔值或上一帧证据。校准 ROI 应留出至少 3 像素外围；边界被裁掉、缺失或未定位时保守拒绝。该几何约束还不是所有皮肤、遮挡和动画场景的白板验收。

公共牌用 `FaceObservation::in_public` 从当前区域取得同样的白板证据，按指定方向旋转牌面。模型回退检查四种方向；重复接受同一类别可保留最高分方向，接受了不同类别则拒绝该牌。任一方向的后端执行失败也拒绝；不能只挑最高分掩盖另一方向的矛盾解释。公共牌重叠、单牌超过四张、未知区域及唯一高亮来牌规则继续生效。

实时引擎中的手牌、牌河、四家副露及指示牌均可回退；动作按钮仍由模板识别。任一必需牌面失败就拒绝本帧，不把剩余牌拼成完整手牌，也不复用旧建议。模型结果仍经过连续三帧确认和原有牌数/局面合法性检查。

## 本机诊断

```powershell
cargo run -p vision-core --example inspect_classifier --release --locked -j 1 -- <清单JSON> --faces <牌面PNG> ...
cargo run -p vision-core --example inspect_classifier --release --locked -j 1 -- <清单JSON> --hand <本机RGB原始字节> <宽> <高>
```

前者接收 1–256 张 RGB/RGBA PNG，后者自动定位 1–14 个手牌候选。输出 accepted/rejected、原因、分数及分阶段时间；不输出麻将推荐，不启用浮窗，也不保存截图。输出日志须重定向到 `.local-test-data/`。RGB 文件必须与声明尺寸精确相符，尺寸上限 16384×1024。

契约测试覆盖清单变异、数值稳定性、赤五顺序、模糊/非有限结果、缺边空白与完整牌槽；Windows 测试覆盖独立 SHA-256 参考、真实原生加载、坏哈希、错误维度及锁的释放。它们随版本和全项目回归脚本运行。

实时链路另可离线诊断：

```powershell
cargo run -p runtime-core --example inspect_model_pipeline --release --locked -j 1 -- <清单JSON> <手牌RGB> <宽> <高> [<牌河PNG> <己方副露PNG> <左家PNG> <对家PNG> <右家PNG> <指示牌PNG>]
```

命令运行三帧，输出等待、拒识或局面与 Top-3。可省略全部六张公共牌图片，但那样只验证手牌链路；按钮输入始终缺席。它不捕获窗口、不验证浮窗、不模拟真实时序，处理耗时不能代替 250 ms 端到端验收。
