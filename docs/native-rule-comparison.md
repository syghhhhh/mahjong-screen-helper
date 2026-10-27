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
