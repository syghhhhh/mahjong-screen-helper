# mahjong-screen-helper

Windows 雀魂网页版四人日麻只读屏幕辅助程序。v0.0.1 正在开发。当前已有 Rust 牌效与局面状态核心、窗口采集与校准、模板识别核心、实时处理链路，以及可启动的 Windows 浮窗。开发便携包可双击运行，并允许手动输入手牌验证 Top-3 切牌；自动识牌和行动建议尚未经过真实雀魂画面验收。

## 本机运行

在 Windows 10/11 x64 上构建后，双击 `RiichiScreenHelper.exe`。浮窗可寻找浏览器窗口、逐项校准区域、开始或暂停采集，也可在输入框输入 14 张手牌代码后点击“修正”验证牌效。快捷键：F8 暂停或恢复、F9 隐藏或显示、F10 重新识别。自动识牌需要在可执行文件旁的 `models/template-pack/` 中放置完整的本机 PNG 模板包，格式见下方文档。缺少模板包时只提供手动验证；识别不稳定时暂停确定性推荐。

便携包在仓库根目录运行 `scripts/package-portable.ps1 -OutputDirectory <目标目录>` 生成。打包脚本会同时生成 zip；包内没有 Python、Node 或 Rust 运行时。开发包尚不能作为正式版使用：牌面与按钮模板的来源、授权和真实画面准确率仍待验收。

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

Rust 核心测试：`cargo test --workspace --all-targets --locked`。原始游戏截图只允许保存在被 Git 忽略的 `.local-test-data/`。
