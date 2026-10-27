# mahjong-screen-helper

Windows 雀魂网页版四人日麻只读屏幕辅助程序。v0.0.1 正在开发。当前已有 Rust 牌效与局面状态核心、窗口采集与校准、模板识别核心、实时处理链路，以及可启动的 Windows 浮窗。开发便携包可双击运行，并允许手动输入手牌验证 Top-3 切牌；自动识牌和行动建议尚未经过真实雀魂画面验收。

项目代码采用 [MIT 许可证](LICENSE)；雀魂画面和第三方素材不在此授权范围内。

## 本机运行

在 Windows 10/11 x64 上构建后，双击 `RiichiScreenHelper.exe`。浮窗可寻找浏览器窗口、逐项校准手牌、四家副露、牌河、宝牌指示牌及行动按钮共八个区域，开始或暂停采集；也可输入 13 或 14 张手牌代码后点击“修正”验证牌效。旧版五区域配置会保留坐标，须重新校准补齐三家副露区域。快捷键：F7 新局、F8 暂停或恢复、F9 隐藏或显示、F10 重新识别。“新局”会清除上一局的立直、副露、可见牌及手动输入，保留校准、模板和暂停状态；“重识别”保留当前局信息。

自动识牌默认使用可执行文件旁的 `models/template-pack/` 完整本机 PNG 模板包。也可显式安装兼容的小分类器到 `models/tile-classifier/classifier.json`，由清单引用同目录的 ONNX；验证通过后，模型处理模板不确定的手牌和公共牌。牌图模板不完整时由模型承担所有牌面分类，仍要求完整五种按钮模板。已安装模型验证失败时会停止本次启动并显示原因。当前不随包提供训练权重或游戏模板，开箱即用的默认视觉资源仍待完成。

没有模板时可输入当前牌码或按钮标签并点击“学习模板”，逐步从本机画面采集；“开始”会显示已准备的牌图和按钮数量。模板格式与采集步骤见下方文档。公共牌与来牌识别已接入，但仍未通过真实牌桌验收；识别不稳定、模型拒识或执行失败时暂停确定性推荐。

便携包在仓库根目录运行 `scripts/package-portable.ps1 -OutputDirectory <目标目录>` 生成。打包脚本会同时生成 zip；包内没有 Python、Node 或 Rust 运行时。开发包尚不能作为正式版使用：牌面与按钮模板的来源、授权和真实画面准确率仍待验收。

Windows x64 MSVC 构建使用静态 CRT，打包前检查实际 exe 的 DLL 导入，拒绝额外 VC++ 运行库依赖。该审计需要构建机已有的 Visual Studio `dumpbin`，终端用户不需要此工具；导入检查仍不代替干净 Windows 的启动验收。

- [v0.0.1 产品规格](docs/releases/v0.0.1-product-spec.md)
- [v0.0.1 任务与验收](docs/releases/v0.0.1.md)
- [开发和测试流程](docs/development-workflow.md)
- [第三方依赖与许可证](docs/third-party-licenses.md)
- [本机视觉模板包格式](docs/vision-template-pack.md)
- [轻量模型与原生 ONNX 实验](docs/vision-model-research.md)
- [本机模型安装与拒识契约](docs/model-manifest.md)
- [行动按钮输入与原生后端](docs/action-recognition.md)

在 Windows PowerShell 中运行：

```powershell
.\scripts\validate-task.ps1 -Version v0.0.1
.\scripts\test-version.ps1 -Version v0.0.1
.\scripts\test-all.ps1 -Version v0.0.1
```

Rust 核心测试：`cargo test --workspace --all-targets --locked`。原始游戏截图只允许保存在被 Git 忽略的 `.local-test-data/`。
