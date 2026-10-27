# 第三方依赖与许可证

本项目自身代码和文档采用 [MIT 许可证](../LICENSE)，版权声明为 `Copyright (c) 2026 syghhhhh`。便携包根目录附有项目 `LICENSE`。该授权不覆盖第三方游戏牌图、截图、模型或其他不属于本项目的素材。

当前直接 Rust 依赖：

| 依赖 | 用途 | 上游许可证 |
| --- | --- | --- |
| `serde` / `serde_json` | 校准配置序列化 | MIT OR Apache-2.0 |
| `windows-capture` | Windows Graphics Capture 采集 | MIT |
| `png` | 读取本机 PNG 识牌模板 | MIT OR Apache-2.0 |
| `windows` | Windows 系统 ONNX 接口的 Rust 绑定 | MIT OR Apache-2.0 |

间接依赖的精确版本由 `Cargo.lock` 固定。便携包构建时运行 `scripts/generate-license-bundle.ps1`，把 Cargo 元数据中的锁定注册表依赖、许可证字段、许可证原文及文件 SHA-256 写入 `licenses/DEPENDENCIES.json` 和 `licenses/texts/`。该清单包含 Cargo 返回的所有锁定注册表依赖，可能多于 Windows 实际使用的依赖。发布前仍须核对随包分发的模型、牌图、字体和其他资产；目前没有引入第三方模型或牌图。参考项目只用于架构调研，未复制其代码。

## 视觉实验数据的边界

本机诊断曾使用 [pjura 的雀魂牌图数据集](https://huggingface.co/datasets/pjura/mahjong_souls_tiles/raw/main/README.md)。数据集卡标注 Apache-2.0，同时明确图片取自雀魂游戏。该标注没有说明数据集作者拥有游戏原始美术的再分发授权；因此实验牌图仅放在被 Git 忽略的 `.local-test-data/`，不得随本仓库或便携包分发。另一开源识别项目 [QueHun](https://raw.githubusercontent.com/jzjzzzzzzz/quehun/main/templates/actions/README.md) 的按钮模板也标明来自真实客户端，当前不复制进本项目。正式发布前须取得可分发素材的明确授权，或改用本机由用户提供的模板而不打包游戏素材。

另测试了 [FluffyStuff 的公有领域日麻牌图](https://github.com/FluffyStuff/riichi-mahjong-tiles)，上游提交 `26e127ba2117f45cdce5ea0225748cc0cfad3169`。直接将其 37 张 PNG 作为模板，对第一张公开雀魂手牌截图的 14 个槽位进行候选匹配时，整手标签判定失败，默认置信度门槛也未通过。失败摘要保存在本机 `.local-test-data/runs/local-vision-tiles-f765675d230d4766a6cda12a59dcd3dd/`。这套素材具有可分发的授权，但不能未经适配就声称支持默认雀魂牌面，因此当前未打包。

另在本机评估 [mahjong-utils-app](https://github.com/ssttkkl/mahjong-utils-app) 提交 `637410ae10204fc69e4c6052a29e0e40881adac8` 的 `best.fp16.onnx`，文件 5,348,843 字节，SHA-256 为 `70caa1c6b6809345695cf3a1794a97772edb055356afccdf8e441f14e025797e`。其应用许可证为 MIT，但权重内嵌元数据及[同目录模型元数据](https://github.com/ssttkkl/mahjong-utils-app/blob/637410ae10204fc69e4c6052a29e0e40881adac8/mahjong-detector/src/wasmJsMain/resources/best_web_model/metadata.yaml)声明 AGPL-3.0。[Ultralytics 的许可说明](https://www.ultralytics.com/license)要求采用其开源许可的集成项目满足相应整体开源要求。当前项目用户已选择 MIT，因此该权重仅保存在被忽略的实验目录，没有加入仓库、依赖或便携包。

该模型只有 34 类，不区分赤五。按上游灰度、对比度拉伸、黑边填充和 FP16 输入处理后，三张公开手牌样本的 34 个基础类别匹配为 41/41；复核赤五真值后，37 类精确匹配为 40/41；单线程 CPU 单次推理约 76–84 ms，牌河仍漏检或误认。第一次使用通用 RGB 预处理的失败记录位于 `.local-test-data/runs/local-small-model-4df29d3dcd144ddbb21947566332c3f6/`，修正后的结果位于 `.local-test-data/runs/local-small-model-1ecc7f0772bf4b8dbad3c0f3196c870c/`。两次均只是本机研究，不能证明全牌型准确率、赤五识别或 250 ms 端到端目标通过。

另从零训练了只使用上述 FluffyStuff 公有领域牌图的小型 CNN，没有复制外部模型权重或使用游戏截图训练。修正赤五真值后，当前最好的本机已知手牌探针为 40/41，仍有一索误认；训练脚本、输入哈希、权重及失败结果留在本机，尚未打包。项目新增的 Windows ML 后端只调用操作系统自带的 `Windows.AI.MachineLearning`，不分发 ONNX Runtime DLL、Python 或训练框架。提交的微型 ONNX fixture 是本项目自行构造的线性算术图，采用 MIT，不包含牌图或训练数据。细节见 [轻量模型实验](vision-model-research.md)。
