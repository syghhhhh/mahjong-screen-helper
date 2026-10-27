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

这里保存的是一次真实运行故障，不是已确定的功能代码缺陷。后续捕获结果见下文；仍须定位并修复或证实外部原因，再从 `in_progress` 开始验证。不得通过永久单线程、屏蔽失败测试、把异常改成预期成功或只重跑完整套件来宣称解决。

行动输入的 15 个原创 oracle、21 个输入的逐值一致性、数值后端及原生 shape/STA 测试此前通过，字体候选仍未通过完整按钮识别。这些局部结果均不能替代本次失败的项目回归。

## 后续捕获：已复现，根因仍未确定

在提交 `4a0ae8c893981049897deefa3eff4110c62d9f74`、工作树干净时继续诊断，功能源码与原故障提交相同。调试器默认可能启用调试堆，因此仅对自行启动的子进程设置 `_NO_DEBUG_HEAP=1`，保留默认并行测试。没有修改全局环境、注册表或正式测试脚本。[Windows 调试环境变量](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/general-environment-variables)、[堆设置说明](https://learn.microsoft.com/en-us/windows/win32/api/heapapi/nf-heapapi-heapsetinformation)。

| 条件 | 观察结果 | 本机证据目录（均位于 `.local-test-data/runs/`） |
| --- | --- | --- |
| 原 exe，正常堆 | 第 4 次出现未处理访问异常，实际退出 `0xc0000005`，RIP 为 `0x1a`；dump 已保存 | `pipeline-standard-heap-75dc039b84e8457f92a28c6f7ee6f025` |
| 原 exe，捕获首次访问异常 | 第 6 次在规则搜索指令处读地址 `0x33` 时异常；dump、寄存器、附近栈及代码段已保存 | `pipeline-first-fault-050b807a421b4579b60da25c2ca2a785` |
| 原规则源码独立编译，4 个线程执行合法手牌 | 10 次通过，暂未复现 | `minimal-core-reproducer-36fd7e2ccd694a45b398889baac179e9` |
| 链接原编译规则库，同一独立检查 | 20 次通过，暂未复现 | `linked-core-reproducer-785208d0644c49d98460eaeefde3982a` |
| 全新 Cargo 产物目录构建完整 pipeline | 第 20 次出现首次访问异常；dump 已保存 | `pipeline-clean-reproduction-d85a949d1b3246488c8761771fab41a9` |

首次异常捕获会在保存现场后主动终止子进程，避免异常处理期间再次出错覆盖首个现场；此类记录的强制退出 `1` 不是原测试的自然返回结果。正常堆第 4 次的未处理异常才是这一批的自然失败退出证据。

全新构建命令为：

```powershell
cargo test -p runtime-core --test pipeline --locked --no-run --target-dir .local-test-data/builds/pipeline-clean-20261028
```

新 exe SHA-256 为 `3379900af9cf658583a48e3c52cd57d183068276b96280354ae4d035b1958259`。原产物、全新产物以及首次异常时读取的 `.text` 段逐字节一致，SHA-256 均为 `c5bef746818db03a0792201ab5d6b1828ca8b15eb1239bb1edf8a75166091f92`。新旧 `.data`、`.pdata`、`.fptable` 和 `.reloc` 也一致；`.rdata` 的 23 字节差异集中于调试记录位置。不能仅把故障归因于旧构建缓存。

原 exe 首次异常时，当前规则搜索帧的工作指针槽值为 `0x21`，同帧另一份入参副本仍指向栈内数组。候选栈符号涉及 `search_normal` 及 `Iterator::find/try_fold`，但栈字中也有不落在正常调用返回位置的地址。这些是局部指针/栈异常线索，**不是经过展开验证的完整调用栈，也没有确定写坏内存的指令**。独立规则检查仅覆盖选定手牌和不同进程布局，不能据通过结果排除规则或构建问题。

另做了进程级调度对照：通过 Windows CPU set 查询得到两组逻辑处理器，仅限制该测试子进程；低效率等级组 8 个逻辑处理器、较高等级组 16 个、全部 24 个分别运行 30 次，均暂未复现。目录分别为 `pipeline-affinity-low-4d8fe69993394e0e83c95144c8296be5`、`pipeline-affinity-high-e7a4e40696084664b6be758f36162c2e`、`pipeline-affinity-all-fc6c3decafb14f9b99e12d63abc9c718`。低等级组部分运行与另一诊断进程重叠，不能视为受控性能比较。亲和性会同时改变时序及可用并行度，有限样本不足以判定处理器、调度器或硬件原因。该设置不进入产品和正式测试。

硬件断点诊断只作用于自己的子进程。外层规则帧指针写入监视的 5 次运行暂未复现；内部帧采样因 45 秒时限终止，未捕获写入；`find` 返回点检查经 1,326,411 次断点观察未见指针副本不一致，因诊断开销主动终止。这两种终止均是诊断未完成，不是测试通过。相关目录分别为 `pipeline-counts-watch-3f85a21b001147b28fb1cbe1b4a9d465`、`pipeline-inner-counts-watch-ccda4b5f8a394a5e9daebd83e811e7c3`、`pipeline-find-return-watch-d93803f65bdf465588c8f435333c3a62`。早期逐次首次异常捕获工具还发生过异常处理中的重复故障，导致同名现场文件被后续异常覆盖；`pipeline-first-chance-45e0ba3811f84c20b15fcfa507640af5` 保留这次失败，不用其被覆盖文件解释首个现场。后续采用上文保存后立即停止的捕获方式。

原始调试堆 30 次通过、上述暂未复现结果以及当前提交 GitHub CI 五项通过，均不能消除三次已捕获的本机访问异常。下一步需要捕获首次异常前的实际写入或可靠调用路径，获得可检验的原因；在此之前 V001-006 保持 `failed`，不发布、不合并。

## 进一步缩小：规则库与标准线程即可触发

在干净提交 `01ea22ee99c2a07306b6e5039b04c1c44d242426` 上，给相同 pipeline 源码的诊断构建添加 `-C force-frame-pointers=yes`。第 2 次运行退出 101：规则搜索 `lib.rs:167` 减法溢出；记录为 `pipeline-frame-pointer-b81f29428a8a4b0bb0c3d66fd6817790`，exe SHA-256 为 `21c336da4b68131fd89712b57de457736efe70c6b872a1f073432b09a91dad56`。

同一诊断 exe 启用子进程 `RUST_BACKTRACE=full` 后，首次运行出现多个 Rust panic，其中一个在 `lib.rs:126` 使用了 `420118248864` 这一越界索引。完整回溯经过规则递归、向听、有效牌、推荐以及碰后局面集成；异常前的 dump 已保存，进程自然退出 101。记录为 `pipeline-frame-panic-513edf103f3a4c05bfd6effa5945bf3c`。标准库仍有未保留帧指针的函数，手工 RBP 链为空，不把原始栈字解释为已验证调用链。只单独执行该碰后测试 30 次暂未复现，记录为 `pipeline-single-test-21340afd608a4a839ad1c0b4b9c909bf`；单项执行不进入正式测试配置。

随后用原编译规则库构造更小的程序，所有工作线程各自创建手牌、计数和推荐结果，无共享可变手牌：

| 独立程序 | 首次捕获结果 | 本机目录 |
| --- | --- | --- |
| 规则库 + 10 个标准测试，不链接视觉/状态/浮窗 | 第 2 次读地址 `0x22` 时访问异常 | `core-test-harness-7d6a1a894678419aa7a76d0f9ac54bf9` |
| 规则库 + 10 个普通标准线程，无测试框架 | 第 5 次读地址 `0x21` 时访问异常 | `core-ten-threads-318444b8e4f845289c8ab88d6d2a751b` |
| 原规则源码独立编译，Rust 1.94.0，静态 CRT，10 线程 | 第 2 次试图执行地址 `0x21` 时访问异常 | `core-stable-static-67a2d599f95f43a49d068979a9191d3a` |
| 原规则源码独立编译，Rust 1.93.1，动态 CRT，10 线程 | 第 5 次读地址 `0x22` 时访问异常 | `core-1.93.1-dynamic-aa5d0999a2454ba1be508514b4b959e5` |

这些访问异常均在正常堆的 Windows 调试事件下捕获，保存后主动停止，强制退出不能冒充自然失败返回码。原始完整回归的自然访问异常仍独立保留。不同编译器、CRT 及线程数的结果不能直接证明算法、编译器或机器哪一方有问题。

另用 MSVC 14.41.34120 编译独立 C++ 普通手搜索对照，10 个线程、每次递归检查计数恢复，10 次运行通过，目录 `cpp-rule-control-da1c4d602112482e92d41096363aa06d`。它不是完整牌效实现，代码、栈布局和时序不同，不能凭其通过判定 Rust 有缺陷或本机硬件正常。

为持续保留这条风险路径，新增 `crates/riichi-core/tests/concurrency.rs` 并纳入版本及规则回归。格式化和该 crate 的 Clippy 通过；新契约直接正常启动（无调试器、无亲和性修改）的有界 10 次检查通过，记录为 `concurrency-contract-a3452dc4d3c74fd6b9370f0dd6a11c8b`，保留未提交测试源码 SHA 和构建哈希。这是新增测试的验证，**不是故障修复证据**。新增 V001-009 继续 `in_progress`，要求根因与本机 `native-parallel-resolution` 证据；CI 通过不能单独完成它。

## 干净提交后的完整回归再次自然失败

提交 `62c757eb4dfc39010877db4ec85fbc8d71d990b1`、工作树干净时执行 `scripts/test-all.ps1 -Version v0.0.1`。运行 `all-0996c22b41d644199ac8b76417d8daea` 于 `2026-10-27T20:43:30.8721206Z` 结束，9 项中 8 项通过、1 项失败，无跳过。完整版本套件、浮窗集成及新增规则并行契约通过，但 `tests/regression/state-actions.test.ps1` 的 `game-state/tests/public_contract.rs` 进程以 `0xc0000005 STATUS_ACCESS_VIOLATION` 自然退出。本次未附加调试器、未改堆或处理器亲和性。

原始摘要/日志保持原样；本机 `state-access-violation-d54ba5e3b41c4956a1915b8395ad70af` 保存 exe、PDB、完整控制台副本和失败 JSON。exe SHA-256 为 `a05c91da902bfb70f5da479215990e0591a28307e5caacd74263da7f5cfb76cb`，PDB 为 `a5fe53f583a3fd2c105d3dc46b81285891176e63dbf2f597f82e5ef921f3ee02`。V001-009 置为 `failed` 并追加失败历史，原 V001-006 失败不删除；这些结果不能证明两进程故障一定同因。

这次日志还暴露了独立流程缺陷：PowerShell 5 的 UTF-8 命令头与 `Tee-Object` 默认 UTF-16 内容混入同一日志，原文件 93,818 字节中有 46,155 个 NUL 字节，直接文本检索漏掉失败行。另存的原控制台为正常 UTF-8，可确认上面的自然退出。原日志不转换；新增 V001-010 专门修复两个入口的逐行编码并测试失败传播，不把日志修正当作原生故障修复。

## 新并行契约在正常完整测试中捕获故障

日志修正提交 `12db20f6f69278c0c51e58d19a2c93315fa1aff1` 的干净完整运行 `all-add896cdaad741b38996799197b1cbf7` 在 `2026-10-27T20:50:20.3468100Z` 结束：10 项中 8 项通过、2 项失败，无跳过，未附加调试器或改变线程/堆默认值。

- 嵌套版本套件中的新 `concurrency.rs` 契约自然失败：工作线程在规则 `lib.rs:154` 减法溢出，外层 scoped thread 报告子线程 panic，测试进程退出 101。这证明新契约能够在实际完整运行中暴露故障，之前 10 次通过不再被误用为排除证据。
- `overlay-integration` 的原 pipeline 进程报告栈溢出，随后自然退出 `0xc0000005`。原 exe/PDB 与最初保存的哈希相同。
- 两个日志契约脚本通过。完整日志 46,697 字节，严格 UTF-8 解码成功，NUL 字节数为 0，可直接检索到两处失败；日志修正有效，原生问题未修复。

本机 `concurrency-natural-failure-541a286eb6b44cab876631f7a94c35bc` 保存两个 exe/PDB、完整控制台及失败 JSON。规则契约 exe SHA-256 为 `7dd9e0b20ff52fbdaaa4220cdaa2adb46b44ff0dbae9987041a8f7b9706c5aa4`，PDB 为 `e13dc703c17f03ee86fc45e92005e5fab9a95d0cf775b27fe4fd51e065082d0b`。V001-009 保持 `failed` 并追加本次失败；V001-010 保留 `self_tested` 阶段信息，状态为依赖 V001-009 的 `blocked`，完整回归未通过前不执行成功门禁。不再重复运行本提交以取得一次绿色结果。

## 断言诊断、环境线索与同一二进制对照

后续在本机副本增加递归前后牌数恢复断言、独立 while 查找首张非零牌与 `find` 的比较。该副本正常运行 10 次及调试运行 10 次暂未复现，分别保存在 `rule-invariant-probe-2d18f21e1d7d463790eaef23cdc9db73` 和 `rule-invariant-debug-e6a3ac90874c48bf8567a54234c321b4`。原十线程规则二进制的无调试器 30 次检查也暂未复现，保存在 `original-rule-native-3c843c2cb020430a91f1c627b359f872`。不含项目代码的普通递归、七参数递归及通用分支数组探针各做了正常/调试对照，均暂未触发，目录为 `generic-recursion-c361c255a53c44c3b09734730870c611`、`generic-stack-args-b98c6fdc29ee4122ba52c881407f999f`、`generic-branching-9c762e4bd7ee4064888cbf16ebad3527`。这些副本和探针改变代码、栈或时序，均不能当作修复，也未修改生产算法。

只读 CIM/注册表查询得到：处理器 i7-13700K，整机厂商 THUNDEROBOT，主板标识 `PRO Z790-A WIFI DDR4 (MS-7E07)`，BIOS 1.11、日期 2022-09-19。Windows `Update Revision` 是四字节 `0B-01-00-00`，按小端整数为 `0x10B`；这不是直接读取处理器 MSR 的结果。最近三天 WHEA 查询返回无匹配事件，不足以排除硬件因素。

[Intel 官方当前说明](https://www.intel.com/content/www/us/en/support/articles/000102331/processors.html)（2026-07-21 复核）建议相关 13/14 代桌面处理器遵循 Intel 默认设置，使用含 `0x12F` 或更高微码的最新 BIOS。本机报告的固件信息因此成为待排除的环境线索，**尚未证实本次故障来自硬件、固件或编译器**。若处理固件，须由整机厂商核对匹配该整机的更新；本次检查仅收集信息。

已将同一个自然失败规则契约 exe 制成独立对照包，SHA 保持 `7dd9…c5aa4`，约 404 KiB，可解压双击运行，运行前验证哈希；最多 10 次、每次 30 秒，遇首次失败停止。报告只记录规则输出、退出码与选定环境字段，不包含截图或序列号。包位于 `.local-test-data/diagnostics/rule-runtime-probe-beb769087501415bb74bf450d2d938e7.zip`，ZIP SHA-256 为 `7a1dcc95d4b547c77f1c9a9e9c5c6d210c93db0d855da1c430352b0426859f33`。

该包在本机单次验证于 `2026-10-27T21:02:20.4775693Z` 结束，仍自然退出 `0xC0000005`，未超时；报告及 stderr 保存在同名目录的 `results-20261028-050214-2ad2ece23bfc4ae59f518a0359582fe3`。报告类型为 `independent-host-rule-reproduction`，明确不能充当 `native-parallel-resolution`。另一台 Windows 上的同一二进制对照仍待完成。

## CI 编码边界失败

提交 `964828b` 的 [CI 40581870231](https://github.com/syghhhhh/mahjong-screen-helper/actions/runs/40581870231) 中，版本和回归检查因日志契约失败，其他三项通过。下载的 Windows Server 2022 日志显示 `stdout-中文Ω` 已在原生子进程边界丢失；统一文件写入编码尚未覆盖英文系统的默认 OEM 控制台编码。这一 CI 失败不是原生访问异常的证据。

新增 CP437 启动基线先在本机复现编码失败，再让两个入口显式设置原生输出/管道解码为 UTF-8。测试还验证子进程实际报告 `encoding-65001`，中文/非 ASCII stdout 与 stderr 均保留，故意失败及只读日志负例仍失败。此修正继续由 V001-010 记录，原生计算任务仍未通过。

修正后的干净提交 `6dd27fe7cba190e499007191afc0bdd4789d4c6a` 执行了一次新的完整验证，`all-2ac6a027e96049ca9b45de40d614561a` 在 `2026-10-27T21:08:36.2739114Z` 结束：10 项全部通过，无跳过，嵌套版本运行 `version-v0.0.1-ffbdf54bb7214b0f8ad453462de657bb` 通过。这次源变更只修正日志编码边界，没有修改规则算法。成功结果追加记录，不替代任何旧失败，不把 V001-009 改为成功；V001-010 已验证到回归阶段，但其声明的 V001-009 依赖仍失败，故维持阻塞。新 CI 需另外检查。

## 行动输入修复后的正常回归仍触发相同二进制异常

完整笔画输入提交 `b6c4a0fc7559697df872fabb8b059ea975e6a358` 的干净完整运行 `all-components-0026fd17ca554ea490c136dede73531a` 于 `2026-10-27T21:38:20.1669374Z` 结束：10 项中 9 项通过、1 项失败，无跳过。嵌套版本 `version-v0.0.1-f5fd59b077b04af8b224baef2b934879`、视觉、集成、状态和日志回归通过，但规则回归的 `concurrency.rs` 测试进程自然退出 `0xc0000005 STATUS_ACCESS_VIOLATION`。没有调试器、堆设置或亲和性变更；没有为取得绿色结果重跑本提交。

本次规则 exe SHA-256 仍为 `7dd9e0b20ff52fbdaaa4220cdaa2adb46b44ff0dbae9987041a8f7b9706c5aa4`，PDB 仍为 `e13dc703c17f03ee86fc45e92005e5fab9a95d0cf775b27fe4fd51e065082d0b`，与前次保存的规则二进制完全相同。行动输入变更没有重新生成这份规则二进制；相同二进制可先通过嵌套版本、后在回归中失败，进一步说明单次通过不足以排除间歇性问题，仍不能确定根因。

本机 `component-regression-native-failure-4a4f329c3b25451e9ac2c4ffde987759` 保存 exe、PDB、摘要、完整日志及控制台副本。日志 49,915 字节，严格 UTF-8 有效，NUL 为 0，可直接检索异常。V001-003 保留未完成状态，V001-009 保持失败，V001-010 更新阻塞运行编号；未执行成功门禁。
