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
