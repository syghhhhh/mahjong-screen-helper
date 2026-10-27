# 自动化开发与测试闸门流程

## 1. 目标

本流程用于约束智能体和人工开发者对 `mahjong-screen-helper` 的持续迭代，避免代码、测试和任务状态逐步失真。

每个版本必须同时维护：

- 版本开发文档。
- 版本 `task.json`。
- 版本专用测试。
- 全项目回归测试。
- 可追溯的测试证据。

只有专用测试和全项目回归测试都通过，任务才允许进入 `succeed`。

## 2. 目录约定

Windows PowerShell 5 下，测试入口必须以显式 UTF-8 写入命令及每一行 stdout/stderr，不得将默认使用 UTF-16 的 `Tee-Object` 追加到 UTF-8 日志。日志写入失败必须导致入口失败，不能生成成功证据。日志编码、写入失败及退出码传播由最小仓库集成测试持续验证；旧的失败原始日志保留，不原地转换或覆盖。

并行测试的临时目录不得只用 PID 和时钟命名；Windows 的时钟分辨率不保证同一进程的两个调用返回不同值。共用测试目录创建助手以原子序号区分同刻调用，并通过排他创建确认所有权；遇重名有限次尝试新名字，不复用或删除原目录，其他 I/O 错误立即失败。目录隔离须有固定相同时钟、并行调用、原目录保护和重试上限测试。

原生子进程的输出编码与 PowerShell 管道解码也必须统一为 UTF-8；只指定文件编码不足以保留英文 Windows 默认 OEM 编码下的中文。集成测试从 CP437 启动两个入口，验证其子进程实际使用代码页 65001。

```text
docs/
  development-workflow.md
  releases/
    v0.0.1.md

tasks/
  v0.0.1/
    task.json

tests/
  version/
    v0.0.1/
  regression/
  fixtures/
    synthetic/

scripts/
  test-version.ps1
  test-all.ps1
  task-gate.ps1
  validate-task.ps1
  validate-local-evidence.ps1
```

真实截图、个人对局数据和本机模型文件只允许放在 `.local-test-data/`，该目录不提交到 Git。仓库中只保存合成 fixture、脱敏结构化结果、哈希和测试摘要。

## 3. 任务状态机

正常路径：

```text
planned → in_progress → implemented → self_tested → regression_tested → succeed
```

异常状态：

- `failed`：测试失败或验收条件不满足。
- `blocked`：被外部依赖、权限、数据或环境问题阻塞。

状态含义：

- `planned`：任务已定义，尚未开始。
- `in_progress`：正在修改代码。
- `implemented`：代码完成，但还没有完成测试。
- `self_tested`：本版本专用测试通过。
- `regression_tested`：全项目回归测试通过。
- `succeed`：门禁脚本验证证据后自动写入。

智能体可以更新除 `succeed` 之外的工作状态，但不得手工把任务改成 `succeed`。`task-gate.ps1` 只接受当前状态为 `regression_tested` 的任务，并在所有校验成功后写入 `succeed`。

## 4. 开发执行顺序

1. 读取本文件、当前版本文档和 `tasks/<version>/task.json`。
2. 检查任务依赖、Git 状态和当前提交。
3. 明确任务影响的模块和测试文件。
4. 先添加或更新测试，再修改实现。
5. 运行格式化、静态检查和版本专用测试。
6. 检查完整测试输出和失败原因，不能只看退出码。
7. 提交代码，使测试证据对应一个明确的 `source_sha`。
8. 在该提交上运行全项目回归测试。
9. 将任务状态更新到 `regression_tested`。
10. 执行 `scripts/task-gate.ps1`。
11. 只在门禁脚本成功后允许出现 `succeed`。
12. 检查最终 diff，确认没有删除测试、隐藏失败或未记录的行为变化。
13. 通过 Pull Request 合并到 `main`。

失败时必须保留日志，并把任务置为 `failed` 或 `blocked`。修复后从 `in_progress` 重新开始，不能用一次成功重跑覆盖既有失败记录。

任务可用追加的 `failure_history` 保存失败提交、命令、运行编号、退出码、日志/摘要位置、环境及诊断结果；后续状态变化不得删除历史项。诊断成功与原始失败分开记录，不能据此补写成功证据。崩溃 dump、二进制、符号文件及系统报告仅放在本机忽略目录。

## 5. 测试分层

### 5.1 版本专用测试

位置：`tests/version/<version>/`

覆盖当前版本新增的规则、模块、集成流程、视觉 fixture、性能和打包要求。运行：

```powershell
scripts/test-version.ps1 -Version v0.0.1
```

### 5.2 全项目回归测试

位置：`tests/regression/`

覆盖已经完成版本的核心规则、状态机、数据接口、识别流程、配置迁移、构建和便携包冒烟测试。运行：

```powershell
scripts/test-all.ps1
```

如果新功能改变旧行为，必须同时修改旧测试期望、版本文档和兼容性测试，之后重新执行完整回归。不能只修改测试让它通过而不记录行为变化。

### 5.3 测试分类

测试统一使用以下分类名称：

```text
unit
integration
fixture
e2e
performance
packaging
regression
```

测试框架必须启用严格 marker 或等价的未知分类检查，防止测试因为拼写错误被静默跳过。

## 6. 测试证据

每次门禁运行至少记录：

- 被测试的源代码提交 SHA。
- 版本测试运行编号。
- 回归测试运行编号。
- 实际执行的命令。
- 日志路径。
- 操作系统和 CPU 架构。
- fixture 哈希。
- 完成时间。

代码提交和 `task.json` 状态更新可以是两个提交，但 `task.json` 必须保存被测试的源代码 SHA。缺少任意关键证据时，任务不能变成 `succeed`。

需要真实画面或目标机器验收的任务在 `local_evidence_required` 列出证据类型。执行门禁时用 `-LocalEvidencePaths` 传入位于 `.local-test-data/` 的本机摘要 JSON；每种类型恰好一份。摘要须包含 `kind`、`run_id`、`source_sha`、`passed: true`、命令、日志路径、操作系统与架构、完成时间，以及至少一个本机 fixture 的路径和 SHA-256。门禁会复核日志、fixture 文件及哈希，并要求摘要与当前提交一致。候选诊断、空闲浮窗测试或失败摘要不能替代声明的真实对局验收类型。

## 7. GitHub 合并门禁

`main` 分支禁止直接推送，必须通过 Pull Request 和 required checks：

```text
task-schema
version-tests
regression-tests
build-smoke
package-smoke
```

新提交后必须重新运行检查。GitHub Actions 默认使用 `pull_request`，workflow 依赖固定到完整 commit SHA，并保存测试日志和构建产物。涉及 workflow、测试或 `task.json` 的变更必须触发对应校验。

## 8. 质量增强措施

- 每个任务声明 `scope` 和 `affected_tests`，门禁检查代码影响范围是否有对应测试。
- 规则核心增加属性测试，例如牌数合法、单牌最多四张、推荐切牌来自手牌、重复计算结果一致。
- Rust 规则核心稳定后定期运行 `cargo-mutants`，检查测试是否能捕获故意插入的错误。
- 所有测试设置超时；永久 skip 必须有原因和跟踪编号。
- flaky 测试单独记录，不能自动当作成功。
- 每个版本建立 Git tag，并保留通过完整门禁的便携包。
- 配置格式变化必须提供迁移测试。
- 发现回归时优先回滚到最近一个通过门禁的版本 tag，不重写历史任务状态。

## 9. 门禁脚本接口

```powershell
scripts/validate-task.ps1 -Version v0.0.1
scripts/test-version.ps1 -Version v0.0.1
scripts/test-all.ps1
scripts/task-gate.ps1 -Version v0.0.1 -TaskId V001-001
scripts/task-gate.ps1 -Version v0.0.1 -TaskId V001-003 -LocalEvidencePaths @('.local-test-data/runs/手牌验收/summary.json', '.local-test-data/runs/按钮验收/summary.json')
```

门禁脚本失败时返回非零退出码，并且不得写入 `succeed`。

### 测试期间的一致性

门禁持有当前版本的独占锁，避免两次门禁同时修改状态。测试前记录 HEAD 与任务文件原始字节；测试完成及写入前再次核对 HEAD、工作区、任务文件，并重新检查本机证据和 fixture 哈希。测试期间改变源文件、测试、提交或任务文件时，本轮门禁失效；保留其他编辑者的新内容，不覆盖为旧状态。

成功状态先写入任务目录的临时文件并校验结构，再以原子替换更新 `task.json`；原始字节保留为本轮完整回归目录中的 `task-before-gate.json`。失败时清理本次临时文件并释放锁。该检查是写入前的一致性检查，不是文件系统隔离或对任意外部编辑者的事务保证；运行门禁期间智能体仍不得修改源代码、测试、任务或证据文件。隔离微型仓库测试覆盖中途改源文件、改 HEAD、改任务、改本机 fixture、跳过测试和正常通过六种路径。

## 10. 视觉真值与模型实验

- 推理前保存样本来源、ROI、原始文件 SHA-256、人工标签及标注时间；冻结权重和预处理版本后再运行新样本。标签不能来自待测模型本身。
- 数牌须逐张数符号；疑似图案放大复核。赤五分别检查主体颜色和赤色标记，空白牌须在完整牌槽上下文中确认。不能只凭缩略图或文章中另一副牌的文字猜标签。
- 发现标注错误时保留旧标签、旧预测和旧分数，新增真值修订及复算摘要；记录更正理由、可复核的视觉证据、旧摘要和新真值哈希。不得改写旧运行目录来隐藏更正。
- 参与训练或调参的图片属于开发集。数据增强产生的多个变体和同一局的相邻帧不是独立对局样本；报告时说明来源、相关性和覆盖范围。
- 分类最高标签命中率、数值后端一致性、未知输入拒识、现场连续识别和端到端性能分别评估。模型高 softmax 分数不能单独当作可靠置信度；必须保留非牌面、背景、遮挡和未知字形的压力测试。

字牌真值同时填写独立的观察名称和数字牌码。手牌用 `observed_honors` 记录从 1 开始的位置及 `observed_honor`，公共牌在当前牌项填写 `observed_honor`。观察名称来自原图复核，不能从已有数字牌码反向生成。统一编码如下，与本项目索引及[常用立直实现](https://github.com/MahjongRepository/mahjong/blob/master/mahjong/constants.py)一致：

| observed_honor | 原图含义 | 牌码 |
| --- | --- | --- |
| east | 東 / 东 | `1z` |
| south | 南 | `2z` |
| west | 西 | `3z` |
| north | 北 | `4z` |
| white | 白板空白面 | `5z` |
| green | 發 / 发 / 発 | `6z` |
| red | 中 | `7z` |

版本与回归套件会检查当前视觉真值的名称/牌码对应、缺失/重复观察、位置范围及手牌四张上限，并用故意把西写成北的负例验证检查确实失败。这些检查验证标签录入一致性，原图内容及人工观察仍须单独复核。修订当前标签时继续保留旧清单字节、旧模型输出和单独复算证据。
