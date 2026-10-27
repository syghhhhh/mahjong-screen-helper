# 行动按钮识别：输入与原生后端

此功能仍在 V001-003 内开发。当前浮窗继续使用完整的五种本机按钮模板；以下是下一步默认资源的输入、数值后端及研究证据，尚未替代模板、接入实时链路或随包分发权重。

## 输入与拒识边界

`vision_core::action_input::prepare_action_word` 在内存中处理 16–1024 像素宽高的 RGB 图，不读写文件或修改原图。输入标识为 `action-bright-word-mask-v1`。

- 提取亮字候选：最大通道至少 180、最小通道至少 60、三通道和至少 420。
- 以四连通域寻找足够大的主体：面积至少为输入的 0.3%，高度至少为输入的 1/8，宽度至少为输入的 1/32。主体联合边界内的小笔画保留，外围小装饰不扩大边界。
- 将字形保持比例放入 120×56，再居中到 128×64。尺寸采用最近偶数舍入；采样复现训练端 Pillow NEAREST 的逐步 float64 坐标累加。输出为单通道 float32，亮字为 1，背景为 0。
- 没有亮点返回 `Empty`；有亮点但无法定位主体返回 `Unlocalized`。调用方不能把后者当成没有行动按钮。即使为 `Empty` 或已准备好字形，也不能据此证明按钮不存在或已启用，仍需按钮底板、位置、状态和置信度证据。

15 个原创几何 fixture 与独立 Pillow 输出逐像素比较，覆盖尺寸、孔洞、分离笔画、噪点、颜色边界、两种恰半舍入，以及最近邻落在源像素边界的情况。最初整数采样公式在 21 个开发输入上产生 254 个像素差异；新增条纹 fixture 复现失败后，按 [Pillow 12.2.0 的采样语义](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Geometry.c)修正为逐步累加。初始差异、失败日志和修复后的独立对照均保留，没有用放宽误差门槛代替修复。

## 两个独立的原生 tensor 契约

Windows CPU 后端复用内部会话管理，但只向外提供两种固定类型：

| 接口 | float32 输入 | float32 输出 |
| --- | --- | --- |
| `WinMlTileSession` | `[1,3,48,32]` | `[1,37]` |
| `WinMlActionSession` | `[1,1,64,128]` | `[1,7]` |

两种模型不能互换。后端仍检查文件范围、输入/输出类型及形状、输入范围、有限值，并保持创建线程的 COM apartment 生命周期。七类后端只返回原始 logits；不验证模型来源、类别语义、分数阈值、按钮底板或局面，不自动加载或改变应用模式。

四个原创算术 ONNX fixture 和四个原生测试覆盖独立均值仿射参考、维度混用拒绝、非法输入、非有限输出，以及牌面/行动会话交错使用和 STA 保持。字形输入另有五个测试。这些测试由现有版本与回归视觉脚本自动运行；合成数据和原生数值成功不代表真实按钮识别成功。

开发诊断命令：

```powershell
cargo run -p vision-core --example inspect_action_words --release --locked -j 1 -- <行动ONNX> <新建tensor.f32le> <本机按钮PNG> ...
cargo run -p vision-core --example inspect_action_logits --release --locked -j 1 -- <行动ONNX> <tensor.f32le>
```

第一条限制 1–256 张本机图片，输出输入类别、边界、tensor 和 logits；目标 tensor 文件须不存在。第二条只读取有界的已准备输入。两条均不读取模型清单、不执行正式置信度或按钮上下文验收，也不启用浮窗识别。终端用户仍不需要 Python、Node 或训练环境。

## 字体方案的已知结果

本机固定 [majsoul_wrapper](https://github.com/747929791/majsoul_wrapper/tree/d1667d527c3f8c44b37654deec8de46dad9d7f1f/action/template) 到提交 `d1667d527c3f8c44b37654deec8de46dad9d7f1f`，保存吃、碰、杠、立直、跳过、和、自摸及菜单参考图，均按 Git blob 校验。该仓库声明 MIT，但这不单独证明游戏美术的分发授权；参考图只在本机，没有复制其代码或把图纳入仓库、训练输入或便携包。旧碰/跳过裁剪也只是开发参考。

直接用开放字体字形距离匹配，吃、立直与其他文字会混淆，未采用。随后使用已有 Noto Sans/Serif CJK、Ma Shan Zheng、LXGW WenKai 的 OFL 字体合成字形，从零训练七类候选：Chi、Pon、Kan、Riichi、Pass、Unsupported、Unknown。包含和、自摸和其他文字负例；不使用游戏截图像素、预训练权重或实图分数校准。固定 8,000 步及最后一步权重，阈值预设为概率 0.9、前两类差 0.25、归一化熵不高于 0.35。

候选 `font-action-cnn-v1-a14c9df66f20479ea5c68ac499cb28b6` 为 1,616,311 字节；输入清单仍标为研究原型，不能直接当作可安装资源。原生对照 `native-action-input-parity-49a937cc5f1845a8bd6d0fa5685159b1` 的 21 个开发输入，Rust 与独立 Python/Pillow 输入逐值相同，Windows CPU 与 PyTorch 最大 logits 误差约 1.19e-6，最高类别一致。

分类效果仍不足：四种请求动作里，碰和立直满足预设门槛；吃约 0.871 而拒绝，杠最高为 Unknown 且未过门槛。跳过被高分报为 Unknown，是分类失败，不能隐去；它没有产生一个错误的吃碰杠立直建议，但也不证明未来实时链路会正确处理。21 项共 9 项按预设类别正确接受、1 项类别错误、11 项拒绝，完整探针退出 1。旧裁剪、未知文字和不同画面仍需继续验证，不能把训练种子满分或部分动作成功当成默认资源验收通过。

所有训练脚本、权重、来源、失败候选和原始画面保存在被忽略的 `.local-test-data/`。下一步仍需可靠权重、模型清单与哈希锁、按钮定位/底板证据、未知按钮拒识、多按钮组合、实际状态机集成和新来源/现场验收。任务状态保持 `in_progress`。

## 完整笔画输入 v2：修复裁剪，保留 v1 兼容性

检查开发参考图发现，v1 不仅排除外部装饰，也会截断跨越主体边界的短笔画。例如“槓”的短连通域范围为 `[130,64,147,74)`，与主体边界 `[80,4,149,71)` 相交，但高度不足以成为主体，所以底部三行被裁掉。“跳過”也存在顶端、右端笔画被部分截断的情况。这是几何输入问题，不能靠降低模型门槛解决。

新增 `prepare_complete_action_word`，标识 `action-complete-word-mask-v2`：从原主体矩形出发，只要矩形内实际包含某个四连通域的前景像素，就纳入该连通域的完整范围；继续处理新增矩形范围，直到不再扩大。仅外接框重叠而实际像素不相交，不应被纳入。实现对新增的互不重叠矩形条带排队，每个像素、连通域最多处理一次，时间与输入像素和连通域数成正比，输入尺寸限制仍为 16–1024。

旧函数与 `action-bright-word-mask-v1` 的输出保留，旧模型不自动改用 v2。新接口仍是研究接口，不证明按钮底板、位置或启用状态，也没有接入浮窗。诊断命令只有显式增加 `--complete-components` 才使用 v2：

```powershell
cargo run -p vision-core --example inspect_action_words --release --locked -j 1 -- --complete-components <行动ONNX> <新建tensor.f32le> <本机按钮PNG> ...
```

六个新增测试覆盖连续扩展、四个方向、旋转、外部装饰、只有外接框重叠、兼容性和最大尺寸碎片输入；三组原创合成图的预期边界由手工指定，输出逐像素对照独立 Pillow 参考。原 v1 的全部测试与 fixture 不变，新测试由版本与回归视觉入口运行。

本机原生对照 `native-action-complete-component-parity-b5fb922cb3674b87bc46f8e8859898b3` 在 21 项已检查开发输入上与 SciPy/Pillow 输入逐值相同，Windows CPU 与 PyTorch 最大 logits 差约 1.19e-6。该探针因分类仍有拒识而退出 1；数值一致不能当作识别通过。Python 几何对比 `action-component-closure-afd15d273f94465c84f433273cb17065` 中，“槓”补全后 OCR 文字从“植”变成“槓”，但概率仅约 0.50，继续拒绝，五种按钮仍未全部达标。

## 后续资源实验及淘汰结果

字体覆盖审计发现 Ma Shan Zheng 缺少“槓”“過”。从 [Google Fonts 固定提交](https://github.com/google/fonts/tree/ddf386ba1b9fbc6b2c818f1ddcd9148f54ced94b) 获取并校验 Yuji Boku、Yuji Syuku、Yuji Mai 的 OFL 字体；它们覆盖两字，但都不覆盖“碰”，故没有替代原四字体。只增加三种字体，其余结构、种子、8,000 步和阈值不变，得到候选 `font-action-cnn-v2-c40ef1feda394dddad1631f0f7585270`，1,616,311 字节，SHA-256 `9b460246c66a6171f4558a31ac5b97aab2e4f63685c3d0f15078fbe1c10d3a23`。

在旧 v1 输入的同一批 21 项开发数据上，该候选 9 项正确接受、0 项错误接受、12 项拒绝；吃提高到约 0.924，立直却降至约 0.758，杠和跳过继续失败。新 v2 几何输入上为 10 项正确接受、0 项错误接受、11 项拒绝，仍只有吃、碰满足五种主要按钮中的门槛。这里“正确接受”包含 Unsupported、Unknown；不能误读为识别了 9 或 10 个可操作按钮。训练种子与这些反复使用的开发图都不是独立验收集，候选继续留在本机。

另测试 [RapidOCR 固定提交的模型登记表](https://github.com/RapidAI/RapidOCR/blob/80b427d69bfc5b91a12913394875910296c6f315/python/rapidocr/default_models.yaml)。每份 ONNX 按登记 SHA-256 校验，使用本机 ONNX Runtime CPU、2 线程，固定四种输入方案：整图 BGR、字形裁剪 BGR、裁剪反色、黑字白底二值图。CTC 仅接受完整精确词，保留字符的最低概率须达到 0.9；没有子串匹配或把错字映射为动作。

下表分母是这 21 项中的 7 个已知主要动作输入：五种按钮加两个旧裁剪。其余输入用于检查错误动作接受。四种方案的顺序与上文一致；同一输入反复比较，不能作为独立准确率。

| 模型 | 文件字节数 | 四种方案正确动作接受数 / 7 | 21 项中的错误动作接受数 |
| --- | ---: | --- | --- |
| PP-OCRv4 mobile | 10,857,958 | 1、2、3、2 | 各为 0 |
| 繁体 PP-OCRv3 | 11,152,536 | 0、0、0、0 | 各为 0 |
| PP-OCRv6 tiny | 4,489,813 | 1、2、3、2 | 各为 0 |
| PP-OCRv6 small | 21,234,383 | 4、5、5、3 | 各为 0 |

v4 mobile 和 v6 tiny 的词典没有“槓”；繁体 v3 虽有该字，仍未满足按钮置信度。v6 small 字典覆盖目标字，其裁剪方案能接受吃、碰、立直、跳过和旧跳过，仍拒绝杠与旧碰。完整笔画输入改善了“槓”的转写，接受数仍为 5/7。普通印刷体合成正例能得到多项正确高分转写，说明探针不是完全不能工作，但游戏字形结果不满足要求。

| 模型 | SHA-256 | 本机开发运行编号 |
| --- | --- | --- |
| v4 mobile | `48fc40f24f6d2a207a2b1091d3437eb3cc3eb6b676dc3ef9c37384005483683b` | `rapidocr-action-development-adbc141efbf54ab195e596bcc9a7c308` |
| 繁体 v3 | `779656d044ce388045e02ea9244724616194e63928606436cdfc6dc3c9528cc6` | `rapidocr-action-development-19f85fa3be3e450992a308f389e2356d` |
| v6 tiny | `e16e242de5937ad92609223f19bc2aff3727ee40b095f996907c24749bad251b` | `rapidocr-action-development-ae93535792e145a7844188e8d644d083` |
| v6 small | `6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884` | `rapidocr-action-development-3d98c06781c945ea8d1d6d5a43c2eed7` |

上游 README 声明代码及转换模型使用 Apache-2.0，并说明 PaddleOCR 来源；其链接的 `MODEL_LICENSES.md` 在该固定提交返回 404，详细来源归属审查尚未完成。实验没有把上游脚本作为项目运行代码，也没有分发模型。OCR 的三维时序输出不符合现有 `[1,7]` 原生动作后端，不能直接替换模型文件。结果和授权两方面都未完成，因此没有新增 OCR 运行依赖或把任何权重装入便携包。
