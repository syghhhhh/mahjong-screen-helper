# 第三方依赖与许可证

本项目自身代码和文档采用 [MIT 许可证](../LICENSE)，版权声明为 `Copyright (c) 2026 syghhhhh`。便携包根目录附有项目 `LICENSE`。该授权不覆盖第三方游戏牌图、截图、模型或其他不属于本项目的素材。

当前直接 Rust 依赖：

| 依赖 | 用途 | 上游许可证 |
| --- | --- | --- |
| `serde` / `serde_json` | 校准配置序列化 | MIT OR Apache-2.0 |
| `windows-capture` | Windows Graphics Capture 采集 | MIT |
| `png` | 读取本机 PNG 识牌模板 | MIT OR Apache-2.0 |

间接依赖的精确版本由 `Cargo.lock` 固定。便携包构建时运行 `scripts/generate-license-bundle.ps1`，把 Cargo 元数据中的锁定注册表依赖、许可证字段、许可证原文及文件 SHA-256 写入 `licenses/DEPENDENCIES.json` 和 `licenses/texts/`。该清单包含 Cargo 返回的所有锁定注册表依赖，可能多于 Windows 实际使用的依赖。发布前仍须核对随包分发的模型、牌图、字体和其他资产；目前没有引入第三方模型或牌图。参考项目只用于架构调研，未复制其代码。

## 视觉实验数据的边界

本机诊断曾使用 [pjura 的雀魂牌图数据集](https://huggingface.co/datasets/pjura/mahjong_souls_tiles/raw/main/README.md)。数据集卡标注 Apache-2.0，同时明确图片取自雀魂游戏。该标注没有说明数据集作者拥有游戏原始美术的再分发授权；因此实验牌图仅放在被 Git 忽略的 `.local-test-data/`，不得随本仓库或便携包分发。另一开源识别项目 [QueHun](https://raw.githubusercontent.com/jzjzzzzzzz/quehun/main/templates/actions/README.md) 的按钮模板也标明来自真实客户端，当前不复制进本项目。正式发布前须取得可分发素材的明确授权，或改用本机由用户提供的模板而不打包游戏素材。

另测试了 [FluffyStuff 的公有领域日麻牌图](https://github.com/FluffyStuff/riichi-mahjong-tiles)，上游提交 `26e127ba2117f45cdce5ea0225748cc0cfad3169`。直接将其 37 张 PNG 作为模板，对第一张公开雀魂手牌截图的 14 个槽位进行候选匹配时，整手标签判定失败，默认置信度门槛也未通过。失败摘要保存在本机 `.local-test-data/runs/local-vision-tiles-f765675d230d4766a6cda12a59dcd3dd/`。这套素材具有可分发的授权，但不能未经适配就声称支持默认雀魂牌面，因此当前未打包。
