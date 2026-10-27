# mahjong-screen-helper

Windows 雀魂网页版四人日麻只读屏幕辅助程序。v0.0.1 正在开发；当前仓库已建立自动化开发门禁、Rust 牌效核心、Windows 窗口采集与校准，以及基于本地模板的视觉识别核心。真实游戏截图验收、桌面程序集成与便携包尚未完成。

- [v0.0.1 产品规格](docs/releases/v0.0.1-product-spec.md)
- [v0.0.1 任务与验收](docs/releases/v0.0.1.md)
- [开发和测试流程](docs/development-workflow.md)
- [第三方依赖与许可证](docs/third-party-licenses.md)
- [本机视觉模板包格式](docs/vision-template-pack.md)

在 Windows PowerShell 中运行：

```powershell
.\scripts\validate-task.ps1 -Version v0.0.1
.\scripts\test-version.ps1 -Version v0.0.1
.\scripts\test-all.ps1 -Version v0.0.1
```

Rust 核心测试：`cargo test -p riichi-core --all-targets --locked` 与 `cargo test -p vision-core --all-targets --locked`。原始游戏截图只允许保存在被 Git 忽略的 `.local-test-data/`。
