# 同一规则二进制的 Windows 主机对照

这是原生规则故障的诊断包，不是 Mahjong Screen Helper 产品发布。它不读取屏幕，不含视觉模型、截图、PDB 或 dump，不联网或修改系统/固件设置。源代码与 MIT 许可证随包保留。

解压到可写目录，双击 `Run-Diagnostics.cmd`，无需 Rust、Python 或 Node。默认最多十次，每次固定十个工作线程独立计算合法手牌；第一次非零退出、超时或测试未实际执行时立即停止。每次上限三十秒，结果保留在独立 `results-<编号>/report.json` 和 stdout/stderr 文件。可用下列命令改变有界次数和超时，实际参数记录在报告中：

```powershell
powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File .\Run-RuleDiagnostic.ps1 -MaxAttempts 10 -TimeoutSeconds 30
```

运行前核对 `manifest.json` 中的源提交与 exe SHA-256 格式，校验实际 exe 并持续持有只读共享锁，防止校验后替换。哈希用于核对文件身份，不是签名或来源可信证明；只运行从本项目已审查构建取得的包。报告记录 OS、CPU 名称、逻辑处理器数量及四个已知 Rust/调试环境变量，不收集机器名、账户、序列号、任意环境变量或画面；原有环境不被覆盖。

`all_attempts_passed` 仅描述这组有限诊断。`acceptance_verified` 始终为 false；结果不是 `native-parallel-resolution` 成功门禁证据，不能证明硬件正常或故障已修复。单次失败与既有失败均须保留。对照时必须先确认所有报告的 `exe_sha256` 相同，且与下载包实际文件一致；不同主机独立重编译后通过不能替代同一文件的对照。

仓库开发者从干净提交构建：

```powershell
scripts/package-rule-probe.ps1 -OutputDirectory "$PWD\.local-test-data\diagnostics\新目录"
```

打包使用 Cargo JSON 产物信息选择唯一的十线程规则契约，只编译不执行；核对前后源码状态，并运行静态 CRT 导入检查。输出目录须尚不存在，所有诊断产物应保存在忽略的 `.local-test-data/`。

独立 GitHub workflow 在 Windows 2022 构建一次，后续 Windows 2022/2025 作业下载同一 artifact、各跑最多二十次，不重新编译，再由开发机下载相同文件对照。原有五项 CI 和版本/回归门禁不被替代。作业之间共享构建产物的方式见 [GitHub 官方说明](https://docs.github.com/en/actions/tutorials/store-and-share-data)；项目另行校验 exe 哈希，因为 artifact 下载校验不等于根因诊断。

## 首次三环境对照结果

源分支提交 `3e444ced49c6511742d95167b251d11b91e20e54` 的[诊断运行 40598298281](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40598298281) 全部通过。默认 PR checkout 的实际构建提交为合并快照 `38d51cce5238c9b625a26d9393cfe0b3511649ca`；下载后核对该提交与 PR 合并引用、全部随包文件哈希，并逐字核对 runner、规则源码和十线程测试（只归一化 CRLF/LF）。

三份报告都运行同一个 SHA-256 为 `1f1e6355caf7d92cd987ce5c8d9380cd5b1f9346c26ca760e8ade4f932ce162c` 的 exe，大小 995,328 字节。逐份核对二十项退出码、超时标记及 stdout 中实际一项测试通过、零跳过的结果：

| 环境 | CPU / 可用逻辑处理器 | 结果 |
| --- | --- | --- |
| GitHub Windows 2022，build 20348 | AMD EPYC 7763 / 4 | 20/20，零失败、零超时 |
| GitHub Windows 2025，build 26100 | AMD EPYC 7763 / 4 | 20/20，零失败、零超时 |
| 开发机 Windows 11，build 26200 | i7-13700K / 24 | 20/20，零失败、零超时 |

两个 GitHub VM 的 CPU 型号相同，不能称作已验证两个不同物理硬件平台；这些有限、相关样本也不是现场应用或硬件稳定性验收。原始报告和下载元数据保存在 `.local-test-data/runs/ci-rule-comparison-b313b784a409408fb2262a51dd646bf0/`；开发机报告在 `ci-rule-comparison-fa7316aa8943489ea968471fd9757f14/native-rule-probe/results-7829c899865042b897ea51df79bdd376/`。汇总 `three-host-comparison.json` 只记录结构化结果，原报告未被改写。

原本机归档程序的 SHA-256 仍为 `7dd9e0b20ff52fbdaaa4220cdaa2adb46b44ff0dbae9987041a8f7b9706c5aa4`，大小 994,304 字节，已有自然访问异常、栈溢出及减法溢出。两个 exe 的内嵌 Rust 编译器路径标识均为 `01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf`，但原始 `.text` 长度相差 512 字节、重叠范围有 71,441 个字节不同；路径、地址与链接布局都可能影响原始字节，尚未做语义等价分析。PE 只读比较保存在第一次下载目录的 `pe-comparison.json`。新文件六十次通过不能覆盖旧文件失败，也不能据此定位硬件、链接器或编译器原因。下一步仍需解释构建差异或在独立主机运行原失败文件，不能把更换文件视作修复。

同源的[主 CI 40598298229](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40598298229) 五项检查也全部通过，于 `2026-10-27T23:02:11Z` 核实。本机最新完整版本仍为 10/11 通过，规则退出 101；相关 failed 状态和原失败证据继续保留，没有用云端绿色结果直接写入 `succeed`。
