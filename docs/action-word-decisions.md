# 行动字形的候选与拒识契约

`vision-core::action_decision` 把七类原始 logits 转为字形候选或带原因的拒识。它不证明按钮位置、底板或当前启用状态，也不直接转换成 `ActionLabel`。浮窗仍使用现有模板流程，没有启用实验行动权重。

固定顺序为 `Chi, Pon, Kan, Riichi, Pass, Unsupported, Unknown`。只有前五类能成为 `ActionWordDecision::Candidate`，Pass 是明确的跳过字样，不能用缺失观察代替。Unknown、Unsupported 即使分数接近 1 也只返回拒识；无字形或未定位不能证明“当前没有按钮”。

输入先检查形状、有限的 `[0,1]` 像素、非空字形、可表示的矩形及前景计数，实心亮矩形拒绝作为字形。float64 稳定 softmax 拒绝非有限 logits。诊断默认保持分数 0.9、前两类差 0.25、归一化熵上限 0.35；其他显式配置须通过范围验证，不会静默修正。分数不是现场准确率。

六个测试覆盖五类候选/Pass、Unknown/Unsupported、无字形/实心亮块、非法张量/矩形、非有限/模糊结果、非法配置及极端数值/整体偏移不变性，进入现有视觉版本和完整回归入口。

本机诊断新增 `word_decision`，保留原输入文件和 `logits`：

```powershell
cargo run -p vision-core --example inspect_action_words --release --locked -j 1 -- --complete-components <已核对的actions.onnx> <新建inputs.f32le> <button.png> ...
```

该工具不读取安装清单，也不把候选交给浮窗或状态机；开发者须先核对权重与输入契约。模型安装、底板/启用状态、位置和连续帧仍需独立验证。

## 描边与局部形变实验

原十字体数据只渲染实心字，而开发用跳过按钮有明显描边样式。v4 保留原十份 OFL 字体、结构、种子、8,000 步、最终检查点、预处理和门槛，为所有支持词和未知词增加 1–4 像素空心描边，原实心 0–4 样式保留。v5 使用同一批种子张量，以独立随机源 20261029 对一半训练输入增加 4×8 控制网格、最大 2.5 像素的平滑局部形变；主随机源、初始化和优化配置不变。训练没有游戏像素或预训练权重。

两个模型均为 1,616,311 字节，只存本机：

| 候选运行目录 | 权重 SHA-256 |
| --- | --- |
| `font-action-cnn-v4-outline-28e51bf963c44c1f84b7f18fd787dc10` | `4ba0bb1bdf366e5089f3fe82565a851354692c25f08c99510ed0916ed6818bfb` |
| `font-action-cnn-v5-elastic-0510a13e2cda480d908d7714bacd515f` | `7b283df3a08aebcb555a2a3a18ab71cc5a658f1727d2a3b15c3b5b525639e474` |

已另行逐值核对 v4/v5 的种子张量和标签相同，原始张量/标签哈希及形状在下述复测汇总目录的 `seed-data-parity.json`；不依赖含 ZIP 时间戳的 NPZ 文件哈希来声称内容相同。

两轮采用同一批 21 项已知开发数据，原始概率统计保留。实际 Rust 决策另区分七个支持词正例（主要五类加旧碰/跳过）和十四个非操作词/背景；拒绝未知词是正确结果，不应因为 Unknown 的低分拒识而判作漏按钮。

| 候选 | 支持词正确候选 / 7 | 主要按钮 / 5 | 非操作输入错误候选 / 14 |
| --- | ---: | ---: | ---: |
| v3 基线 | 1 | 1 | 0 |
| v4 描边 | 2 | 2 | 0 |
| v5 局部形变 | 2 | 2 | 1 |

v4 的吃约 0.990、碰约 0.930；杠仍最高为 Unknown，立直和跳过低分拒绝。旧跳过被高分判为 Unknown，是支持词漏识，不是错误的可操作词候选。v5 主要五类的最高类别虽均正确，仍只有吃、碰达到门槛；未知“跳舞”却成为约 0.912 的 Pass 候选，因而明确被否决。两轮均未满足完整支持词要求，没有降低门槛、安装或分发权重。

原新模型探针为 `native-action-outline-v4-parity-bbc7380681cf42b19f1e749c31c10c96`、`native-action-elastic-v5-parity-eca6e05b4f264e70abb6b41dd3fc321c`。加入类型化决策后的冻结模型复测汇总在 `.local-test-data/runs/action-word-decision-comparison-34045d4d3ce948638cb4af64722110a9/summary.json`，各原生运行另存 `word-decision-summary.json`。输入逐值一致，最大原生/PyTorch logits 误差分别约 2.74e-6、1.43e-6、1.91e-6；数值契约不等于分类或现场验收通过。

另核对固定 Google Fonts 提交 `ddf386ba1b9fbc6b2c818f1ddcd9148f54ced94b` 的 [Long Cang](https://github.com/google/fonts/blob/ddf386ba1b9fbc6b2c818f1ddcd9148f54ced94b/ofl/longcang/METADATA.pb)、[Zhi Mang Xing](https://github.com/google/fonts/blob/ddf386ba1b9fbc6b2c818f1ddcd9148f54ced94b/ofl/zhimangxing/METADATA.pb)、[Liu Jian Mao Cao](https://github.com/google/fonts/blob/ddf386ba1b9fbc6b2c818f1ddcd9148f54ced94b/ofl/liujianmaocao/METADATA.pb) 元数据、OFL 和 Git blob。三者都缺槓、過，未加入这两轮训练，没有把缺字方框当成字形正例。字体未安装或打包。
