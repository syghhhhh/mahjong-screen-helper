# 2026-10-28：模板局面集成进程访问异常

本次完整回归未通过，V001-006 已置为 `failed`。故障根因尚未定位；不发布、不合并此开发草稿，不用诊断重跑成功覆盖失败。

## 原始证据

- 代码提交：`9c3cbd2c1ee41adacf83cad182baaa5e4d4bf32b`，运行时工作树干净。
- 命令：`scripts/test-all.ps1 -Version v0.0.1`。
- 完整摘要：`.local-test-data/runs/all-b8e86d5d08ea4f11a1690ec055d90a1c/all-tests.json`，9 项中 8 项退出 0、1 项失败，无跳过；其中完整版本测试通过。
- 失败检查：`tests/regression/overlay-integration.test.ps1` 在 `cargo test -p runtime-core --all-targets --locked` 阶段退出 1。
- 子进程：`pipeline-6c18002044607b8f.exe`，运行 10 个图像/局面测试，只完成一项输出后以 `0xc0000005 STATUS_ACCESS_VIOLATION` 退出，没有正常 Rust panic 栈。
- Windows Application Error 记录故障地址 `0x1`，模块 unknown/StackHash。该进程只走合成模板、局面与规则，没有安装牌面分类器；系统模块清单也未显示 WinML。当前证据不足以认定是此次行动后端改动、旧缺陷、构建环境或机器问题。

环境为 Windows 11 x64 10.0.26200、Rust 1.93.1。原始完整日志、失败摘要不改写；当时的 exe、PDB、Report.wer、文件哈希和诊断记录保存在本机：

```text
.local-test-data/runs/pipeline-access-violation-9bb800bb715546d3b5db692aca5ddb69/
```

exe SHA-256 为 `8789767b18e1629a0b4720402bf150fe607c3a2c2202423182d83f515d65c96c`；PDB SHA-256 为 `54d47f0a06c916c0b1d543f1283a591fa28e202300b22731da03d8f1c0aa3427`。这些私有诊断产物不上传 CI 或仓库。

## 已做诊断与限制

保留原二进制和符号文件后，在本机对该副本做 30 次 Windows 调试事件下的并行测试运行，均通过。调试器只控制自行启动的合成测试子进程，无全局注册表、系统配置、测试串行化或放宽断言；若再次出现未处理异常，辅助脚本会保存线程寄存器、候选栈符号和 minidump。该批未复现，因此没有崩溃 dump。调试器会改变时序，30 次通过不能排除原始问题，更不能把完整回归改成成功。

这里保存的是一次真实运行故障，不是已确定的功能代码缺陷。下一步仍需用原二进制和正常线程/堆行为捕获现场，获得可信调用栈，定位并修复或证实外部原因，再从 `in_progress` 开始验证。不得通过永久单线程、屏蔽失败测试、把异常改成预期成功或只重跑完整套件来宣称解决。

行动输入的 15 个原创 oracle、21 个输入的逐值一致性、数值后端及原生 shape/STA 测试此前通过，字体候选仍未通过完整按钮识别。这些局部结果均不能替代本次失败的项目回归。
