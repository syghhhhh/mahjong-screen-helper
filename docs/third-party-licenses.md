# 第三方依赖与许可证

本项目的自身许可证尚未确定。发布便携包前应由仓库所有者确定项目许可证，并在包内附上依赖的许可证文本。

当前直接 Rust 依赖：

| 依赖 | 用途 | 上游许可证 |
| --- | --- | --- |
| `serde` / `serde_json` | 校准配置序列化 | MIT OR Apache-2.0 |
| `windows-capture` | Windows Graphics Capture 采集 | MIT |
| `png` | 读取本机 PNG 识牌模板 | MIT OR Apache-2.0 |

间接依赖的精确版本由 `Cargo.lock` 固定。发布打包任务应从锁文件生成完整 SBOM 与许可清单，并检查所有随包分发的模型、牌图、字体和其他资产；目前没有引入第三方模型或牌图。参考项目只用于架构调研，未复制其代码。

## 视觉实验数据的边界

本机诊断曾使用 [pjura 的雀魂牌图数据集](https://huggingface.co/datasets/pjura/mahjong_souls_tiles/raw/main/README.md)。数据集卡标注 Apache-2.0，同时明确图片取自雀魂游戏。该标注没有说明数据集作者拥有游戏原始美术的再分发授权；因此实验牌图仅放在被 Git 忽略的 `.local-test-data/`，不得随本仓库或便携包分发。另一开源识别项目 [QueHun](https://raw.githubusercontent.com/jzjzzzzzzz/quehun/main/templates/actions/README.md) 的按钮模板也标明来自真实客户端，当前不复制进本项目。正式发布前须取得可分发素材的明确授权，或改用本机由用户提供的模板而不打包游戏素材。
