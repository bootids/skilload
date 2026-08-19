---
plan_id: PLAN-0003
branch: codex/p2-library-portable-import-export
pull_request: https://github.com/bootids/skilload/pull/3
status: review
depends_on: [PLAN-0002]
---

# 建立可移植 Library 导入与导出


本交付让用户能够在不联网、不建立 Trust、也不接触外部 Skill 内容的前提下，将一个受限且版本化的 Library 元数据文件预演导入、原子导入到本机持久状态，并重新导出为确定性文件。用户通过 `skilload library import --input <PATH> [--dry-run]` 和 `skilload library export --output <PATH>` 观察结果；导入后导出的文档只包含可移植的已解析 Skill 证据和用户元数据，不包含本机路径、凭据、Trust、缓存或部署状态。

本 ExecPlan 是活文档。实施过程中必须持续更新 `Progress`、`Surprises & Discoveries`、`Decision Log`、`Outcomes & Retrospective` 与 `Review Conversation Log`。本文件必须遵守仓库根目录的 `docs/PLANS.md`。

## Delivery Metadata


本交付是完成的 `PLAN-0002`（Rust 工作区与配置垂直切片）之后的直接后继。`PLAN-0002` 已在默认分支完成并提供了 Cargo 工作区、`skilload-core` 的内向应用边界、`skilload-cli` 的薄展示适配器、严格 XDG 根解析以及当时的 API-v1 配置渲染模式；本 Plan 的 active rework 将 current producer 统一切换到 API-v2。`PLAN-0001` 是其已完成的文档前提，因而不重复列为直接依赖。

本交付只建立可移植 Library 元数据传输与为它服务的最小 SQLite 持久化边界。它不实现 GitHub 输入解析、网络解析、首次 Trust 审批、Library add/remove/list/search/get/refresh、直接元数据编辑、FTS 查询、缓存、工作区、全局部署、manager、doctor 或数据库迁移/恢复命令。未实现的命令必须继续是 usage error，不能注册占位叶子。本计划在 `plan` 状态及 Draft PR 中仅定义和发布工作；只有后续明确的人类执行授权才可以移动到 `active` 并改动实现。

## Product Baseline


本交付完整实现并验证以下两个原子行为：`SKL-LIB-009` 为 Revision 3，`SKL-LIB-010` 为 Revision 4。

* `docs/product-specs/library.md` 中 Revision 3 的 `SKL-LIB-009` 要求 `library export --output <PATH>` 将确定性、版本化且仅含 Library 来源/元数据的 `LibraryExportData` 原子写入请求路径；文件排除 Trust、全局/工作区状态、profile、绝对本机路径、凭据、缓存内容和操作时间，命令自身仍产生既定人类或 API-v2 结果。它在创建 staging 前拒绝活动 database、WAL、SHM 或 database lock target；rename 前失败保留旧 output 或无 output，rename 后父目录 sync 失败返回错误且不声称旧 output 仍在。
* 同一文件中 Revision 4 的 `SKL-LIB-010` 要求 `library import --input <PATH> [--dry-run]` 在读取前以 no-follow、nonblocking descriptor 和 `fstat` 拒绝非常规或 identity-drift input，并在构建任何 model 或 `ImportPlan` 前执行流式非模型预验证。它分别限制 67,108,864 bytes、10,000 entry objects、1,000,000 JSON values、八层 object/array、1,048,576-byte string token 和 128-byte number token，拒绝 duplicate object key、invalid JSON、unknown field、wrong type 和 invalid metadata；整个 batch 要么提交、要么不改变 durable Library，alias 或同 batch canonical duplicate 均以规定的 `internal_duplicate` conflict rollback，dry-run 与未变基线上的实际 import 报告相同计划。

  Revision 4 还要求六种 ceiling 以 API-v2 独立 code `library_input_limit_exceeded` 返回 `LimitDetails` 的 first exceeded dimension、无损 measured/allowed decimal 值和 input `PathValue`；该 code 不得重用 API-v1 仅适用于 Agent project-input 的 `agent_input_limit_exceeded`。首次 import 在 data-directory descriptor 内 no-clobber 发布 staging database；commit 前失败只清理本调用创建且 identity 未变的 state，commit 后 durability-sync failure 返回错误且不伪称 state 未改变。

Revision 3/4 的“同一可移植文档”与严格 input byte ceiling 共同要求完整的 P2 durable Library 也能被当前 import 读取：import 与 dry-run 必须在 mutation/result 前计数 post-import deterministic document，export 也必须在 staging 前执行相同检查。它是既有单一 transfer format 的实现闭环与 defect 修复，不引入新命令、字段、API code 或行为 revision。

导入文件中的 `ResolvedSkill`、`SourceIdentity`、完整 SHA、完整性摘要、已验证名称、描述和计数必须满足 API-v2 的可移植表示。为防止损坏的本地记录，本交付会复用 `SKL-SRC-002`、`SKL-SRC-007` 与 `SKL-SRC-012` 的 canonical source、名称与摘要约束，并对 alias/category/tag/note 执行 `SKL-LIB-008` 的大小、Unicode 15.1.0、NFC、`White_Space` 裁剪和 C/F 完整默认大小写折叠规则。这些约束的局部复用不表示来源获取、直接元数据命令或完整 Source/Library 行为已经完成；`SKL-SRC-*`、`SKL-LIB-001`、`SKL-LIB-004`、`SKL-LIB-005`、`SKL-LIB-008` 和 `SKL-LIB-011` 仍保持 planned，直到各自完整 acceptance 被独立交付。

Revision 2 的 `SKL-CLI-004`、`SKL-CLI-005` 与 `SKL-CLI-012` 以 API-v2 current-producer cutover 的最小必要范围加入本交付；它不增加命令、双版本协商或 API-v1 compatibility mode。其余 `SKL-CLI-*`、`SKL-OPS-*` 不在本次完成基线中。本交付遵守适用约束：JSON stdout 只写一个 API-v2 信封、常见成功结果正确区分 observed/changed/unchanged、路径用 `PathValue`、读和 dry-run 不联网且不创建 skilload 根、导入写入仅在完整验证之后发生；未知较高 schema 拒绝写入，已识别的数据库损坏绝不被静默替换且必须返回 `database_corrupt` 的 `DatabaseCorruptDetails`。P2 不创建备份或导出位置索引，因此该诊断如实返回空 `backups` 和 `recoverable_exports` 集合、数据库 `PathValue` 与 `database-corruption-v1`；但不会宣称这些跨全产品行为的全部 acceptance 已满足。

完成时的可观测证明是：用户先对合法 regular-file 导入文件运行带 `--dry-run --json` 的命令，得到 `library.import` 的 `observed` 结果且 XDG data/state 根仍不存在；再运行实际导入，得到 `changed` 或 `unchanged`，只建立所需的 data SQLite 文件与写锁；运行 export 后得到确定性 `LibraryExportData` 文件。重复导入不重写数据库；混入无效条目、重复 JSON 键、超限输入、非常规 input、重复 canonical source 或 alias 冲突的批次不产生部分条目或持久写入。首次 import 的 commit 前注入失败后 data/state 根恢复为 absent；commit 后 sync 失败不报告成功或 absence。export 拒绝 database generation/lock target；rename 前输出失败保留旧 target，而 rename 后父目录 sync 失败返回错误且新 target 可能已发布。对损坏数据库的 import/export 返回带路径、空 P2 已知恢复集合和 `database-corruption-v1` 的 `database_corrupt`，并保持原文件及持久状态不变。

任何 P2 已接受的完整 Library 都能导出为不超过 67,108,864 bytes 的 deterministic `LibraryExportData`，随后由同一二进制重新 import；试图通过多次 individually valid import 累积超过该 bound 的 batch 在 mutation/plan result 前以 `validation_failed` 的 `library_portable_document_bytes` constraint 失败。首次 import 在 lock 内发现另一 importer 已发布 database 时，以同一 document 重新规划 existing state 并正常序列化；staging basename 在 SQLite open 前后都必须绑定到 held file，export 最终 rename 失败清理原 staging 与 publication link 而不触碰未知 replacement。

## Design and Architecture Inputs


`ARCHITECTURE.md` 要求 `skilload-core` 保持可复用 domain/application/ports/adapters 分层，CLI 只负责参数、调度与投影，产品变更由应用服务经显式端口提交。Library 元数据是 durable SQLite 的所有者，外部 Skill 字节、Trust、workspace 文件和缓存不是本交付数据库的替代或副本。有效的 config/data/state/cache application root 必须继续通过现有 `StateRootResolver` 同时解析、检查分离和在写前重新验证。

`docs/design-docs/application-and-persistence.md` 已指定 `data/skilload.db` 为 durable 数据库、查询缺失状态时返回内存空视图、写入仅在输入验证达到持久阶段后创建数据库、以及 Library export 不携带本机数据库行 ID 或操作时间。本交付采用该方向，但只创建 v1 的 `schema_info`、`state_revision`、`library_entries` 和 `library_tags` 最小表；不得假装 Trust、global、profile、workspace、owned link、confirmation token 或 FTS 已有真实业务所有者。

`docs/design-docs/cli-json-and-release.md` 规定每个已注册叶子只映射一个应用请求，CLI 不自行编排仓库调用；本分支已将可移植传输参数澄清为 `--input <PATH>`、`--output <PATH>`，使文件中只有可导入数据，而命令结果仍保留正常 API-v2 信封。`docs/design-docs/application-and-persistence.md` 还要求 P2 以 no-follow、nonblocking input descriptor 维持 scanner resource bound、以 staging database 避免首次失败发布 partial state，并区分 rename 前与 rename 后的 export sync failure。`docs/references/rust-sqlite-unicode-library-foundation.md` 记录了本交付的依赖事实：使用无默认特性的 `rusqlite 0.40.2` 加 `bundled`，以及精确 `unicode-normalization =0.1.23`；后者的表是 Unicode 15.1.0，而当前较新版本是 Unicode 17.0.0，不能使用。

本轮 `review` 内 ordinary remediation 继续遵守这些输入：完整 portable document 的 encoder/byte limit 位于 domain，SQLite adapter 在 global lock 内对刚出现的 database 重规划，并在首个 staging SQLite connection 的任何 SQL 前后比较 held inode；transfer adapter 则为 final rename failure 分别清理 identity-matched staging 与 publication link。它们只修复已写明的 P2 atomic transfer、并发和可移植闭环，不触发 review-to-active 逆向事务。

## Purpose / Big Picture


当前二进制只提供 configuration。完成本交付后，用户可以将另一台机器上产生的可移植 Library 元数据文件安全地预演和导入，并在本机生成稳定的可移植备份。导入不会把文件中的任何来源视为已 Trust，也不会下载、缓存、部署或执行外部内容；随后 export 仍能重建同样的版本化文件。

这个范围刻意小于完整 Library。它先提供真实的持久化用户价值和可验证的离线安全边界，再让后续交付在同一真实数据库和 domain 模型上增加 Library 查询、搜索、直接元数据变更和在线 add，而不是提前暴露假命令或用临时 JSON 文件充当数据库。

## Progress


- [x] (2026-08-19 02:15Z) 在干净、已同步的 `main` 上完成 mise、GitHub 鉴权、远端抓取、Plan 状态、产品行为和当前 P1 实现核对；`main` 的 HEAD 是已合并 PR #2 的 `39c2fb88d1cd8eea6ee340efecfd64f5be0febd9`。
- [x] (2026-08-19 02:15Z) 选择 `PLAN-0003`、直接依赖 `PLAN-0002`、文件 slug `p2-library-portable-import-export` 和分支 `codex/p2-library-portable-import-export`；没有开放 PR 或同名远端分支可复用。
- [x] (2026-08-19 02:15Z) 记录 SQLite/Unicode 依赖证据，并在产品/CLI 设计中补充 Revision-1 文件传输接口澄清；尚未改动任何运行时代码。
- [x] (2026-08-19 03:05Z) 处理 PR #3 的规划评审：将新增的文件传输和 alias-error 语义从 Revision 1 提升为 `SKL-LIB-009`/`SKL-LIB-010` Revision 2，规定 `internal_duplicate` 的 Library alias 字段语义，并补足 P2 数据库损坏诊断；尚未改动任何运行时代码。
- [x] (2026-08-19 03:09Z) 发现 `database-recovery.md` 仍以已废止的隐式 `library export --json`/`result.data` 表述恢复导出；已同步为显式 `--output <recovery-directory>/library-export.json`，使 recovery procedure 与 Revision 2 文件契约一致。
- [x] (2026-08-19 04:30Z) 处理 PR #3 的第二轮规划评审：将 export target collision、rename 后 sync failure、非常规 import input、首次 persistence failure 与同 batch canonical source duplicate 纳入 `SKL-LIB-009`/`SKL-LIB-010` Revision 3；恢复 database recovery 的第 2 节标题，更新 API catalog、持久化设计和 Review Conversation Log；未改动任何运行时代码。
- [x] (2026-08-19 04:43Z) 已将第二轮 PR #3 评审的六个有效问题以提交 `e7467f70d55cc48548f4c17e4df067d529719e34` 推送，逐一回复并关闭全部内联线程；本 Review Conversation Log 已记录每个 reply URL 和验证证据，计划仍为 `plan`、PR 仍为 Draft。
- [x] (2026-08-19 05:05Z) 已完成 `execute-exec-plan` 预检：工作树干净，当前/远端分支与 Draft PR #3 的 `6bb4230a103d47a5b165d831672df7c8f3fb6d12` 一致，`PLAN-0002` 已在 `origin/main` 的 `completed/` 中；本 Plan 已进入 `active`，开始里程碑 1。
- [x] (2026-08-19 05:44Z) 完成里程碑 1：锁定 `rusqlite 0.40.2` bundled、精确 `unicode-normalization =0.1.23` 与 `libc =0.2.189`；将带许可证的 Unicode 15.1.0 `CaseFolding.txt`/`PropList.txt` 纳入版本控制，build script 生成 C/F folding 与 `White_Space` 表；实现 portable source、resolved evidence、metadata 与 tag domain。
- [x] (2026-08-19 05:44Z) 完成里程碑 2：实现 no-follow/nonblocking/fstat input gate、六个非模型 JSON 上限与 duplicate-key scanner；实现共享两秒 database lock、v1 SQLite repository、first-import staging publish、conflict rollback、empty read/dry-run、deterministic export、protected target rejection 与 corruption diagnostics。
- [x] (2026-08-19 05:44Z) 完成里程碑 3：`Application` 组合 configuration/Library ports；CLI 仅注册 `library import --input <PATH> [--dry-run]` 与 `library export --output <PATH>`，并投影 API-v1 success/error 与 terminal-safe human results。
- [x] (2026-08-19 05:44Z) 完成里程碑 4 的文档同步：产品 status、architecture、persistence/CLI design 与 SQLite/Unicode reference 已反映 P2 的实际范围；未改变 `SKL-LIB-009`/`SKL-LIB-010` Revision 3 正文。
- [x] (2026-08-19 05:52Z) 完成完整运行时验证：`cargo fmt --all --check`、workspace Clippy `-D warnings`、workspace all-features locked tests（6、12、41 个测试通过）与 workspace build 均通过；实际 CLI smoke 证明 dry-run 无状态、commit/export 成功、isolated round trip 字节相同。
- [x] (2026-08-19 05:52Z) 已 staged 全部 34 个交付文件并完整检查 diff；`git diff --cached --check` 通过，包含 vendored Unicode 输入在内的每个变更均已检查。
- [x] (2026-08-19 05:55Z) 实现提交 `4c6a6919921cabcbc29b11cfa255466993ad2adf` 已推送；local、origin branch 与 Draft PR #3 head 相同，PR 仍为 Draft。
- [x] (2026-08-19 05:57Z) 已运行 `gh pr ready https://github.com/bootids/skilload/pull/3`，随后观察到 `isDraft: false` 与 `headRefOid: 47f22f8a1687d5e46b9d787503565e1badad141a`；该 SHA 等于已推送的 implementation/active-Plan HEAD。首次 review-state commit `b30afe3aa7a772f1ccf1885eb041006528f10c24` 推送后曾再次确认 GitHub/repository head 一致，本 Plan 保持 `review`。
- [x] (2026-08-19 06:42Z) 已在 review 状态修复 PR #3 的七项实现反馈：首次 database publish 使用 no-clobber、首次 lock failure 清理、SQLite row decode 损坏分类，以及 portable source 的 repository 标点、已验证名称、Git path 和 Git ref 约束；修复与 preliminary review log 已由 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 推送，local/upstream/PR ready head 已核对为该 SHA。focused source（5）/SQLite（9）测试、workspace all-features locked tests（6、12、46）、格式、Clippy `-D warnings` 与 workspace build 均通过；待逐线程回复。
- [x] (2026-08-19 06:51Z) 已在最终全量会话复读中核对 16 个 review thread 全部为 resolved；本轮七个实际问题均有 GitHub 回复和 `thread resolved: true`。source-path/ref 的初次批量 reply 请求超时后继续写入，重试产生了两条字节相同的回复；两条 URL 均已如实记录，未删除审计记录。
- [x] (2026-08-19 07:58Z) 已在 review 状态完成新增七项反馈的代码、测试与文档修订：安全 descriptor-relative export publish、既有 SQLite 文件 identity binding、缺失 schema 列与 API-v1 UInt corruption 分类、目录 identity cleanup、state revision 上界和 human import 来源列表。focused SQLite（14）、portable transfer（9）与 human（3 unit、1 integration）测试通过；workspace fmt check、Clippy `-D warnings`、all-features locked test（7、12、52）和 build 通过；实际 dry-run CLI 输出 added canonical source，`git diff --check` 通过。待创建并推送 preliminary review commit。

- [x] (2026-08-19 08:03Z) 新增七项 review remediation 已由 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 推送；逐一回复后，PR #3 的 23 个 inline thread 均为 resolved。首次七线程批量写入在 30 秒客户端超时后仍继续，human/revision 两项各留下两条字节相同的回复；两个 URL 均已记录，未删除审计证据。最终 Review Conversation Log reconciliation 已由 `a244195` 文档提交推送。

- [x] (2026-08-19 08:56Z) 已完整读取 PR #3 的 4 条 top-level 评论、32 个 review body 和 32 个 inline thread；其中 23 个既有 ledger source 仍与 GitHub 的 resolved state 一致，新增 9 个未解决实际问题已逐项登记。
- [x] (2026-08-19 08:56Z) 发现 `agent_input_limit_exceeded` 被错误用于 Library import 的 six-limit scanner，而 API-v1 明确将该 code 保留给 Agent project-input，并禁止为不同条件复用 code；已将 ready PR 恢复为 Draft，待 active rework。
- [x] (2026-08-19 09:00Z) 人类已选择 API-v2 并授权 active rework：新增独立 `library_input_limit_exceeded` code，保留 `LimitDetails` 的 measured/allowed 字段；不保留 API-v1 dual-output mode。
- [x] (2026-08-19 10:17Z) 已实现九项 active remediation：API-v2 full producer cutover 与独立 Library limit code；repository display identity；partial directory cleanup；lock/data-directory/staging descriptor identity binding；schema cardinality、tag comparison-key corruption 诊断；以及 export native path 的 `PathValue` validation projection。
- [x] (2026-08-19 10:17Z) 已通过 focused core（61 tests）和 CLI（9 unit、12 integration）测试，以及 `cargo fmt --all --check`、workspace Clippy `-D warnings`、workspace all-features locked tests、build；隔离 CLI smoke 验证 API-v2 dry-run/import/export 和 `library_input_limit_exceeded` details。
- [x] (2026-08-19 10:21Z) 已检查完整 staged diff、`git diff --check`，并将代码、测试、产品/API/设计/reference 文档和 preliminary review ledger 以 `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 推送；local/upstream/Draft PR #3 head 已核对为同一 SHA。
- [x] (2026-08-19 10:23Z) 已执行 `gh pr ready https://github.com/bootids/skilload/pull/3`，确认 `isDraft: false`、`headRefOid: 73f5634a5b9871bb635f5bf8c4fd36ea81bee816` 与已推送 active Plan HEAD 相同；本文件随 review-state commit 移入 `review/`，随后必须重新读取完整 PR 会话并逐源回复/关闭九个线程。
- [x] (2026-08-19 10:32Z) 已重新读取完整 PR 会话：4 条 top-level trigger、41 个 review body 和 32 个 inline thread 没有新增独立问题；全部 32 个 thread 为 resolved，九个新 source 均已回复、关闭并写入 Review Conversation Log。
- [x] (2026-08-19 10:36Z) 最终 ledger commit 推送后重新读取 PR 会话，发现六个新的 inline review 问题；均是既有 P2 durable schema、staging identity、portable source 或 API-v2 文档边界内的 ordinary fix，已记录为 open，不执行 review-to-active 逆向事务。
- [x] (2026-08-19 10:38Z) 已在 review 状态完成六项新增 remediation：empty tags schema/state revision singleton corruption、export API-v2 wording、first staging inode binding、100-byte repository bound 和 immutable commit equality；新增 focused regressions、完整 workspace gates 和 immutable-source CLI smoke 均通过。
- [x] (2026-08-19 10:43Z) 已检查完整 staged diff、`git diff --check`，并将六项 fix、产品/设计/reference 文档和 preliminary review ledger 以 `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 推送；local/upstream/ready PR #3 head 已核对为同一 SHA。
- [x] (2026-08-19 10:48Z) 已重新读取第二轮完整会话：4 条 top-level trigger、48 个 review body 与 38 个 inline thread 无新增独立问题；全部 38 个 thread 已 resolved，六个新 source 均已回复、关闭并同步到 Review Conversation Log。
- [x] (2026-08-19 11:43Z) 已在 `review` 状态登记并完成九项新增 ordinary remediation：staging publish 改为 descriptor-relative `linkat` publication link、首次 lock clone failure rollback、lossless `InvalidState.path`、human conflict projection、existing database final sync identity、sidecar cleanup、SQLite bounded busy、以及第 129 byte number stop。代码与 preliminary ledger 已由 `2e0a46efb308c18546ff6855ac081818fa416088` 推送，local/PR ready head 已核对为同一 SHA；focused portable（12）、SQLite（26）、CLI（11 unit、12 integration）测试、workspace fmt/Clippy/all-features locked tests/build/`git diff --check` 和隔离实际 CLI import/conflict smoke 均通过，待逐线程 reply/resolve。
- [x] (2026-08-19 11:59Z) 已重新获取完整会话，9 个 target thread 均有本次 code SHA、验证和具体处理说明的 GitHub reply，随后均确认 `isResolved: true`；47 个 inline thread 全部 resolved，新增的 9 个空 review body 不含问题。待提交并推送最终 Review Conversation Log reconciliation。

- [x] (2026-08-19 12:18Z) 已完整读取 PR #3 的 6 条 top-level 触发评论、8 个非空自动 review body 和 51 个 inline thread；前两类没有独立问题，47 个既有 source 仍 resolved，4 个新增未解决内联问题均已以 source、路径、文本、状态与初始 disposition 登记到 Review Conversation Log。
- [x] (2026-08-19 12:42Z) 四项 remediation 的代码、回归测试、产品/设计同步和 preliminary Review Conversation Log 已由 `e8a025208e23e6feac7671714e8657f2e789cdcd` 推送；local、upstream 与 open/ready PR #3 的 `headRefOid` 均为该 SHA。五个直接回归以及 `cargo fmt --all --check`、workspace Clippy `-D warnings`、all-features locked tests（77 个 core tests）和 build 全部通过；待逐线程回复并关闭。
- [ ] 收到明确人类合并授权后，完成预检、评审会话记录、completed 事务、必要检查、合并、默认分支更新和本地交付分支清理。

## Surprises & Discoveries


- Observation: 当前 `PLAN-0002` 只实现配置，核心 crate 没有 SQLite、Library domain/port/adapter 或 Library CLI 子命令；当前 `Application::new` 仅注入 `ConfigurationStore`。
  Evidence: `crates/skilload-core/src/application/configuration.rs` 的 `Application` 只有 `configuration_store` 字段，`crates/skilload-cli/src/args.rs` 只注册 `Config`。
- Observation: `unicode-normalization` 的当前稳定版不能满足产品固定的 Unicode 数据版本。
  Evidence: docs.rs 的 `unicode-normalization 0.1.25` 生成表声明 `UNICODE_VERSION = (17, 0, 0)`，而 `0.1.23` 声明 `(15, 1, 0)`。
- Observation: `rusqlite 0.40.2` 的 bundled SQLite 构建明确打开 FTS5。
  Evidence: `libsqlite3-sys` 的 v0.40.2 对应构建脚本传入 `-DSQLITE_ENABLE_FTS5`；本交付仍不会注册 search 叶子或把该能力误报为 `SKL-LIB-004` 完成。
- Observation: `docs/product-specs/database-recovery.md` 仍引用隐式 `library export --json`，并将 API envelope 的 `result.data` 视为可导入文档。
  Evidence: 该文件第 28、34 行与 Revision 2 的 `SKL-LIB-009` 文件输出边界冲突；本次规划修订将其改为保存显式 output 文件。
- Observation: 同目录 atomic rename 已发布新文件后，父目录 `fsync` 仍可能返回错误，无法同时保证“错误”与“旧 target 必然保留”。
  Evidence: 现有 `crates/skilload-core/src/adapters/configuration.rs` 先在第 215 行执行 `staging.persist`，后在第 223–232 行同步目录；后者失败不能撤销已发生的 rename。
- Observation: `symlink_metadata` 后直接 path-based 打开不能单独保证 untrusted import 的资源有界性，因为目标可在检查后替换为 FIFO。
  Evidence: `crates/skilload-core/src/adapters/configuration.rs` 已使用 `OpenOptionsExt`，而 `ARCHITECTURE.md` cross-cutting invariant 1 要求 Library import JSON 在 unrestricted staging/model 前受资源限制；P2 需要 no-follow、nonblocking descriptor 与 `fstat` 的双重检查。
- Observation: 新增依赖首次不在 lockfile 时，计划中的 `cargo update -p rusqlite -p unicode-normalization -p libc` 以 package ID 未匹配失败。
  Evidence: 先运行 `mise exec -- cargo check -p skilload-core` 仅将 13 个新依赖解析进 lockfile，随后同一 `cargo update -p` 命令成功并显示 `Locking 0 packages`；没有升级 P1 已锁定依赖。
- Observation: `tempfile 3.27.0` 的 `NamedTempFile::persist` 是可替换发布，而同版本的 `persist_noclobber` 提供 destination-exists failure。
  Evidence: 本机 `tempfile-3.27.0/src/file/mod.rs` 分别公开这两个 API；`first_import_does_not_replace_a_database_created_during_publish` 在 absent check 后创建外部 database，断言它保持字节不变且 import 返回 `database_identity_drift`。
- Observation: portable import 直接反序列化 `SourceIdentity`/`ResolvedSkill`，因此必须在 domain constructor 重放原始 source identity、root-name、normalized path 和 Git ref 约束。
  Evidence: `SourceIdentity::deserialize` 和 `ResolvedSkill::deserialize` 都调用各自 `new`；`docs/product-specs/source-and-trust.md` 的 `SKL-SRC-002`/`SKL-SRC-007` 规定同一 canonical identity 与名称关系。
- Observation: 七个 GitHub reply/resolve 的批量请求在 30 秒超时后仍继续执行，导致 source-path 与 source-ref 线程各出现两条相同回复。
  Evidence: 最终 `list --all` 显示 `PRRT_kwDOT7YN2s6aXLAs` 的 `discussion_r3810692503`/`discussion_r3810696527` 与 `PRRT_kwDOT7YN2s6aXLAy` 的 `discussion_r3810693274`/`discussion_r3810698817` 内容字节相同，两个 thread 均为 resolved。
- Observation: `tempfile::NamedTempFile::persist` 只能接收路径，不能将 publish 绑定到已验证的父目录 descriptor；本 crate 又禁止 unsafe code，不能直接调用 raw `libc::renameat`。
  Evidence: `tempfile 3.27.0` 的 `persist` API 接收 destination path；`rustix 1.1.4/src/fs/at.rs` 的安全 `renameat` 接收 `AsFd` 目录 handle 与相对文件名。



- Observation: review 发现初始实现将 Library scanner ceiling 复用为 `agent_input_limit_exceeded`，但该 API-v1 code 只表示 `agent-project-input-v1`；人类选择 API-v2 后，rework 将其替换为专用 `AppError::LibraryInputLimit` 和 `library_input_limit_exceeded`。
  Evidence: 2026-08-19 review source `PRRC_kwDOT7YN2s7jLbvB`；当前 `crates/skilload-core/src/error.rs`、`crates/skilload-cli/src/json.rs` 和 `docs/product-specs/api-v2.md` 均使用独立 code。


- Observation: `rustix 1.1.4` 同时提供 macOS/Linux 的 descriptor-relative `renameat_with(..., RenameFlags::NOREPLACE)` 与 `fstat`/`statat(..., SYMLINK_NOFOLLOW)`，可分别维持首次 database no-clobber 和检测 export staging identity drift。
  Evidence: 锁定 crate 的 `src/fs/at.rs`、`src/fs/fd.rs` 和 platform `RenameFlags` 定义；新增 `first_import_rejects_a_replaced_data_directory_before_publish` 与 `export_reports_staging_replacement_after_identity_check` 回归测试。

- Observation: SQLite exclusive contention 在 `singleton_i64` 的 `prepare/query/next` error mapping 被无条件降级为 `database_corrupt`，因此即使 `database_error` 已识别 busy，export 仍错误投影。
  Evidence: `sqlite_contention_returns_a_bounded_busy_error` 初次返回 `DatabaseCorrupt`；改为将这些 SQLite errors 交给 `database_error` 后，在两秒后返回 `Busy { lock_domain: "database", waited_ms: 2000 }`。
- Observation: POSIX `renameat` 没有 source-FD 形态；macOS 试验将 unlinked held FD 通过 `/dev/fd/<N>` hard-link 回目录返回 `EPERM`，但 `rustix::fs::linkat` 可以从已验证的 held staging entry 建立第二个同 inode publication link。
  Evidence: 本机 `/dev/fd` link experiment 返回 `Operation not permitted`；`portable_library` 与 `sqlite_library` regressions 通过 descriptor-relative `linkat` publish path。

- Observation: `SKL-LIB-008` 的 note 同时受 4,096 Unicode scalar 与 16,384 UTF-8 byte 限制；使用一个四字节 scalar 重复 4,096 次即可得到单条有效的最大字节 note，4,097 个此类 entry 的 note bytes 已超过 P2 import ceiling。
  Evidence: `domain::library::tests::transfer_encoding_rejects_valid_metadata_beyond_the_import_ceiling` 构造该有效 fixture，并在 1.43 秒内确认 shared deterministic encoder 返回 `library_portable_document_bytes`。

## Decision Log


- Decision: 将 `SKL-LIB-009` 与 `SKL-LIB-010` 作为单独的 P2 可交付范围，不包含 Library add/list/search/get 或直接元数据命令。
  Rationale: 导入、dry-run、原子持久化和导出形成用户可观察的完整离线闭环；加入尚无真实查询或网络解析支撑的其他叶子会违反无占位命令约束。
  Date/Author: 2026-08-19 / Codex
- Decision: Revision-1 传输使用显式 `--input <PATH>` 与 `--output <PATH>`，不引入隐式 stdin/stdout 格式或第二种 API 信封。
  Rationale: 文件内容可直接成为下一次 import 输入，且命令的 JSON 信封仍可独立报告 outcome；原子输出也避免半写入的便携备份。
  Date/Author: 2026-08-19 / Codex
- Decision: SQLite 使用 `rusqlite 0.40.2`、`default-features = false`、`bundled`；Unicode NFC 使用精确 `unicode-normalization =0.1.23`，大小写折叠与 White_Space 使用仓库内由 Unicode 15.1.0 数据离线生成的表。
  Rationale: 该组合把 SQLite/FTS5 与 Unicode 数据从宿主和未来 SemVer 解析中固定下来，并避免在构建或运行时网络下载。
  Date/Author: 2026-08-19 / Codex
- Decision: P2 不提供可选的 import replace 模式，`LibraryImportData.updated` 始终为空；已有 canonical source 默认 kept。
  Rationale: `SKL-LIB-010` 只允许而不要求 replace。先实现必需的无覆盖导入可避免在尚无直接 metadata 命令时引入不必要的替换语义；未来若需要 replace，必须由后续计划以同一原子模型加入。
  Date/Author: 2026-08-19 / Codex
- Decision: 导入预验证实现为有界字节扫描器，而非把 `serde_json` 反序列化当作资源边界。
  Rationale: 产品要求在完整模型或 `ImportPlan` 之前检查每一种限制和重复键。扫描器只保留受限的语法/对象键状态与最多 67,108,864 字节的已验证输入；通过后才允许严格 schema 反序列化。
  Date/Author: 2026-08-19 / Codex

- Decision: 将 `SKL-LIB-009` 与 `SKL-LIB-010` 的文件传输和 alias-error 语义定为 Revision 2，而非 Revision 1。
  Rationale: `origin/main` 的 Revision 1 没有强制 `--input`/`--output`、文件与命令结果分离、原子输出或 alias error 的可观察契约；这些语义必须以可追踪的行为修订交付。
  Date/Author: 2026-08-19 / Codex
- Decision: Library alias 冲突使用现有 `Conflict.kind: "internal_duplicate"` 的专用字段约束，不新增 API-v1 枚举值。
  Rationale: 该值表达 durable Library 唯一 alias 的重复，同时固定冲突 alias 与被拒绝 source；复用既有 API-v1 union 避免以新枚举值破坏既定版本约束。
  Date/Author: 2026-08-19 / Codex
- Decision: P2 在识别到 `data/skilload.db` 损坏时返回 `database_corrupt`，但不在本交付中实现备份、导出索引、迁移或 reset。
  Rationale: `SKL-OPS-004` 与 API catalog 已要求可恢复诊断；P2 尚未创建或记录这些恢复资产，因此必须如实返回空集合并给出规范 recovery procedure，而不是伪造路径或降级为一般状态错误。
  Date/Author: 2026-08-19 / Codex
- Decision: 数据库恢复过程的可移植 Library evidence 使用 `library export --output <recovery-directory>/library-export.json --json`，而不是从命令 API envelope 提取数据。
  Rationale: Revision 2 明确 file document 与命令结果分离；recovery directory 已在规范过程的第一步建立，因而能保留可直接再导入的原子输出与独立操作证据。
  Date/Author: 2026-08-19 / Codex
- Decision: 将第二轮评审发现的 export path/failure、input file-type、first-import cleanup 和 canonical-source duplicate 语义发布为 `SKL-LIB-009`/`SKL-LIB-010` Revision 3。
  Rationale: 这些规则新增或收紧用户可观察的成功、失败和持久状态语义；按仓库 revision 规则不能把它们伪装为 Revision 2 的文字澄清。
  Date/Author: 2026-08-19 / Codex
- Decision: 同一 import batch 中第二个相同 canonical source 一律作为 `internal_duplicate` conflict 拒绝，不静默去重或按输入顺序选择 metadata。
  Rationale: 拒绝消除了 SQLite primary-key failure 和不可解释的 winner，同时保留已有 `ConflictDetails` union；null `name` 与被拒绝 source 区分它和 alias 冲突。
  Date/Author: 2026-08-19 / Codex
- Decision: P2 使用 no-follow、nonblocking descriptor 加 `fstat` 读取 import input，并对原本不存在的 database 使用同目录 staging publish。
  Rationale: 前者阻止 FIFO/device/race 在 scanner 前阻塞；后者使首次 commit 前失败没有 live database，且只需谨慎清理由调用创建的 lock 和空目录。
  Date/Author: 2026-08-19 / Codex
- Decision: P2 的 v1 database 使用 `journal_mode = DELETE`，但 export 仍拒绝 live database、WAL、SHM 和 database-lock target。
  Rationale: DELETE mode 避免首次 staging database 遗留 WAL/SHM sidecar；target guard 仍覆盖已存在 generation 与未来 journal-mode 变化，不能以当前实现细节缩小 Revision 3 的安全边界。
  Date/Author: 2026-08-19 / Codex
- Decision: `LibraryRepository::import` 在 Rust domain 中返回 `LibraryImportOperation`，而非裸 `(MutationOutcome, LibraryImportResult)` tuple。
  Rationale: 该结构以一个 presentation-neutral value 保持同一 outcome/data 所有权，Application/CLI 不会分离或重算任一值；对 Product Baseline 的 API-v1 data 和 outcome 语义没有变化。
  Date/Author: 2026-08-19 / Codex
- Decision: 将本轮七项反馈作为 `review` 内的普通缺陷修复，不执行 review-to-active 逆向事务。
  Rationale: 每一项都实现既有 `SKL-LIB-010` 的无 partial state/未覆盖 durable state 要求，或 Product Baseline 已明确复用的 `SKL-SRC-002`/`SKL-SRC-007` source evidence 约束；没有新增产品行为、命令或 acceptance scope。
  Date/Author: 2026-08-19 / Codex
- Decision: 新 database publication 使用 `persist_noclobber`，且 restrictive lock helper 仅在 `create_new` 成功时记录可删除的 identity。
  Rationale: no-clobber 保持 race 中的外部 authoritative database；create-new 的 file identity 让 commit 前 cleanup 只删除本调用证实创建的 lock，而不把 prior absence 检查误当成 ownership proof。
  Date/Author: 2026-08-19 / Codex
- Decision: review remediation 将 `rustix =1.1.4`（`fs`）作为直接 workspace dependency，并以安全 `renameat` 在持有的 export 父目录 descriptor 内发布 staging。
  Rationale: 这消除最终 output-parent validation 与 path-based publish 之间的祖先替换窗口，同时保持 repository 的禁止 unsafe-code 规则；锁文件已固定相同版本，直接依赖只声明实际使用的安全 API。
  Date/Author: 2026-08-19 / Codex



- Decision: 因第九轮 review 发现的 API-v1 error-code 冲突，将 PR #3 从 ready `review` 逆向恢复为 Draft `active`，不在 review 状态伪装为普通修复。
  Rationale: `SKL-LIB-010` 要求 import ceiling error 保留 measured/allowed 的 `LimitDetails`，而 `agent_input_limit_exceeded` 已被 API-v1 专用于不同的 Agent 输入条件；选择新 major API version 或其他产品契约属于需要人类确认的重大行为决定。其余八项是既有 P2 boundary 内的实施缺口，但必须与该 rework 一起通过 active Plan 生命周期完成。
  Date/Author: 2026-08-19 / Codex


- Decision: API-v2 是唯一的 current JSON producer；它新增 `library_input_limit_exceeded` → `LimitDetails`，API-v1 仅作为历史契约保留。
  Rationale: 人类在 review 中明确选择独立 API major version。全 CLI 一致发出 `api_version: 2` 避免同一二进制按 operation 或 outcome 混用 major version；Version 1 的已有记录仍留在规格中而不被错误重解释。
  Date/Author: 2026-08-19 / 用户确认、Codex 记录


- Decision: data/staging publication 使用 held directory/file descriptor identity，而不是在 final validation 后再次信任可替换路径名。
  Rationale: `renameat_with(NOREPLACE)` 将首次 database publish 限制在已验证 data-directory descriptor；export 在 rename 前后将 held staging FD 与 parent-relative entry 比较。任何检测到的 drift 返回错误且不报告成功，cleanup 只删除已证实仍由本调用持有的 entry。
  Date/Author: 2026-08-19 / Codex

- Decision: export 和 first-import publish 在最终 hook 后不直接 rename 初始 staging name；先通过 held parent descriptor 以 `linkat` 建立并重验随机 publication link，再 rename 该 link。
  Rationale: 新 link 将 publish source 绑定到 held staging inode；若 hook 或初始 source name 已被替换，pre-link/relink identity check 在 destination mutation 前失败。该方案只使用已锁定 rustix 的安全 API，保持 macOS/Linux shared implementation，且成功后按 inode 删除原 staging link。
  Date/Author: 2026-08-19 / Codex

- Decision: 将完整 portable document 不超过 input ceiling 视为 Revision 3/4 的既有 round-trip/唯一 transfer-format 语义，而非新增行为 revision。
  Rationale: `SKL-LIB-009` 已规定 export 产生唯一可移植文档，`SKL-LIB-010` 已对该文档规定严格 byte ceiling；此前多次 import 能产生自有 export 无法再读的 state 是实现遗漏。此次以 `validation_failed` 的既有 `ValidationDetails` constraint 表达，不增加 API code 或字段。
  Date/Author: 2026-08-19 / Codex
- Decision: first-import 在取得 database lock 后发现合法 winner 已发布时，复用 existing-database 的锁内路径重新规划，而不是返回 drift 或释放 lock 后让用户手工重试。
  Rationale: normal concurrent writer 已经在同一 global lock 下完成 durable mutation；基于已验证 document 在当前 baseline 重新规划保持 mutation serialization 和 dry-run/actual plan 的语义。
  Date/Author: 2026-08-19 / Codex
- Decision: 首个 staging SQLite connection 使用 read-write/no-follow/no-create flags，并在 hook 后、配置或 SQL 前再次验证随机 entry 的 held inode。
  Rationale: path replacement 不得令 SQLite 跟随 symlink、创建新文件或在 foreign target 执行 schema/transaction；第二次验证把受控 test replacement 归类为 identity drift，未知 entry 留给其所有者。
  Date/Author: 2026-08-19 / Codex
- Decision: export 最终 rename error 独立按 held inode 清理 publication link 和原 staging entry。
  Rationale: `linkat` 已可能创建两个指向 held inode 的随机名称；仅删除原名称会遗留完整 portable 文件，删除不匹配名称又会破坏 unknown replacement。
  Date/Author: 2026-08-19 / Codex

## Outcomes & Retrospective


P2 implementation 与完整验证已完成，PR #3 已于 2026-08-19 05:57Z 转为 ready for review；ready transaction 的实现头为 `47f22f8a1687d5e46b9d787503565e1badad141a`，GitHub 已返回 `isDraft: false` 与相同 `headRefOid`。P2 提供仅含 portable resolved Library evidence 的 `data/skilload.db`：dry-run/absent export 不创建 XDG roots，实际 import 在所有 scanner/schema/domain/conflict planning 后才 staging/publish，existing canonical source 保持 kept，alias/canonical duplicate 以规定 `internal_duplicate` rollback，export 以稳定顺序写出独立 JSON 文件。`mise exec -- cargo fmt --all --check`、`mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings`、`mise exec -- cargo test --workspace --all-features --locked`（6、12、41 tests passed）和 `mise exec -- cargo build --workspace --all-features --locked` 均已通过；实际 `target/debug/skilload` smoke 在两个隔离 XDG root 中验证 dry-run observed/no-state、changed import、portable-only export 与 byte-identical second import/export。下一步是人类 review 与所需会话处理；只有明确人类 merge 授权才可进入 completed。

首次 review-state commit `b30afe3aa7a772f1ccf1885eb041006528f10c24` 推送后，PR #3 当时仍为 open、ready，`headRefOid`、local HEAD 与 origin branch 一致，且 `docs/exec-plans/review/p2-library-portable-import-export.md` 是唯一的 current Plan copy。

2026-08-19 06:42Z 的 review remediation 已由 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 推送：first-import database publish 改为 `persist_noclobber`、creation identity 从 restrictive lock helper 传给 RAII cleanup guard、SQLite durable row decode 错误映射为 `database_corrupt`，并收紧 portable source 的 repository/name/path/ref 重验证。local、upstream 与 PR #3 ready head 已核对为该 SHA；focused source（5）和 SQLite（9）测试、workspace all-features locked tests（6、12、46）、格式、Clippy 和 build 均通过；GitHub thread 回复于 06:51Z 完成。

2026-08-19 06:51Z 的最终会话核对确认：所有 16 个 inline thread 均为 resolved；本轮七个问题的每个 source 都有相应的 pushed-fix reply。source-path/ref 的批量请求超时留下的重复回复已在 Review Conversation Log 保留两个 URL，未影响修复、验证或 thread 状态。

2026-08-19 07:58Z 的新增 review remediation 已完成本地验收：export 使用持有父目录 descriptor 的安全 `rustix::fs::renameat`，既有数据库不再通过可创建或可跟随的路径打开，缺列/越界 schema 的错误保持 API-v1 可表示的 `database_corrupt`，first-import cleanup 保留 identity-mismatched directory，state revision 在写 entry 前受限递增，human import 输出枚举已计划 source。focused 与 workspace gates 均通过，实际 dry-run CLI smoke 已显示 quoted source；下一步仅为 preliminary commit、push、逐线程回复和关闭。

2026-08-19 08:03Z 的全量会话 reconciliation 确认：3 条 top-level `@codex` 触发评论和 31 个 review bodies 未提出新的独立问题，23 个 inline thread 全部为 resolved；新增七个 source 均有 code commit、验证、GitHub reply URL 和 close state。最终文档 reconciliation commit `a244195` 已推送；随后仍须重新读取会话和 PR head 以确认没有后续漂移。


2026-08-19 08:56Z 的完整 review 重读发现九个新的 inline 问题。八项文件 identity、SQLite corruption、portable evidence 与 typed path 修复均落在既有 P2 基线内；但 Library scanner 上限错误 code 与 API-v1 的强制语义相冲突，无法在不作产品决定的前提下满足既有 `LimitDetails` acceptance。已运行 `gh pr ready https://github.com/bootids/skilload/pull/3 --undo` 并确认 PR 仍 open、`isDraft: true` 且 head 为 `ae76b9fab46ea22147a8ce044a255b07659be2b9`；本 Plan 随本提交回到 `active`，所有新线程保持 open，等待人类 API 决定和重新执行授权。


2026-08-19 10:17Z 的 active rework 已完成本地实现和验收。API-v2 catalog 成为唯一 current producer contract，保留 API-v1 历史定义；六个 Library scanner ceiling 现在投影为 `library_input_limit_exceeded`/`LimitDetails`。九个 review defect 均有回归覆盖：data/lock/staging descriptor races、partial directory rollback、schema/tag durable corruption、source display binding 和 lossless error path。下一步是审阅完整 diff、推送 preliminary commit，再按原 PR review workflow 回复并关闭线程。


2026-08-19 10:23Z 已完成 active-to-ready 的 GitHub 事务：PR #3 为 open、ready，`headRefOid` 等于完整 remediation/active-Plan evidence commit `73f5634a5b9871bb635f5bf8c4fd36ea81bee816`。本 review-state commit 只移动本 Plan 并记录 ready evidence；九个新 review source 仍保持 open，下一步由 `address-pr-threads` 在 review 状态重新获取会话、回复并关闭。

2026-08-19 10:32Z 的最终 conversation reconciliation 确认：4 条 top-level `@codex` trigger 和 5 个非空 bot review body 只触发 inline review，不含新的独立问题；32 个 inline thread 均为 resolved，Plan 的 32 个 source heading 与 GitHub thread 一一对应。九项本轮 remediation 均引用 code commit `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c`、验证和 reply URL；本条 review documentation commit 推送后将再次核对 PR head。

2026-08-19 10:36Z 在 final ledger commit 推送后，自动 review 对最新 head 新增六个 inline source。它们要求空 Library 的 tags schema probe、state revision singleton、export acceptance API-v2 cross-reference、first-import staging inode binding、100-byte repository name 和 commit-intent/resolved-commit equality。所有问题都直接收紧既有 P2 durable/evidence contract，不增加命令、行为 ID 或产品 revision；本 Plan 保持 `review`，先记录 source-complete open ledger 后执行普通 review remediation。

2026-08-19 10:38Z 的第二轮 ordinary review remediation 已完成本地实现。`validate_database` 现在独立 probe empty Library 的 required tags schema 且验证 state revision singleton；first import 与 export 一样在 held staging FD/descriptor-relative entry 上执行 pre/post inode comparison。Source validation 限制 GitHub repository identity 到 100 bytes，并要求 immutable ref SHA 等于 resolved commit；product spec 消除了 export API-v1 遗留文字。core 66 tests、workspace gates 与无状态 immutable-source CLI smoke 均通过，待 preliminary commit/push。

2026-08-19 10:48Z 的第二轮 final reconciliation 确认：4 条 top-level trigger 和 5 个非空 bot review body 没有额外问题；38 个 inline thread 均为 resolved，Plan 的 38 个 source heading 与 GitHub thread 一一对应。六项 ordinary remediation 均记录 code commit `19fe009ac578e8fb6bd1eefc2649eaa1802611bf`、66-test/workspace/CLI evidence 和 reply URL；本条 final documentation commit 推送后将进行最后一次完整核对。

2026-08-19 11:43Z 的第三轮 review remediation 完成：九项新增 source 均属于现有 P2 atomic transfer、API-v2 或 CLI projection 基线。export 与 first import 现在将已验证 held staging inode link 到随机 publication entry 后才 rename；first lock clone、sidecar rollback、existing database final sync 和 SQLite contention 都有 deterministic regression。API-v2 `InvalidStateDetails.path` 是 `SKL-CLI-012` 允许的 optional `PathValue`，human conflict output 与 JSON `ConflictDetails` 同时保留 actionable alias/source。focused tests、完整 workspace gates 和实际 CLI smoke 已通过；下一步是 preliminary commit/push、逐线程 reply/resolve 和最终 ledger reconciliation。

2026-08-19 11:43Z 的 code 与 preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；GitHub PR #3 仍 open、ready，`headRefOid` 与 local HEAD 相同。九个 source 仍保持 open，下一步只处理 GitHub 回复、thread closure 和 final ledger commit。

2026-08-19 11:59Z 的 GitHub reconciliation 确认 47 个 inline thread 均为 resolved。九个本轮 source 的 reply URL、code SHA 和 validation 已写回下列 ledger；新出现的九个 `bootids` review body 均为空，不构成独立问题。待本 Plan 的 final documentation commit 推送后重新读取会话和 PR head，以排除最后漂移。

2026-08-19 12:42Z 的第四轮 remediation 已由 code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 推送，PR #3 仍为 open、ready，`headRefOid`、local 与 upstream 一致。共享 67,108,864-byte deterministic encoder 现在阻止不可重新导入的 aggregate Library；SQLite 在 lock 后重规划 winner 已发布的 database，并在 staging open 的 SQL 前后验证 inode；export 的 rename-error 分支清理 publication link。产品规格与持久化设计说明了既有唯一可移植文档的闭环。下一步只逐 thread 回复、关闭和最终 Review Conversation Log reconciliation。
## Review Conversation Log


### PRRC_kwDOT7YN2s7jExK8 — 数据库损坏诊断

Source: 内联评论 `PRRC_kwDOT7YN2s7jExK8`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809678012)；线程 `PRRT_kwDOT7YN2s6aVL0n`，当前已解决。

Problem: P2 的 SQLite 打开路径只计划返回 `SchemaDetails` 或 `InvalidStateDetails`，没有为已识别的数据库损坏返回 `database_corrupt` 和既有 `DatabaseCorruptDetails` 恢复证据。

Disposition: fixed

Status: resolved

Resolution: 已由 `91e52913477d46682e32ba1fefefc3515ba80b77` 更新本 Plan：P2 对已识别的 SQLite 损坏返回 `database_corrupt` 的 `DatabaseCorruptDetails`，携带数据库 `PathValue`、空 P2 已知 backup/export 集合和 `database-corruption-v1`，并保留原数据库；同步修正恢复过程的显式 export 文件调用。

Evidence: 修订提交 `91e52913477d46682e32ba1fefefc3515ba80b77` 已推送，PR #3 头提交已核对为同一 SHA；文档一致性断言和 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3809773334；thread resolved: true。

### PRRC_kwDOT7YN2s7jExK9 — alias 冲突 API 表示

Source: 内联评论 `PRRC_kwDOT7YN2s7jExK9`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809678013)；线程 `PRRT_kwDOT7YN2s6aVL0o`，当前已解决。

Problem: `SKL-LIB-010` 要求 alias 冲突失败，但 API-v1 未规定 `ConflictDetails` 如何无歧义地表示该 Library 域冲突。

Disposition: fixed

Status: resolved

Resolution: 已由 `91e52913477d46682e32ba1fefefc3515ba80b77` 更新 `docs/product-specs/api-v1.md` 与本 Plan：Library 唯一 alias 冲突使用既有 `internal_duplicate`，`name` 为 alias、`source` 为被拒绝 entry，`agent`/`path` 为 null；无需新增 API-v1 枚举值。

Evidence: 修订提交 `91e52913477d46682e32ba1fefefc3515ba80b77` 已推送，PR #3 头提交已核对为同一 SHA；API catalog/`SKL-LIB-010` 字段一致性断言和 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3809773891；thread resolved: true。

### PRRC_kwDOT7YN2s7jExK_ — 文件传输行为修订

Source: 内联评论 `PRRC_kwDOT7YN2s7jExK_`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809678015)；线程 `PRRT_kwDOT7YN2s6aVL0q`，当前已解决。

Problem: 当前 PR 为 `SKL-LIB-009` 和 `SKL-LIB-010` 新增了强制文件参数、文件/命令结果分离和原子替换等可观察语义，却仍标为 Revision 1。

Disposition: fixed

Status: resolved

Resolution: 已由 `91e52913477d46682e32ba1fefefc3515ba80b77` 将 `docs/product-specs/library.md` 的 `SKL-LIB-009` 与 `SKL-LIB-010` 提升为 Revision 2，并把强制 file options、文件/命令结果分离、原子输出和 alias error 语义纳入正文与 acceptance；本 Plan 的 Product Baseline、进度、决策和后续 status 文字已同步。

Evidence: 修订提交 `91e52913477d46682e32ba1fefefc3515ba80b77` 已推送，PR #3 头提交已核对为同一 SHA；对 `origin/main` Revision 1 的对比、文档一致性断言和 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3809774313；thread resolved: true。

### PRRC_kwDOT7YN2s7jFpZe — export 目标与活动数据库碰撞

Source: 内联评论 `PRRC_kwDOT7YN2s7jFpZe`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809908318)；线程 `PRRT_kwDOT7YN2s6aVw_e`，当前已解决。

Problem: 现有计划允许 `library export --output` 将可移植 JSON 原子替换为活动 `data/skilload.db`、其 WAL/SHM sidecar 或数据库写锁，从而破坏 Library 的权威 SQLite generation。

Disposition: fixed

Status: resolved

Resolution: 提交 `e7467f70d55cc48548f4c17e4df067d529719e34` 已更新 `docs/product-specs/library.md`、`docs/design-docs/application-and-persistence.md` 与本计划：export 在 staging 前拒绝活动 database/WAL/SHM/database lock target，并以 rename 前/后不同的 failure contract 和 fixture 验收；本轮 planning 未改运行时代码。

Evidence: `e7467f70d55cc48548f4c17e4df067d529719e34` 已推送且 PR #3 head 已核对为同一 SHA；一次性文档一致性断言确认 Revision 3 export target/failure contract、设计和 Plan 对齐，`git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810093425；thread resolved: true。

### PRRC_kwDOT7YN2s7jFpZh — import 非常规文件输入

Source: 内联评论 `PRRC_kwDOT7YN2s7jFpZh`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809908321)；线程 `PRRT_kwDOT7YN2s6aVw_g`，当前已解决。

Problem: 当前 scanner 直接读取 `--input`，FIFO、device 或 race 后的非常规文件可在资源上界检查前无限阻塞。

Disposition: fixed

Status: resolved

Resolution: 提交 `e7467f70d55cc48548f4c17e4df067d529719e34` 已更新 `docs/product-specs/library.md`、`docs/design-docs/application-and-persistence.md` 与本计划：import 以 no-follow、nonblocking descriptor 和 `fstat` 证明 regular file、拒绝 symlink/directory/FIFO/socket/device/identity drift，并加入 FIFO/device fixture。

Evidence: `e7467f70d55cc48548f4c17e4df067d529719e34` 已推送且 PR #3 head 已核对为同一 SHA；一次性文档一致性断言确认 Revision 3 regular-input gate、设计和 Plan 对齐，`git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810094616；thread resolved: true。

### PRRC_kwDOT7YN2s7jFpZj — 同一 batch 的 canonical source 重复

Source: 内联评论 `PRRC_kwDOT7YN2s7jFpZj`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809908323)；线程 `PRRT_kwDOT7YN2s6aVw_i`，当前已解决。

Problem: 两个输入 entry 具有同一 `skill.source.canonical` 但没有 alias 冲突时，尚未规定 batch 是拒绝、去重还是选择一项，SQLite 主键与 import result 因而不确定。

Disposition: fixed

Status: resolved

Resolution: 提交 `e7467f70d55cc48548f4c17e4df067d529719e34` 已更新 `docs/product-specs/library.md`、`docs/product-specs/api-v1.md` 与本计划：同一 batch 的后出现 canonical source duplicate 是原子 `internal_duplicate` conflict，使用 `name: null` 和该 entry source；`SKL-LIB-010` 升为 Revision 3 并加入 fixture。

Evidence: `e7467f70d55cc48548f4c17e4df067d529719e34` 已推送且 PR #3 head 已核对为同一 SHA；一次性文档一致性断言确认 API catalog 与 Revision 3 canonical-duplicate shape 对齐，`git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810095767；thread resolved: true。

### PRRC_kwDOT7YN2s7jFpZl — 首次 import 失败遗留状态

Source: 内联评论 `PRRC_kwDOT7YN2s7jFpZl`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809908325)；线程 `PRRT_kwDOT7YN2s6aVw_l`，当前已解决。

Problem: 首次 import 在创建根、lock 或 SQLite 文件后失败时，现有计划只依赖 transaction rollback，可能遗留原先不存在的 durable state。

Disposition: fixed

Status: resolved

Resolution: 提交 `e7467f70d55cc48548f4c17e4df067d529719e34` 已更新 `docs/product-specs/library.md`、`docs/design-docs/application-and-persistence.md` 与本计划：absent database 使用同目录 staging publish，commit 前清理仅由调用创建的 database/sidecar/lock/空目录，commit 后 durability-sync error 不伪称旧状态。

Evidence: `e7467f70d55cc48548f4c17e4df067d529719e34` 已推送且 PR #3 head 已核对为同一 SHA；一次性文档一致性断言确认 Revision 3 first-import staging/cleanup contract、设计和 Plan 对齐，`git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810097033；thread resolved: true。

### PRRC_kwDOT7YN2s7jFpZp — export rename 后同步失败

Source: 内联评论 `PRRC_kwDOT7YN2s7jFpZp`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809908329)；线程 `PRRT_kwDOT7YN2s6aVw_n`，当前已解决。

Problem: 当前计划把 directory `fsync` 失败也表述为保留旧 output，但 rename 已发生时新 output 可能已经发布。

Disposition: fixed

Status: resolved

Resolution: 提交 `e7467f70d55cc48548f4c17e4df067d529719e34` 已更新 `docs/product-specs/library.md`、`docs/design-docs/application-and-persistence.md` 与本计划：rename 前 failure 保留旧 target/无 target，rename 后 parent-sync failure 返回 error 且允许新 target 已发布。

Evidence: `e7467f70d55cc48548f4c17e4df067d529719e34` 已推送且 PR #3 head 已核对为同一 SHA；一次性文档一致性断言确认 Revision 3 rename 前/后 failure contract、设计和 Plan 对齐，`git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810097641；thread resolved: true。

### PRRC_kwDOT7YN2s7jFpZr — recovery salvage 小节标题

Source: 内联评论 `PRRC_kwDOT7YN2s7jFpZr`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3809908331)；线程 `PRRT_kwDOT7YN2s6aVw_p`，当前已解决。

Problem: `database-recovery.md` 从第 1 节直接跳到第 3 节，但 reset 授权仍要求完成第 1 和第 2 节，salvage 阶段不再可识别。

Disposition: fixed

Status: resolved

Resolution: 提交 `e7467f70d55cc48548f4c17e4df067d529719e34` 已更新 `docs/product-specs/database-recovery.md`，在 salvage 段落前恢复 `## 2. Salvage Readable Product Data` 标题；procedure 内容和版本未变。

Evidence: `e7467f70d55cc48548f4c17e4df067d529719e34` 已推送且 PR #3 head 已核对为同一 SHA；一次性 heading/交叉引用断言确认 recovery 的 1/2/3 小节顺序，`git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810098110；thread resolved: true。

### PRRC_kwDOT7YN2s7jHvvZ — 首次数据库发布竞争

Source: 内联评论 `PRRC_kwDOT7YN2s7jHvvZ`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810458585)；线程 `PRRT_kwDOT7YN2s6aXLAS`，当前已解决。

Problem: 首次 import 在 publish 前的 absent 检查与 `NamedTempFile::persist` 之间存在竞争；后创建的 `skilload.db` 会被替换，违反不得覆盖外部权威 generation 的 ownership invariant。

Disposition: fixed

Status: resolved

Resolution: 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已修改 `crates/skilload-core/src/adapters/sqlite_library.rs`，用 `persist_noclobber` 发布首次 database；destination 竞争映射为 `database_identity_drift`，并新增 publish-window race 注入测试。

Evidence: `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local/upstream/PR #3 ready head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked sqlite_library`（9 tests）、workspace all-features locked tests、Clippy 与 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810687979；thread resolved: true。

### PRRC_kwDOT7YN2s7jHvvl — 可移植 repository 标点

Source: 内联评论 `PRRC_kwDOT7YN2s7jHvvl`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810458597)；线程 `PRRT_kwDOT7YN2s6aXLAb`，当前已解决。

Problem: `SourceIdentity` 将 owner 与 repository 共用仅允许连字符的校验，拒绝产品 fixtures 明确允许的 repository `.` 和 `_`，使有效 root Library export 无法重新导入。

Disposition: fixed

Status: resolved

Resolution: 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已修改 `crates/skilload-core/src/domain/source.rs`，分离 canonical owner/repository component 校验并允许 repository 的 `.`/`_`；新增 root repository display 拼写的 portable round-trip test。

Evidence: `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local/upstream/PR #3 ready head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked source`（5 tests）、workspace all-features locked tests、Clippy 与 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810688949；thread resolved: true。

### PRRC_kwDOT7YN2s7jHvvq — SQLite 行类型损坏诊断

Source: 内联评论 `PRRC_kwDOT7YN2s7jHvvq`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810458602)；线程 `PRRT_kwDOT7YN2s6aXLAe`，当前已解决。

Problem: 持久 SQLite schema 或 Library 列的错误 storage type 产生 `rusqlite::Error::InvalidColumnType`，当前落入 `invalid_state` 而不是要求的 `database_corrupt` recovery diagnostic。

Disposition: fixed

Status: resolved

Resolution: 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已修改 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 SQLite error 映射，将 durable row decoding 的列类型、UTF-8、数值及 conversion error 分类为 `database_corrupt`；新增 BLOB column fixture。

Evidence: `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local/upstream/PR #3 ready head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked sqlite_library`（9 tests）、workspace all-features locked tests、Clippy 与 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810689783；thread resolved: true。

### PRRC_kwDOT7YN2s7jHvvu — 已验证名称与来源关系

Source: 内联评论 `PRRC_kwDOT7YN2s7jHvvu`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810458606)；线程 `PRRT_kwDOT7YN2s6aXLAh`，当前已解决。

Problem: portable import 仅检查 `ResolvedSkill.name` 的独立语法，没有强制 `SKL-SRC-007` 的 non-root 最终目录或 root repository display 派生名称关系，因而可持久化永远无法通过来源验证的 evidence。

Disposition: fixed

Status: resolved

Resolution: 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已修改 `crates/skilload-core/src/domain/source.rs`，在构造 `ResolvedSkill` 时比较已验证 name 与 source 的非根末段或 root display 派生 segment；新增 root/non-root mismatch 回归测试。

Evidence: `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local/upstream/PR #3 ready head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked source`（5 tests）、workspace all-features locked tests、Clippy 与 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810690645；thread resolved: true。

### PRRC_kwDOT7YN2s7jHvv2 — 首次 import 锁失败清理

Source: 内联评论 `PRRC_kwDOT7YN2s7jHvv2`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810458614)；线程 `PRRT_kwDOT7YN2s6aXLAn`，当前已解决。

Problem: 首次 import 在获取 `database.lock` 前创建 state/locks 后，锁获取失败会越过既有 cleanup，遗留本调用创建的 state、locks 或 lock 文件。

Disposition: fixed

Status: resolved

Resolution: 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已修改 `crates/skilload-core/src/adapters/configuration.rs` 与 `crates/skilload-core/src/adapters/sqlite_library.rs`：restrictive lock 只在 `create_new` 成功时返回创建 identity，首次 import 在任何目录创建前安装 cleanup guard，并新增锁准备失败回归测试。

Evidence: `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local/upstream/PR #3 ready head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked sqlite_library`（9 tests）、workspace all-features locked tests、Clippy 与 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3810691591；thread resolved: true。

### PRRC_kwDOT7YN2s7jHvv_ — 可移植 source path 限制

Source: 内联评论 `PRRC_kwDOT7YN2s7jHvv_`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810458623)；线程 `PRRT_kwDOT7YN2s6aXLAs`，当前已解决。

Problem: 当前 source path 只拒绝空、绝对和 `.`/`..` segment，仍接受控制字符或 `.git` segment，违反 untrusted portable source 的 normalized Git-path 限制。

Disposition: fixed

Status: resolved

Resolution: 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已修改 `crates/skilload-core/src/domain/source.rs`，拒绝 control byte、反斜线和 `.git` path segment，同时保留 root source 的空 path；新增 hostile path 回归测试。

Evidence: `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local/upstream/PR #3 ready head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked source`（5 tests）、workspace all-features locked tests、Clippy 与 build 通过。

GitHub outcome: 批量请求超时后产生两条内容相同的回复 https://github.com/bootids/skilload/pull/3#discussion_r3810692503 与 https://github.com/bootids/skilload/pull/3#discussion_r3810696527；thread resolved: true。

### PRRC_kwDOT7YN2s7jHvwF — 可移植 branch/tag ref 限制

Source: 内联评论 `PRRC_kwDOT7YN2s7jHvwF`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810458629)；线程 `PRRT_kwDOT7YN2s6aXLAy`，当前已解决。

Problem: 当前 branch/tag suffix 只检查 slash 与 `.`/`..` segment，仍接受 Git 无法解析的 `..`、`.lock`、控制字节和保留 ref 字符。

Disposition: fixed

Status: resolved

Resolution: 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已修改 `crates/skilload-core/src/domain/source.rs`，实现完整 Git ref-name suffix 规则并保留 namespace prefix；新增与 `git check-ref-format` 对齐的 malformed-ref 回归测试。

Evidence: `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local/upstream/PR #3 ready head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked source`（5 tests）、workspace all-features locked tests、Clippy 与 build 通过。

GitHub outcome: 批量请求超时后产生两条内容相同的回复 https://github.com/bootids/skilload/pull/3#discussion_r3810693274 与 https://github.com/bootids/skilload/pull/3#discussion_r3810698817；thread resolved: true。

### PRRC_kwDOT7YN2s7jJWGX — export 父目录替换竞争

Source: 内联评论 `PRRC_kwDOT7YN2s7jJWGX`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810877847)；线程 `PRRT_kwDOT7YN2s6aYP_z`，当前已解决。

Problem: export 在最后一次父目录验证后仍通过原始路径调用 `persist`，同账户进程可替换祖先目录并使 publish 重新解析到受保护的 data 路径。

Disposition: fixed

Status: resolved

Resolution: 提交 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已修改 `Cargo.toml`、`crates/skilload-core/Cargo.toml` 与 `crates/skilload-core/src/adapters/portable_library.rs`：以 no-follow、identity-verified 的父目录 `File` 持有 publish capability，并通过 `rustix::fs::renameat` 以相对名称发布 staging；新增 `export_does_not_publish_through_a_replaced_parent_directory` 回归测试。

Evidence: `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已推送；`mise exec -- cargo test -p skilload-core --locked portable_library` 通过（9 tests），workspace fmt check、Clippy `-D warnings`、all-features locked tests（7、12、52）、build 与 `git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3811133054；thread resolved: true。

### PRRC_kwDOT7YN2s7jJWGZ — 既有数据库打开竞争

Source: 内联评论 `PRRC_kwDOT7YN2s7jJWGZ`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810877849)；线程 `PRRT_kwDOT7YN2s6aYP_1`，当前已解决。

Problem: 既有 database 的 regular-file 检查与 `Connection::open` 之间存在替换窗口，默认 read-write/create 路径语义可能创建新文件或跟随 raced symlink。

Disposition: fixed

Status: resolved

Resolution: 提交 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已修改 `crates/skilload-core/src/adapters/sqlite_library.rs`：既有 database 以 `READ_ONLY`/`READ_WRITE` 加 `SQLITE_OPEN_NOFOLLOW` 和 no-create 语义打开，打开前后及写前以 device/inode 绑定 identity，sync 也不跟随路径；新增 `existing_import_rejects_a_database_replaced_after_open`。

Evidence: `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已推送；`mise exec -- cargo test -p skilload-core --locked sqlite_library` 通过（14 tests），workspace fmt check、Clippy `-D warnings`、all-features locked tests（7、12、52）、build 与 `git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3811134300；thread resolved: true。

### PRRC_kwDOT7YN2s7jJWGc — 缺失 schema 列的损坏诊断

Source: 内联评论 `PRRC_kwDOT7YN2s7jJWGc`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810877852)；线程 `PRRT_kwDOT7YN2s6aYP_4`，当前已解决。

Problem: durable schema 缺少必需列时 SQLite 的 `no such column` 错误落入 `invalid_state`，没有返回已承诺的 `database_corrupt` recovery diagnostic。

Disposition: fixed

Status: resolved

Resolution: 提交 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已修改 `crates/skilload-core/src/adapters/sqlite_library.rs`：`SqliteFailure` 与 bundled SQLite 的 `SqlInputError` 均将缺表/缺列分类为 `database_corrupt`；新增 `missing_schema_column_is_database_corrupt` fixture。

Evidence: `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已推送；`mise exec -- cargo test -p skilload-core --locked sqlite_library` 通过（14 tests），workspace fmt check、Clippy `-D warnings`、all-features locked tests（7、12、52）和 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3811135351；thread resolved: true。

### PRRC_kwDOT7YN2s7jJWGd — 超出 API-v1 UInt 的 schema version

Source: 内联评论 `PRRC_kwDOT7YN2s7jJWGd`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810877853)；线程 `PRRT_kwDOT7YN2s6aYP_5`，当前已解决。

Problem: 大于 API-v1 `UInt` 最大值的非负 SQLite schema version 会进入 `SchemaNewer` 并生成不可表示的 `found_version`。

Disposition: fixed

Status: resolved

Resolution: 提交 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已修改 `crates/skilload-core/src/adapters/sqlite_library.rs`：在构造 `SchemaNewer` 前将原始版本限制为 API-v1 `UInt` 最大值，并以 `database_corrupt` 拒绝越界值；新增 `schema_version_above_api_uint_range_is_database_corrupt` fixture。

Evidence: `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已推送；`mise exec -- cargo test -p skilload-core --locked sqlite_library` 通过（14 tests），workspace fmt check、Clippy `-D warnings`、all-features locked tests（7、12、52）和 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3811136533；thread resolved: true。

### PRRC_kwDOT7YN2s7jJWGh — 首次 import 目录 identity 清理

Source: 内联评论 `PRRC_kwDOT7YN2s7jJWGh`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810877857)；线程 `PRRT_kwDOT7YN2s6aYP_8`，当前已解决。

Problem: pre-commit failure cleanup 只检查当前路径是目录，未比较调用创建时的 device/inode，可能删除外部进程替换出的空目录。

Disposition: fixed

Status: resolved

Resolution: 提交 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已修改 `crates/skilload-core/src/adapters/configuration.rs` 与 `crates/skilload-core/src/adapters/sqlite_library.rs`：每个创建目录携带 device/inode，first-import cleanup 只移除同一 identity 的空目录；新增 `first_import_cleanup_preserves_replaced_created_directory`。

Evidence: `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已推送；`mise exec -- cargo test -p skilload-core --locked sqlite_library` 通过（14 tests），workspace fmt check、Clippy `-D warnings`、all-features locked tests（7、12、52）和 build 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3811137704；thread resolved: true。

### PRRC_kwDOT7YN2s7jJWGm — human import 计划来源投影

Source: 内联评论 `PRRC_kwDOT7YN2s7jJWGm`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810877862)；线程 `PRRT_kwDOT7YN2s6aYP_-`，当前已解决。

Problem: 非 JSON import 输出只显示集合长度，丢失 dry-run 与多 entry import 的 planned added/updated/kept canonical source 身份，违反同一 application outcome 的双投影约束。

Disposition: fixed

Status: resolved

Resolution: 提交 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已修改 `crates/skilload-cli/src/human.rs`：每个 added/updated/kept/conflicts 集合在保留计数后稳定逐项输出 terminal-safe quoted canonical source；新增 `library_import_renderer_lists_quoted_planned_sources`。

Evidence: `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已推送；`mise exec -- cargo test -p skilload-cli --locked human` 通过（3 unit tests、1 CLI integration test），实际 dry-run CLI smoke 输出 quoted added source；workspace fmt check、Clippy `-D warnings`、all-features locked tests（7、12、52）和 build 通过。

GitHub outcome: 批量请求在客户端超时后继续写入，产生两条内容相同的回复 https://github.com/bootids/skilload/pull/3#discussion_r3811138645 与 https://github.com/bootids/skilload/pull/3#discussion_r3811140238；thread resolved: true。

### PRRC_kwDOT7YN2s7jJWGr — state revision 溢出

Source: 内联评论 `PRRC_kwDOT7YN2s7jJWGr`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3810877867)；线程 `PRRT_kwDOT7YN2s6aYQAA`，当前已解决。

Problem: `state_revision = i64::MAX` 通过当前校验后，SQLite 加一可能存为 REAL 并提交，下一次读取才把本次制造的状态识别为损坏。

Disposition: fixed

Status: resolved

Resolution: 提交 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已修改 `crates/skilload-core/src/adapters/sqlite_library.rs`：`state_revision` 在 additions 前以受上界约束的 SQL update 递增；不可递增时在写入任何 entry 前返回 `state_revision_not_incrementable`；新增 `nonincrementable_state_revision_rejects_import_without_mutation`。

Evidence: `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 已推送；`mise exec -- cargo test -p skilload-core --locked sqlite_library` 通过（14 tests），workspace fmt check、Clippy `-D warnings`、all-features locked tests（7、12、52）和 build 通过。

GitHub outcome: 批量请求在客户端超时后继续写入，产生两条内容相同的回复 https://github.com/bootids/skilload/pull/3#discussion_r3811139704 与 https://github.com/bootids/skilload/pull/3#discussion_r3811142216；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbul — 首次数据库发布绑定 data directory

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbul`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425189)；线程 `PRRT_kwDOT7YN2s6aZqw3`，当前已解决。

Problem: final root revalidation 后，`persist_noclobber(&database)` 仍按路径重新解析 data directory；同账号进程替换该目录可让首次数据库发布逃离已验证的 XDG identity。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 新增持有 no-follow data-directory descriptor 和 `renameat_with(..., RenameFlags::NOREPLACE)`；首次 staging 的 cleanup 也通过 held descriptor 删除同 inode entry。publish 前后重验 directory identity，`DataDirectoryReplacement` fixture 证明 replacement 后不会在新旧目录发布 database。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；`first_import_rejects_a_replaced_data_directory_before_publish`、core 61 tests、workspace fmt/Clippy/all-features locked test/build 和隔离 CLI smoke 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812150222；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbus — schema version 行完整性

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbus`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425196)；线程 `PRRT_kwDOT7YN2s6aZqw6`，当前已解决。

Problem: `schema_info` 缺少 version row 时 `query_row` 被映射为 `invalid_state`，并且多行 version 被静默忽略；两种 durable schema 损坏都应进入 `database_corrupt`。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `singleton_i64` 要求 `schema_info` 恰有一条可解码 version；缺失、额外或错误类型均返回 `database_corrupt`。新增 `missing_schema_version_row_is_database_corrupt` 与 `multiple_schema_version_rows_are_database_corrupt`。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；core 61 tests（含两个 cardinality fixture）和 workspace gates 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812152604；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbuu — repository display 与 canonical identity

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbuu`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425198)；线程 `PRRT_kwDOT7YN2s6aZqw8`，当前已解决。

Problem: `SourceIdentity::new` 只拒绝空 `repository_display`，允许其 ASCII-lowercased repository identity 与 canonical `repository` 不同，进而让 root Skill 的 derived name 依赖伪造 display spelling。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/source.rs` 在 `SourceIdentity::new` 要求 `repository_display.eq_ignore_ascii_case(repository)`，并新增 `portable_source_rejects_mismatched_repository_display`。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；core 61 tests 证明 root Skill 不能从不相关 display spelling 导出 name。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812154556；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbu1 — partial directory 创建清理

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbu1`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425205)；线程 `PRRT_kwDOT7YN2s6aZqxA`，当前已解决。

Problem: `ensure_restrictive_directory` 在创建多个祖先后若后续 create/open/restrict 失败，会在结果返回前丢失已创建 descriptor，导致 pre-COMMIT failure 遗留调用创建的空 state directory。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/configuration.rs` 将 `ensure_restrictive_directory` 的创建过程包入 failure cleanup：任何后续 create/open/restrict/hook error 都按 held descriptor 与 device/inode 逆序删除空、仍属本调用的目录。新增 `restrictive_directory_rolls_back_partial_created_prefix`。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；core 61 tests（含 partial-prefix rollback fixture）和 workspace gates 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812158967；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbvB — Library import limit API 表示

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbvB`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425217)；线程 `PRRT_kwDOT7YN2s6aZqxH`，当前已解决。

Problem: six 个 Library import JSON ceiling 当前均返回 `agent_input_limit_exceeded`，但 API-v1 将该 code 保留给 `agent-project-input-v1`，并禁止对不同条件复用 code。

Disposition: fixed

Status: resolved

Resolution: 新增 `docs/product-specs/api-v2.md`，将 current CLI producer 统一切换为 `api_version: 2`，并定义 `library_input_limit_exceeded` → `LimitDetails`；`AppError::LibraryInputLimit` 只由六个 Library scanner ceiling 构造。`api-v1.md` 保留为历史 contract，`SKL-LIB-010` 升为 Revision 4、相关 CLI IDs 升为 Revision 2。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；`api_v2_library_limit_uses_its_dedicated_code`、scanner regressions 和隔离 CLI smoke 返回 `library_input_limit_exceeded`、`library_import_number_bytes`、`129`/`128` 与 input `PathValue`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812161254；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbvG — database lock descriptor identity

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbvG`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425222)；线程 `PRRT_kwDOT7YN2s6aZqxK`，当前已解决。

Problem: 既有 lock 的 `symlink_metadata` 与 `open_restrictive_lock` 之间发生 file replacement 时，no-follow 打开的 descriptor 未与已检查 inode 比较；两个进程可能在不同 inode 上分别持锁并并发写同一数据库。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/configuration.rs` 将 inspected path、opened descriptor 和 expected device/inode 绑定，并在取得 advisory lock 后再次比较；existing 与 `AlreadyExists` race branch 均覆盖。新增 `lock_rejects_replacement_after_path_inspection`。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；core 61 tests 的 replacement fixture 保留外部 lock 并返回 `lock_path_identity_drift`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812163765；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbvM — 持久 tag comparison key 损坏

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbvM`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425228)；线程 `PRRT_kwDOT7YN2s6aZqxP`，当前已解决。

Problem: `load_tags` 只读取 display 并重新计算 key，未验证数据库中规范 Unicode comparison key；损坏 key 因而被 export/import 接受而不是作为 corruption 拒绝。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `load_tags` 同时读取 `comparison_key` 与 display，以固定 Unicode-15.1 normalizer 验证两者都为 canonical stored value；mismatch 返回 `database_corrupt`。新增 `malformed_tag_comparison_key_is_database_corrupt`。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；core 61 tests（含 comparison-key corruption fixture）和 workspace gates 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812166952；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbvS — staging inode 发布验证

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbvS`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425234)；线程 `PRRT_kwDOT7YN2s6aZqxS`，当前已解决。

Problem: export 最终 `renameat` 只按 staging filename 发布；同账号进程在 write/sync 后替换该 directory entry 时，可发布与已 sync descriptor 不同的内容。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 以 `fstat` 与 parent-descriptor-relative `statat(..., SYMLINK_NOFOLLOW)` 在 rename 前后验证 held staging inode；drift 不报告成功，且 `NamedTempFile` cleanup 不会删除未知 replacement。新增 `export_reports_staging_replacement_after_identity_check`。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；core 61 tests 的 staging replacement fixture 返回 `validation_failed` 而非 success。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812172488；thread resolved: true。

### PRRC_kwDOT7YN2s7jLbvc — native output path typed error

Source: 内联评论 `PRRC_kwDOT7YN2s7jLbvc`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3811425244)；线程 `PRRT_kwDOT7YN2s6aZqxb`，当前已解决。

Problem: export staging/write/sync/rename failure 把可能含 invalid UTF-8 bytes 的 native output path 以 lossy `.display()` 写入 `InvalidStateDetails.expected`，既丢失 PathValue bytes 又滥用 logical state-label field。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 的 `export_io` 改为 `AppError::Validation` 并携带原始 `NativePath`，JSON 由 API-v2 `ValidationDetails.path: PathValue` 投影；不再将 lossy path display 放入 `InvalidStateDetails.expected`。新增 core 与 CLI JSON native-byte fixtures。

Evidence: `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送；`export_io_uses_a_typed_native_output_path`、`api_v2_error_paths_preserve_native_bytes` 和 workspace gates 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812175152；thread resolved: true。

### PRRC_kwDOT7YN2s7jOVB2 — empty Library 的 tags schema probe

Source: 内联评论 `PRRC_kwDOT7YN2s7jOVB2`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812184182)；线程 `PRRT_kwDOT7YN2s6abnkR`，当前已解决。

Problem: `library_entries` 为空时不会调用 `load_tags`，缺失或 malformed `library_tags` table 可被 export 误报为健康空 Library。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 新增 `validate_library_tags_schema`，在 entry iteration 前以 required-column probe 验证 `library_tags`；新增 `empty_library_with_missing_tags_schema_is_database_corrupt`。

Evidence: `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 已推送；core 66 tests 的 empty durable schema fixture 返回 `database_corrupt`，完整 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812270359；thread resolved: true。

### PRRC_kwDOT7YN2s7jOVB8 — state revision singleton

Source: 内联评论 `PRRC_kwDOT7YN2s7jOVB8`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812184188)；线程 `PRRT_kwDOT7YN2s6abnkW`，当前已解决。

Problem: 多个 `state_revision` row 时 `query_row` 任取一个，read/dry-run 可误报健康，后续 import 才以 `invalid_state` 失败而绕过 corruption recovery。

Disposition: fixed

Status: resolved

Resolution: `validate_database` 现在对 `state_revision` 调用 `singleton_i64`，缺失、额外或错误类型均归为 `database_corrupt`；新增 `multiple_state_revision_rows_are_database_corrupt`。

Evidence: `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 已推送；core 66 tests 的 singleton fixture 返回 `database_corrupt`，完整 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812272487；thread resolved: true。

### PRRC_kwDOT7YN2s7jOVCG — export acceptance 的 API-v2 引用

Source: 内联评论 `PRRC_kwDOT7YN2s7jOVCG`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812184198)；线程 `PRRT_kwDOT7YN2s6abnkc`，当前已解决。

Problem: `SKL-LIB-009` acceptance 仍将 command result 称为 API-v1，和同层 product spec 的 API-v2 sole-current-producer contract 冲突。

Disposition: fixed

Status: resolved

Resolution: `docs/product-specs/library.md` 的 `SKL-LIB-009` acceptance 已改为 API-v2 command result；不改文件 output、错误或 revision semantics。

Evidence: `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 已推送；`SKL-CLI-004` Revision 2、product index 和 Library export acceptance 一致，`git diff --check` 与完整 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812274260；thread resolved: true。

### PRRC_kwDOT7YN2s7jOVCL — first-import staging inode

Source: 内联评论 `PRRC_kwDOT7YN2s7jOVCL`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812184203)；线程 `PRRT_kwDOT7YN2s6abnkg`，当前已解决。

Problem: 首次 database publish 的 `renameat_with` 使用 staging name，但没有把该 entry 与 held staging file inode 比较；同账号 replacement 可让 import 报告 `changed` 却发布不同 inode。

Disposition: fixed

Status: resolved

Resolution: `FirstImportStaging::verify_entry` 在 `renameat_with` 前后以 `fstat`/descriptor-relative `statat(SYMLINK_NOFOLLOW)` 比较 held staging FD；drift 返回 `database_identity_drift`，不报告 success。新增 `first_import_reports_staging_identity_drift_after_publish_race`。

Evidence: `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 已推送；core 66 tests 的 deterministic replacement fixture 证明 foreign inode 被 publish 时 import 返回 error。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812276183；thread resolved: true。

### PRRC_kwDOT7YN2s7jOVCO — GitHub repository name length

Source: 内联评论 `PRRC_kwDOT7YN2s7jOVCO`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812184206)；线程 `PRRT_kwDOT7YN2s6abnkj`，当前已解决。

Problem: `SourceIdentity` 接受 101+ byte repository component，尽管 GitHub repository metadata name 上限为 100 ASCII characters。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/source.rs` 将 normalized repository 限制为 1–100 ASCII bytes，并新增 `portable_source_rejects_github_overlength_repository`；`SKL-SRC-002` 同步记录已存在 GitHub identity constraint。

Evidence: `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 已推送；verified GitHub 100-character limit、core 66 tests 和完整 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812277867；thread resolved: true。

### PRRC_kwDOT7YN2s7jOVCV — immutable commit source evidence

Source: 内联评论 `PRRC_kwDOT7YN2s7jOVCV`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812184213)；线程 `PRRT_kwDOT7YN2s6abnkm`，当前已解决。

Problem: `ref_kind: "commit"` 的 source ref 和 `ResolvedSkill.commit` 可各自合法却不同，产生 normal resolution 不可能生成的 immutable source evidence。

Disposition: fixed

Status: resolved

Resolution: `ResolvedSkill::new` 对 commit-kind source 要求 `source.ref_value == commit`，并新增 `immutable_source_requires_matching_resolved_commit`；`SKL-SRC-002`/`SKL-SRC-005` 同步澄清 immutable evidence。

Evidence: `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 已推送；core 66 tests 和隔离 CLI smoke 均证明 mismatch 返回 `validation_failed` 且不创建 XDG state。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812279916；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSnG — export staging publish race

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSnG`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436422)；线程 `PRRT_kwDOT7YN2s6acRkl`，当前已解决。

Problem: export 在最终 staging inode 检查后仍可能以被替换的 staging entry 执行 `renameat`，报错前已经覆盖旧 output。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 修改 `crates/skilload-core/src/adapters/portable_library.rs`：final hook 后重验 held staging/parent identity，创建并安全移除随机 placeholder，以 descriptor-relative `linkat` 建立且重验 held inode 的 publication link，再 rename 该 link；成功后按 held identity 清理原 staging link。`export_reports_staging_replacement_after_identity_check` 现断言旧 output 保留。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送，PR head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --locked portable_library`（12 tests）、workspace fmt/Clippy/all-features locked tests/build 和 `git diff --check` 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812703250；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSnV — first-import staging publish race

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSnV`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436437)；线程 `PRRT_kwDOT7YN2s6acRk0`，当前已解决。

Problem: first import 在 final staging inode check 后可将替换 entry 发布为 `skilload.db`，随后才报告 identity drift。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 修改 `crates/skilload-core/src/adapters/sqlite_library.rs`：`FirstImportStaging::link_for_publication` 在 held data directory 内建立、重验并 no-clobber rename held inode 的 publication link；成功后删除原 staging link，staging replacement fixture 现断言没有 live `skilload.db`。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送，PR head 已核对为同一 SHA；`first_import_reports_staging_identity_drift_after_publish_race`、`mise exec -- cargo test -p skilload-core --locked sqlite_library`（26 tests）及 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812705777；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSnk — first-lock clone cleanup

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSnk`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436452)；线程 `PRRT_kwDOT7YN2s6acRk_`，当前已解决。

Problem: 新建 `database.lock` 后若 cleanup handle 的 `try_clone` 失败，cleanup guard 尚未登记该 lock，导致首次 import 的 pre-COMMIT rollback 遗留 lock/state 目录。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 令 `crates/skilload-core/src/adapters/sqlite_library.rs` 在 created lock 的 hook/`try_clone` failure 分支使用仍持有的 original FD 调用 `remove_created_lock`，随后 RAII guard 清理目录；仅 clone 成功才登记 retained handle。新增 `first_import_created_lock_clone_failure_removes_created_state`。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；clone-failure injection、focused SQLite（26 tests）和 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812707968；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSnr — database error native path

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSnr`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436459)；线程 `PRRT_kwDOT7YN2s6acRlE`，当前已解决。

Problem: database creation/publication/durability I/O error 将 native database path lossy-render 到 `InvalidStateDetails.expected`，违反 API-v2 对状态标签与 `PathValue` 的分工。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 在 `crates/skilload-core/src/error.rs` 增加 `InvalidState.path`/`invalid_state_at_path`；`sqlite_library.rs` 的 durability I/O errors 传入原始 `NativePath`，`crates/skilload-cli/src/json.rs`/`human.rs` 投影可选 lossless path，`docs/product-specs/api-v2.md` 同步 catalog。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；`database_sync_error_preserves_native_path_bytes`、`api_v2_invalid_state_paths_preserve_native_bytes` 和 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812710458；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSn1 — human import conflict details

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSn1`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436469)；线程 `PRRT_kwDOT7YN2s6acRlO`，当前已解决。

Problem: human-mode Library import 对 `ConflictDetails` 只显示计数，遗漏 JSON 中已有的 conflict alias/name 和 rejected canonical source。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 修改 `crates/skilload-cli/src/human.rs`，逐一输出 terminal-safe quoted conflict kind、name（或 null）和 canonical source；新增 alias/null-name renderer regression。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；CLI unit/integration tests（11/12）通过；实际 `target/debug/skilload library import` smoke 先导入一个 alias，随后冲突 import 以 exit 4 输出 `"review"` 和 `github:owner/repository#skills/other@refs/heads/main`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812713029；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSoC — existing database final sync drift

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSoC`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436482)；线程 `PRRT_kwDOT7YN2s6acRlX`，当前已解决。

Problem: existing import 最后一次 database sync 后没有重验 live path/held descriptor identity，且随后 parent sync 重新按 path 打开目录；被替换 generation 仍可能报告 `changed`。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 以 `ValidatedDataDirectory` 的 descriptor-relative `openat`、`fstat`/`statat` revalidation、file sync 和 held-parent sync 替换 existing import 的 path reopen；新增 `existing_import_rejects_a_database_replaced_after_final_sync`。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；focused SQLite（26 tests）和 workspace all-features locked tests 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812716123；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSoX — first-import sidecar rollback

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSoX`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436503)；线程 `PRRT_kwDOT7YN2s6acRln`，当前已解决。

Problem: first-import staging rollback 只清理主 database temp file；SQLite `-journal`、`-wal` 或 `-shm` sidecar 可使新建目录非空，破坏 pre-COMMIT absent-footprint guarantee。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 令未发布 `FirstImportStaging` 仅在 held data-directory descriptor 中清理同 staging basename 的 regular `-journal`、`-wal`、`-shm`，之后由 existing cleanup guard 删除空根；新增 `first_import_precommit_failure_removes_staging_sidecars`。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；sidecar injection rollback regression、focused SQLite（26 tests）和 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812718514；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSog — SQLite contention result

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSog`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436512)；线程 `PRRT_kwDOT7YN2s6acRlu`，当前已解决。

Problem: SQLite `DatabaseBusy`/`DatabaseLocked` 被投影为 `invalid_state`，且 connection 采用默认而非 Library 的两秒 bounded wait，正常 transient contention 失去 `busy` 语义。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 为所有 repository `Connection` 设置既有两秒 `LOCK_WAIT`，将 `DatabaseBusy`/`DatabaseLocked` 以及 singleton/tag iterator SQL errors 经 `database_error` 投影为 `Busy { lock_domain: "database", waited_ms: 2000 }`。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；external `BEGIN EXCLUSIVE` regression 等待两秒后返回 typed busy；focused SQLite（26 tests）和 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812720968；thread resolved: true。

### PRRC_kwDOT7YN2s7jPSom — number ceiling early stop

Source: 内联评论 `PRRC_kwDOT7YN2s7jPSom`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812436518)；线程 `PRRT_kwDOT7YN2s6acRl0`，当前已解决。

Problem: JSON scanner 在检查 128-byte number ceiling 前遍历完整 numeric token，未在第 129 byte 立即停止。

Disposition: fixed

Status: resolved

Resolution: 本次 preliminary commit 将 `JsonScanner::parse_number` 的 integer/fraction/exponent advancement 统一接入即时 ceiling 检查；新增 `scanner_stops_at_the_first_number_byte_overage`，验证第 129 byte 返回 limit 且 cursor 不遍历剩余 token。

Evidence: code/preliminary ledger commit `2e0a46efb308c18546ff6855ac081818fa416088` 已推送；focused portable transfer（12 tests）和 workspace all-features locked tests 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3812723464；thread resolved: true。

### PRRC_kwDOT7YN2s7jQzAF — export publication link 清理

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAF`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831237)；线程 `PRRT_kwDOT7YN2s6adTiE`，当前未解决。

Problem: `publish_staging` 建立 `.skilload-publish-*` publication hard link 后，如果最终 `renameat` 失败，只清理原 `.skilload-library-*` staging entry，遗留的 publication link 违反 `SKL-LIB-009` 对 rename 前失败清理 staging 的要求。

Disposition: fixed

Status: open

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 在 `publish_staging` 的最终 `renameat` error 分支按 held inode 先清理 `publication_name`，再清理原 staging name；新增 `after_publication_link_before_rename` fault hook 与 `export_removes_publication_link_when_rename_fails`，证明外部创建的 destination directory 保留、两个 skilload staging entry 都不存在。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`export_removes_publication_link_when_rename_fails`、`cargo fmt --all --check`、Clippy `-D warnings`、workspace all-features locked tests（77 个 core tests）与 workspace build 均通过。

GitHub outcome: 未回复；thread resolved: false。

### PRRC_kwDOT7YN2s7jQzAQ — portable export/import 字节闭环

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAQ`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831248)；线程 `PRRT_kwDOT7YN2s6adTiN`，当前未解决。

Problem: 单条 note 的 `SKL-LIB-008` 合法上限与多次独立 import 可使 durable Library 的确定性 export 超过 `SKL-LIB-010` 的 67,108,864-byte input ceiling，导致当前二进制成功导出却无法重新导入自身 portable document。

Disposition: fixed

Status: open

Resolution: `crates/skilload-core/src/domain/library.rs` 新增共享 `MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES`、限制写入器和 deterministic encoder；`portable_library.rs` 用它读取/导出，`sqlite_library.rs` 在每个实际 import/dry-run plan 对完整结果调用 consuming size check。`docs/product-specs/library.md`、`docs/design-docs/application-and-persistence.md` 与本 Plan 将其说明为既有唯一可移植文档闭环，行为 revision 不变。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`transfer_encoding_rejects_a_document_over_its_byte_limit` 与 `transfer_encoding_rejects_valid_metadata_beyond_the_import_ceiling`（4,097 个单条合法最大字节 note）均通过，全部 workspace gate 也已通过。

GitHub outcome: 未回复；thread resolved: false。

### PRRC_kwDOT7YN2s7jQzAb — first-import 锁后重新规划

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAb`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831259)；线程 `PRRT_kwDOT7YN2s6adTiT`，当前未解决。

Problem: 两个首次 import 都在 lock 前观察到 database absent 时，胜者发布后败者拿到 lock，当前 `import_first` 仍将存在的 database 视为 `database_identity_drift`，而不是在锁内以当前 durable entries 重新规划并执行 existing-database 路径。

Disposition: fixed

Status: open

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 将 existing import 的锁内工作提取为 `import_existing_with_lock`；`import_first` 取得 lock 后发现合法 database 时以原 document 调用它重新规划，且保留已经出现的 durable state，不重入 lock。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`first_import_replans_after_a_concurrent_winner_publishes` 证明 winner 发布后 loser 添加自己的 source 且 export 含两条记录，全部 workspace gate 已通过。

GitHub outcome: 未回复；thread resolved: false。

### PRRC_kwDOT7YN2s7jQzAh — first-import staging 无跟随打开

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAh`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831265)；线程 `PRRT_kwDOT7YN2s6adTiW`，当前未解决。

Problem: `NamedTempFile` 创建后到 `Connection::open` 前，same-account 替换 staging basename 为 symlink 会使 SQLite 以 read-write/create path open 跟随外部 target；最终 publish identity check 太晚，不能撤销 SQL 已对外部数据库造成的写入。

Disposition: fixed

Status: open

Resolution: `FirstImportStaging::open_connection` 在 hook 前后验证 held staging inode，并以 `SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_NOFOLLOW`、无 create flag 打开；只有第二次验证成功才配置 connection 或执行 SQL。新增 pre-open symlink replacement regression，外部数据库保持原字节，未知 symlink 不被 cleanup。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`first_import_does_not_follow_a_staging_replacement_before_open` 证明 foreign database 字节不变，完整 workspace gate 已通过。

GitHub outcome: 未回复；thread resolved: false。

## Context and Orientation


仓库是一个 Rust Cargo workspace。`crates/skilload-core` 负责 domain、application、ports 和 adapters；`crates/skilload-cli` 是唯一进程入口，负责 clap 参数、终端文本和 API-v2 JSON。P2 已在 `domain/source.rs`、`domain/library.rs`、`domain/unicode_15_1.rs`、`application/library.rs`、`ports/library.rs`、`adapters/portable_library.rs` 与 `adapters/sqlite_library.rs` 实现可移植传输；CLI 的 `args.rs`、`main.rs`、`json.rs`、`human.rs` 除 `config get|set|unset|list` 外只支持 `library import` 与 `library export`。任何后续 Library 行为仍必须沿相同内向方向加入，command handler 不得直接操作 SQLite 或文件。

Library 是本机可搜索的来源元数据集合；在本交付中它只保存一个可移植记录：`ResolvedSkill` 的 canonical source、数字 repository ID、40 位 commit、`sha256:` integrity、验证过的 name/description 和 entry/byte count，加上可选 alias/category/tags/note。canonical source 是带有小写 owner/repository、规范化 Skill path 和完整 branch/tag/SHA ref intent 的字符串身份；它不是 URL、缓存路径或 Trust 凭据。导入的记录永远没有 Trust；未来 Trust 查询可以把它投影为 `missing`，但 P2 不创建 Trust 表或命令。

可移植文档是恰好一个 JSON 对象：顶层 `format_version: 1` 和 `entries` 数组；每个元素是 API-v2 `PortableLibraryEntry`。export 按 canonical source 的二进制字节序排序 entries，按 tag 的 Unicode-15.1 comparison key 排序 tags，并使用稳定 JSON 序列化。导入 parser 先以 no-follow、nonblocking descriptor 和 `fstat` 确认 native input 是同一 regular file，随后才将其路径用于 `PathValue` 错误；每个 source identity 必须能重新渲染为与其 `canonical` 相同的字符串，`repository_display` 可保留当前显示拼写，但其 ASCII-lowercase identity 必须等于 canonical repository。每个 batch 的 canonical source 只允许一个 entry，后出现的相同 source 作为 null-name `internal_duplicate` conflict 使整个 batch 失败。

“原子导入”表示在一次 SQLite 事务中写入全部新增 entries、tags 和递增的 `state_revision`，或一个也不写；对原本不存在的 database，它还表示只在 staging database commit 后经 held data-directory descriptor no-clobber 发布 live file，并在 commit 前失败时清理本调用创建的 database/sidecar/lock/空目录。commit 后 durability-sync failure 不承诺 absence。“预验证”表示 scanner 先在不建立 portable domain model 或 ImportPlan 的情况下、从已验证的 regular-file descriptor 验证 JSON 语法、键唯一性和每个资源限制，随后才以 `serde` 的 closed schema 解析同一已验证字节；ceiling error 使用 API-v2 `library_input_limit_exceeded` 的 measured/allowed `LimitDetails`。无数据库时 export 和 dry-run 的读取返回内存空库，绝不创建 XDG data/state/config/cache 根。


## Plan of Work


### 里程碑 1：固定依赖、Unicode 数据与 Library domain


先扩展根 `Cargo.toml` 的 workspace dependencies：新增 `rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }`、精确 `unicode-normalization = "=0.1.23"` 以及直接 `libc = "0.2.189"`，后者只为现有 Unix adapter 风格中的 `O_NOFOLLOW`/`O_NONBLOCK` 打开标志提供稳定常量；`crates/skilload-core/Cargo.toml` 仅引用实际使用的三项。运行 Cargo 更新后只提交解析到的 `Cargo.lock`，不顺带升级 P1 已锁定依赖。新增 `crates/skilload-core/build.rs` 与版本控制的 Unicode 15.1.0 `CaseFolding.txt`、`PropList.txt` 输入文件及其许可证说明；build script 只能读取这些本地文件，以 `cargo:rerun-if-changed` 生成 OUT_DIR Rust 表。它必须抽取 White_Space 代码点和 CaseFolding 的 `C`/`F` 映射，拒绝意外数据格式，绝不联网。

在 `crates/skilload-core/src/domain/` 新增 `source.rs`、`library.rs`、`unicode_15_1.rs`，并更新 `mod.rs` 和 `lib.rs` 的有意导出。`source.rs` 定义 `RefIntent`、`SourceIdentity`、`ResolvedSkill`、Integrity/SHA/decimal验证及 canonical parse/render；只接受已序列化的 canonical form，不在本交付中接受 GitHub URL 或连接网络。`library.rs` 定义可移植 entry/document、受验证 metadata、tag display/comparison key、import 请求、计划和结果。Unicode helper 的算法是：按 15.1 White_Space 裁剪、NFC、拒绝空/控制/双向格式字符、应用完整 C/F case fold、再 NFC；保存首次 display spelling 和 comparison key。所有长度用 checked UTF-8 字节和 Unicode scalar 计数，绝不以 `String::len` 代替 scalar 限制。

这个里程碑完成后，domain 单元测试必须能证明相同 canonical source 不会因 `@`、`/`、branch/tag namespace 混淆；` Review ` 与 `review` 给出一个保留首次 display 的 key；组合形式 `café` 与 `cafe` 后接 U+0301 的等价拼写共享一个 key；Turkish locale 不影响比较；`unicode_normalization::UNICODE_VERSION` 为 `(15, 1, 0)`；非法名称、SHA、integrity、超长或控制 metadata 均在持久化前失败。

### 里程碑 2：实现有界文件传输与 SQLite Library repository


在 `crates/skilload-core/src/ports/` 新增 `library.rs`，定义 `LibraryRepository` 和 `LibraryTransferStore`。repository 接收并返回经过 domain 验证的值；transfer store 负责 native input/output 路径、受限读、严格解析、同目录原子输出及 output 与 skilload-owned database generation/lock 的碰撞拒绝。不要将 `rusqlite::Connection`、SQL 行或未验证 JSON 泄漏给 application/domain。必要时把 `adapters/configuration.rs` 中可复用的目录创建、锁获取、XDG 重验和安全 I/O 错误映射抽到一个专门 adapter 模块，并让现有 configuration 测试保持原有行为；不能复制第二套 XDG 或锁策略。

新增 `crates/skilload-core/src/adapters/portable_library.rs`。它必须先以 `OpenOptionsExt` 和 `libc::O_NOFOLLOW | libc::O_NONBLOCK` 打开 input，先后比对 no-follow path metadata 与 descriptor `fstat` 的 file identity，并在任何读取前拒绝 symlink、directory、FIFO、socket、device 或 identity drift；regular descriptor 保持有界读取且错误携带 input `PathValue`。随后以增量字节状态机完成第一个 JSON pass：检查 UTF-8/JSON 语法，统计对象、数组、字符串、数字、Boolean、null 的总值，检查嵌套深度、顶层 `entries` 的对象数、字符串解码后 UTF-8 字节数、数字 token 字节数，以及每一个对象中的重复键。它必须在第一个越界点返回 API-v2 `library_input_limit_exceeded` 的 `LimitDetails`，以 `limit_kind` 区分 `library_import_bytes`、`library_import_entries`、`library_import_values`、`library_import_depth`、`library_import_string_bytes` 和 `library_import_number_bytes`，并报告 measured、allowed 与 input `PathValue`。通过 scanner 后，用同一受限字节缓冲的严格 `#[serde(deny_unknown_fields)]` schema 反序列化。

新增 `crates/skilload-core/src/adapters/sqlite_library.rs`。它必须从已有 `StateRootResolver` 取得 data/state roots：没有 `data/skilload.db` 时 export 和 dry-run 返回空库且不建目录；实际 import 在全部文件/schema/domain 验证与冲突规划完成后才创建 data/state root 和 `state/locks/database.lock`。锁的等待和 typed busy 行为沿用配置的两秒有界策略。若 live database 原先不存在，在同一 data directory 创建 restrictive、唯一的 staging database，完整建立 schema、执行 transaction、处理 sidecar、sync staged file、重验 roots 后才原子 rename 为 `data/skilload.db` 并 sync 父目录；commit 前任何失败都关闭并移除仅由本调用创建的 staging database/sidecar/lock/空目录，绝不触碰预先存在或 identity 不匹配的路径。commit 后 file 或 parent sync 失败返回 typed error，不报告 success 或 state absence。数据库路径、锁和已有文件都必须拒绝 symlink/非预期文件类型，创建目录和数据库使用 restrictive current-user permissions，并在提交前重验根绑定。

初始 schema 是一份明确的 v1 事务：`schema_info` 固定版本、`state_revision` 保存单调语义 revision、`library_entries` 以 canonical source 为主键并存储全部 portable resolved/metadata 标量、`library_tags` 以 `(canonical_source, comparison_key)` 唯一并通过外键关联 entry。开启 foreign keys，使用一个 SQLite transaction 计算并写入导入 plan；不创建 FTS 表、Trust 表或未来 ownership 表。现有 source 默认进入 kept；新 source 与现有/同批 alias 冲突必须在事务开始提交前返回 `conflict` 的 `ConflictDetails`，每个被拒绝 entry 使用 `internal_duplicate`、其 alias 为 `name`、其 source 为 `source`，且 `agent`/`path` 均为 null。对同一 batch 中后出现的相同 canonical source，同样在 transaction 前返回 `internal_duplicate`，但 `name` 为 null、`source` 为该后出现 entry 的 source；两类冲突都不修改任何行。实际新增时递增 revision。数据库已有更高 schema 返回 `schema_newer` 的 `SchemaDetails`；已识别的损坏返回 `database_corrupt` 的 `DatabaseCorruptDetails`，其中 database 是 `PathValue`、`backups`/`recoverable_exports` 因 P2 无恢复资产而为空且 `recovery_procedure` 为 `database-corruption-v1`；非普通文件返回 `invalid_state`，不能…

同一 adapter 同时负责 export：在一个只读一致性事务中按 canonical source 和 comparison key 获取记录，构建不含 Trust/local-state 的 `LibraryExportData`；在创建 staging 前，`LibraryTransferStore` 必须以 no-follow inspection、root revalidation 与 file identity 比较拒绝指向 live `data/skilload.db`、WAL、SHM 或 `state/locks/database.lock` 的 output。对其他 output，它在既有、真实父目录中建临时文件、写入完整 JSON、sync 文件并原子 rename，再 sync 父目录；directory 和 symlink output 一律拒绝。rename 前失败保留既有普通 output 或无 output 且清理 staging；rename 后 parent sync 失败返回 typed error，new output MAY 已发布，不能承诺旧 output 仍在。输出失败绝不改变数据库。P2 不注册 `--replace`，因此 `updated` 始终空。

### 里程碑 3：接入应用、CLI 与双投影


在 `crates/skilload-core/src/application/library.rs` 增加 `Application::library_import` 和 `Application::library_export`，并更新 `application/mod.rs`。`Application` 必须同时接收 configuration 和 Library ports，取消只接收一个 configuration store 的构造签名，并完整迁移 `crates/skilload-cli/src/main.rs` 及 `crates/skilload-core/src/adapters/configuration.rs` 测试中的所有 `Application::new` 调用。构造不会打开数据库；生产 composition 使用 `FileConfigurationStore`、`SqliteLibraryRepository` 和 `PortableLibraryTransferStore`。应用在 dry-run 时只读取/规划，在 commit 时只调用原子 repository import，并返回展示中立的 Library data/outcome。

扩展 `crates/skilload-cli/src/args.rs`：注册 `library import --input <PATH> [--dry-run]` 和 `library export --output <PATH>`，不注册任何其他 library 叶子、别名或隐藏 shortcut。将仅识别 configuration 的 JSON-operation 预扫描泛化为所有已实现叶子，使 Library 参数错误在 `--json` 下仍使用正确的 `library.import` 或 `library.export` operation。更新 parser/help 测试，证明这两个叶子存在、未实现的 Library 名称仍失败，且 `--input`/`--output` 不会被错误放到其他叶子。

扩展 `main.rs` 的 `Projection` 和 dispatch，使 CLI 只转换参数并调用 application。扩展 `json.rs` 以投影 `SourceIdentity`、`ResolvedSkill`、`PortableLibraryEntry`、`LibraryExportData` 与 `LibraryImportData`；成功 envelope 始终是单一 API-v2 JSON 值。为本交付新增的限制、alias 冲突、路径校验和数据库状态错误补足与 API-v2 catalog 对应的 `LimitDetails`、带 `internal_duplicate` 字段约束的 `ConflictDetails`、`ValidationDetails`、`SchemaDetails`、`DatabaseCorruptDetails` 或 `InvalidStateDetails`，不能把数字限制或恢复证据塞进散文错误字符串。扩展 `human.rs`，保持英文主要输出和既有注入安全字段编码；人类 import 输出 dry-run/changed/unchanged 加集合计数，人类 export 输出写入的安全引用路径和条目计数。绝不把输入文件、输出文件或异常数据未经编码写到 terminal。

### 里程碑 4：同步文档并完成可观察验收


实施后更新 `docs/product-specs/README.md`、`docs/product-specs/library.md`、`api-v1.md` 历史契约和新的 `api-v2.md` current catalog，使状态 prose 准确列出 `SKL-LIB-009` Revision 3、`SKL-LIB-010` Revision 4 和 API-v2 的 `SKL-CLI-004`/`005`/`012` Revision 2；同步 `docs/product-specs/database-recovery.md` 的显式 export output 调用与 salvage heading；除新的明确产品决定外，不得再修改这些行为正文或 revision。同步 `ARCHITECTURE.md` 的当前实现模块/SQLite ownership 描述，以及 `docs/design-docs/application-and-persistence.md`、`docs/design-docs/cli-json-and-release.md` 的当前实现状态、P2 module names 和真实测试路径。若实现发现本计划中的文件传输语义或字段与 authoritative specification 冲突，先修正实现或在得到明确产品决定后更新产品规格和 Plan baseline；不得静默降低 acceptance。

在 active Plan 中记录每个完成里程碑、所有发现和实际验证。完成实现后，先提交并推送代码、测试、锁文件、文档和 active Plan，再按 `docs/PLANS.md` 的 ready/review 原子事务转换 Draft PR。不要在计划状态中实施任何代码。

## 评审实施补充

2026-08-19 的 active review rework 扩展 Product Baseline，但不增加命令或双版本协商：人类选择 API-v2 current-producer cutover，`library_input_limit_exceeded` 以既有 `LimitDetails` 表示 P2 six-limit scanner。`rustix 1.1.4` 的 descriptor-relative `renameat_with(NOREPLACE)` 绑定首次 database publish，`fstat`/`statat(SYMLINK_NOFOLLOW)` 绑定 export staging FD；data/lock/staging replacement 只会返回 typed error，不能报告 success。`sqlite_library.rs` 还验证 schema_info 恰一 row 和持久 tag comparison key，`source.rs` 绑定 display/canonical repository identity，`configuration.rs` 回滚 partial directory 创建并在 lock acquisition 前后比较 inode，`portable_library.rs` 将 output I/O path 投影为 `ValidationDetails.path: PathValue`。所有修复保持现有命令面、SQLite schema v1 和 P2 离线边界。

## Concrete Steps


所有命令从仓库根目录运行。收到执行授权前只能运行本计划的发布步骤，不得运行会改动运行时代码的命令。

1. 执行授权后的第一步使用 `execute-exec-plan`。它必须确认 `PLAN-0002` 已在 `origin/main` 完成、PR 仍为 Draft、frontmatter branch 与当前分支一致且工作树干净，然后把本文件移动到 `docs/exec-plans/active/`、设置 `status: active`、记录进度并推送。预期是 Draft PR 与 active Plan 的提交均可在 GitHub 上看到。

2. 执行 `mise install`，然后在修改 `Cargo.toml` 后执行：

    mise exec -- cargo update -p rusqlite -p unicode-normalization -p libc
    mise exec -- cargo test -p skilload-core unicode_15_1 --locked

   预期锁文件包含 `rusqlite 0.40.2`、其 bundled SQLite 依赖、直接 `libc 0.2.189` 和精确 `unicode-normalization 0.1.23`；Unicode 测试只通过 `(15, 1, 0)`，解析到 0.1.24 或更高必须失败而不是接受。

3. 实现 domain、生成表、scanner、ports 和 SQLite adapter 后运行 focused core 测试：

    mise exec -- cargo test -p skilload-core --locked library
    mise exec -- cargo test -p skilload-core --locked portable_library
    mise exec -- cargo test -p skilload-core --locked sqlite_library

   预期每组都覆盖 source/metadata validator、Unicode 等价、所有六种 input 限制与重复键、FIFO/device/identity-drift input 拒绝、dry-run 无状态、首次 database staging cleanup、SQLite 全量原子写入、commit 后 sync-error 不伪称 absence、output 原子写入、database/WAL/SHM/lock target 拒绝、rename 前/后 failure 语义、既有 source kept、带规定 `ConflictDetails` 的 alias/canonical-source conflict rollback、带 `DatabaseCorruptDetails` 的损坏数据库只读失败和 FTS5 编译能力检查。

4. 接入 CLI 后在 `crates/skilload-cli/tests/cli_contract.rs` 扩展隔离 XDG 场景并运行：

    mise exec -- cargo test -p skilload-cli --test cli_contract --locked

   预期合法 JSON mode stdout 只含一个可解析 envelope；dry-run 不创建 `data/skilload/skilload.db`；实际 import 后只有所需 data/state 项存在，config/cache 未出现；export 文件本身是无 envelope 的 `LibraryExportData`，可作为下一次 import input；重复 import 是 unchanged 且无数据库重写。CLI fixture 还必须证明 FIFO input 立即失败、duplicate canonical source 不改变导出数据、database-generation/lock output target 在 staging 前失败，以及 rename 后 sync-error 只能报告失败而不能宣称旧 output。

5. 对 67,108,864-byte、10,000-entry、1,000,000-value、八层、1,048,576-byte-string 和 128-byte-number 边界，使用生成 reader/fixture 的 focused test，不把大型 fixture 文本签入仓库。每个精确上界必须通过，超一单位必须在 model/ImportPlan 前以 measured/allowed details 失败；同一组还要创建 FIFO、device（可用时）和 identity-swap input，断言 no-follow/nonblocking descriptor gate 在读取前拒绝。整个测试仍在受控临时目录内运行。

6. 实现完成后执行完整门禁：

    mise exec -- cargo fmt --all --check
    mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings
    mise exec -- cargo test --workspace --all-features --locked
    mise exec -- cargo build --workspace --all-features --locked
    git diff --check

   预期格式、Clippy、所有离线测试、构建和空白检查全部成功。将实际测试计数、commit SHA 和手工场景证据写进 active Plan 后再继续 PR ready 事务。

## Validation and Acceptance


验收必须同时验证 domain、存储和用户表面，而不是只证明 SQLite 可打开。

第一组 core tests 用手写的 `LibraryExportData` 验证 canonical source 的 branch/tag/SHA 区分、40 位 SHA、`sha256:` 摘要、描述与计数；验证文件中 Trust/state/cache/path 字段或未知字段被严格拒绝。测试导入 `Review` 和等价 `review` 标签、NFC/NFD 的 `café`、控制字符、双向格式字符、边界 scalar/byte 长度和第 65 个 tag。它们必须确认有效导入只保留第一个 display spelling，非法值在 SQLite transaction 前失败，并确认同一 batch 的重复 canonical source 无论 metadata 是否相同都返回规定的 null-name `internal_duplicate`。

第二组 parser tests 对每个上界生成精确输入和超一输入。它们检查不建立 `PortableLibraryDocument` 或 `ImportPlan` 的错误路径、重复键不被“最后键覆盖”、JSON string escape 的解码后 UTF-8 计数正确、每一对象层级的重复 key 都被发现，且 API-v2 `library_input_limit_exceeded` 的 `LimitDetails` 同时有 `limit_kind`、decimal measured/allowed 与 input `PathValue`。同组以 FIFO、directory、symlink、device（可用时）和 lstat/open identity swap 证明 input descriptor 在 scanner 前被拒绝，且测试不依赖 writer/EOF 才完成。

第三组 repository tests 从无数据库开始：空 export/dry-run 返回空集合且不创建根；合法 commit import 创建 schema 和 entries/tags；第二次相同 import 返回 unchanged 且数据库内容和文件 identity 不变化；一批中一个 invalid record、alias conflict 或 canonical source duplicate rollback 全部，分别以规定的 `internal_duplicate` alias/name/source 或 null-name/source `ConflictDetails` 失败；首次 import 的 schema/write/commit 前 fault injection 后 data/state 根恢复为 absent，而 commit 后 sync fault 返回错误且不声称 absence。output 目录中的临时写失败不会改数据库或留下最终半文件；database/WAL/SHM/database-lock target 在 staging 前被拒绝；rename 前失败保留旧 output，rename 后 parent sync failure 返回错误且允许新 output 已存在。外部创建的 symlink/非普通 database、lock 或 output 不能被接受。损坏 SQLite fixture 必须保留原文件且返回 `database_corrupt`：database `PathValue`、空 `backups`/`recoverable_exports` 和 `database-corruption-v1` 都与 API catalog 一致。测试还要以 bundled connection 创建临时 FTS5 virtual table 或等价 compile-option probe，证明架构要求的嵌入式能力，而不是把宿主 SQLite 当作依据。

最后执行实际 CLI smoke，所有 XDG 根使用临时绝对路径、网络被禁止：

    skilload library import --input ./portable-library.json --dry-run --json
    skilload library import --input ./portable-library.json --json
    skilload library export --output ./round-trip.json --json

第一条必须是 `library.import`、`ok: true`、`outcome: "observed"`、`dry_run: true`，且不创建 skilload 状态；第二条只能是 `changed` 或在完全相同既有状态下 `unchanged`；第三条必须是 `library.export` 的 observed envelope，并在 `round-trip.json` 中写出仅含 `format_version` 与 `entries` 的可移植 document。用第二个隔离 XDG home 导入该文件并重新导出，规范化 JSON 必须相等，Trust/缓存/绝对路径字符串不得出现。人为插入无效 entry、alias collision、重复 canonical source、重复 JSON key、超限文件或 FIFO input 时，CLI 必须非零退出、JSON stdout 仍只有一个合法 error envelope，并且导入前后 SQLite 可导出数据相同；alias collision 的 envelope 必须含规定的 `internal_duplicate`、alias 与被拒绝 source，canonical duplicate 的 envelope 必须有 null `name`。将 `--output` 指向 database/WAL/SHM/database lock 必须在 staging 前失败且不改动 target；模拟 rename 后 parent sync failure 必须失败且不假称旧 output 保留。损坏数据库 fixture 必须返回 `database_corrupt` 的 `DatabaseCorruptDetails` 且不改写数据库。

本次 active rework 的 smoke 还必须验证所有 success/error envelope 都是 `api_version: 2`；对 129-byte number token 的 import 必须非零退出且返回 `library_input_limit_exceeded`、`library_import_number_bytes`、`measured: "129"`、`allowed: "128"` 和原生 input `PathValue`。

## Idempotence and Recovery


`mise install`、Cargo 格式/测试/构建、同一合法 export、dry-run 和对未变数据库的同一 import 都可以安全重复。实际 import 的 SQLite transaction 要么全部提交 entries/tags/state revision，要么 rollback；所有输入验证、alias/canonical-source 冲突和 commit 前 persistence failure 必须发生在或恢复到无持久 partial state，且首次 import 只能删除该调用创建且 identity 仍匹配的 database/sidecar/lock/空目录。commit 后 durability-sync failure 返回错误但可能已有新 state，不能执行盲目 cleanup。export 只能在目标父目录中临时写入、sync、rename 和 sync 父目录；rename 前失败保留旧普通目标内容或无目标并清理 staging，rename 后 parent sync failure 返回错误且新 target 可能已存在。

实现期间不得删除现有用户根、数据库或 output；首次-import cleanup 仅限本调用创建、仍为空或 identity 匹配的 staging/state 痕迹。测试仅使用临时目录；任何有歧义的 SQLite 文件、锁、symlink、非常规 input 或 output 类型都要失败而非“修复”。export target 若与 live database generation 或 database lock 碰撞也必须失败。此 P2 不实现数据库迁移、备份、导出索引或 reset；发现 version 大于 v1 时拒绝写入，发现数据库损坏时保留原文件、返回带空 P2 已知恢复集合和 `database-corruption-v1` 的 `database_corrupt`，并把完整 recovery 行为留给后续产品交付。

计划生命周期恢复同样必须安全：若 `gh pr ready` 或 review-state push 失败，执行 `gh pr ready <PR-URL> --undo`、确认 PR 回到 Draft，并将/保持计划在 `active` 后重试；若 review 发现 material scope 缺失，先把 PR 退回 Draft，再把 Plan 从 review 移回 active 并记录原因。若 completion 后但 GitHub 报告 `MERGED` 前的检查、队列或 merge 失败，按 `docs/PLANS.md` 把 Plan 恢复为 review 并推送，不能把未合并工作归档为 completed。

## Artifacts and Notes


执行起点的可复核事实如下：`PLAN-0001` 与 `PLAN-0002` 已在默认分支完成；`PLAN-0003` 已于 2026-08-19 05:05Z 经授权进入 `active/`，其唯一关联项是 Draft PR #3。当前分支从包含 PR #2 合并提交的基线创建，预检确认远端分支与 PR head 一致。

依赖证据应保留在 `docs/references/rust-sqlite-unicode-library-foundation.md`：`rusqlite 0.40.2` bundled build 明确启用 FTS5；`unicode-normalization 0.1.23` 表为 15.1.0，而 0.1.25 为 17.0.0。该参考是实施时依赖选择与版本断言的依据，不是运行时网络依赖。

2026-08-19 05:52Z 的验证 evidence：vendored Unicode 输入 SHA-256 与 reference 一致；focused `portable_library` 8 tests、`sqlite_library` 6 tests、`library` filter 16 tests、CLI contract 12 tests 均通过。实际 smoke 使用 `target/debug/skilload library import --input <PATH> --dry-run --json`、实际 import、`library export --output <PATH> --json`，并由第二隔离 root 重导入/重导出得到 byte-identical document。

2026-08-19 05:55Z 的发布前 evidence：implementation commit `4c6a6919921cabcbc29b11cfa255466993ad2adf` 已推送；`gh pr view` 返回 `isDraft: true` 与相同 `headRefOid`，local/remote head 相同且工作树干净。

2026-08-19 05:57Z 的 review-state evidence：首次 review commit `b30afe3aa7a772f1ccf1885eb041006528f10c24` 推送后，PR #3 `isDraft: false`、`state: OPEN`、`headRefName: codex/p2-library-portable-import-export`，local/remote 工作树当时一致且干净。

成功的 portable output 形状应类似下列缩进示例，字段值仅说明结构；真实 source/description 必须由 validator 保留：

    {
      "format_version": 1,
      "entries": [
        {
          "skill": { "source": { "canonical": "github:owner/repo#skills/example@refs/heads/main" }, "repository_id": "42", "commit": "0123456789012345678901234567890123456789", "integrity": "sha256:0123456789012345678901234567890123456789012345678901234567890123", "name": "example", "description": "Example", "entry_count": "1", "byte_count": "10" },
          "alias": null,
          "category": null,
          "tags": [],
          "note": null
        }
      ]
    }

示例省略了 `SourceIdentity` 的其余必填字段以保持简短；实现测试必须使用 API-v1 catalog 定义的完整对象，不能将该缩略示例当作 parser fixture。

## Interfaces and Dependencies


最终 implementation 具有以下核心接口；字段可为私有，但 portable identity、ownership 与 Product Baseline 的 observable result 不得改变。

    pub enum RefIntent { Branch(String), Tag(String), Commit(String) }
    pub enum RefKind { Branch, Tag, Commit }

    pub struct SourceIdentity {
        pub canonical: String,
        pub owner: String,
        pub repository: String,
        pub repository_display: String,
        pub path: String,
        pub ref_kind: RefKind,
        pub ref_value: String,
    }

    impl SourceIdentity {
        pub fn ref_intent(&self) -> RefIntent;
    }

    pub struct ResolvedSkill { /* source, repository_id, commit, integrity, name, description, entry_count, byte_count */ }

    pub struct PortableLibraryEntry { /* skill, alias, category, tags, note */ }

    pub struct PortableLibraryDocument {
        pub format_version: u64,
        pub entries: Vec<PortableLibraryEntry>,
    }

    pub struct LibraryImportRequest {
        pub input: NativePath,
        pub dry_run: bool,
    }

    pub struct LibraryImportResult {
        pub format_version: u64,
        pub dry_run: bool,
        pub added: Vec<SourceIdentity>,
        pub updated: Vec<SourceIdentity>,
        pub kept: Vec<SourceIdentity>,
        pub conflicts: Vec<SourceIdentity>,
    }

    pub trait LibraryTransferStore: Send + Sync {
        fn read_import(&self, input: &NativePath) -> Result<PortableLibraryDocument, AppError>;
        fn write_export(&self, output: &NativePath, document: &PortableLibraryDocument) -> Result<(), AppError>;
    }

    pub trait LibraryRepository: Send + Sync {
        fn export(&self) -> Result<PortableLibraryDocument, AppError>;
        fn import(&self, document: &PortableLibraryDocument, dry_run: bool) -> Result<LibraryImportOperation, AppError>;
    }

`Application` 必须持有 `Arc<dyn ConfigurationStore>`、`Arc<dyn LibraryRepository>` 和 `Arc<dyn LibraryTransferStore>`，但除了 Library commands 不得打开 SQLite。`SqliteLibraryRepository` 只能使用 `StateRootResolver`/`Environment` 的有效 XDG roots，`PortableLibraryTransferStore` 只能处理传入的 native file path；CLI 不得接触 SQL、文件 transaction 或 Unicode table。JSON 序列化必须从 domain/application 结果投影，而不是直接序列化 SQLite 行或 error prose。

计划修订说明（2026-08-19）：创建了 PLAN-0003 的初始计划、SQLite/Unicode 依赖参考和 Revision-1 文件传输接口澄清；没有开始实现。后续首次推送与 Draft PR 创建必须把 URL 和真实发布证据写回本计划。

计划修订说明（2026-08-19）：初始规划基线推送后已创建 Draft PR #3，并记录其 canonical URL；此 metadata 提交必须单独推送，以保持分支、PR 和 frontmatter 一致。

计划修订说明（2026-08-19）：PR #3 评审指出文件传输语义改变了可观察行为、alias 冲突没有 API 字段约束且数据库损坏未映射既有诊断。本修订将 `SKL-LIB-009`/`SKL-LIB-010` 提升为 Revision 2，明确 `internal_duplicate` 和 `database_corrupt` 的 P2 约束，并同步数据库恢复过程使用显式 `library export --output` 文件；仍未开始实现。

计划修订说明（2026-08-19）：第二轮 PR #3 规划评审发现 output 可覆盖活动 database generation、非常规 input 可绕过资源上界、同 batch canonical source 未定义、首次 import 失败可遗留 state、以及 rename 后 sync failure 的错误承诺不成立。本修订将 `SKL-LIB-009`/`SKL-LIB-010` 提升为 Revision 3，定义拒绝/cleanup/错误语义和 `internal_duplicate` 字段，并恢复 recovery procedure 的第 2 节标题；仍未开始实现。

计划修订说明（2026-08-19）：收到明确执行授权后完成 `execute-exec-plan` 预检，确认 `PLAN-0002` 已在 `origin/main` 完成、PR #3 为 Draft 且工作树/分支/PR head 一致；本 Plan 移入 `active/`，尚未开始实现。

计划修订说明（2026-08-19）：完成 P2 四个实现里程碑：vendored Unicode 15.1.0 输入/build generator、portable Library domain、受限 transfer/SQLite adapters、application/CLI/API-v1 projections、focused tests 与 governed documentation。Plan 仍为 `active`，等待完整验证、提交推送和 Draft-to-review 原子事务。

计划修订说明（2026-08-19）：完成 Draft-to-review 原子事务：GitHub ready 后确认 implementation SHA，随后将 Plan 从 `active/` 移入 `review/` 并将 frontmatter 更新为 `status: review`。review-state commit 推送后必须再次核对 GitHub head 与 repository 状态。

计划修订说明（2026-08-19 06:42Z）：PR #3 ready-review 发现七项现有 P2 implementation 缺口。本修订在 `review` 状态记录其 source-complete preliminary ledger，并实现 no-clobber database publish、first-lock cleanup、SQLite row corruption mapping 与完整 portable source revalidation；所有修复仍属于既有 Product Baseline，待提交、推送、GitHub 回复和线程关闭后再将记录收束为 resolved。

计划修订说明（2026-08-19 06:45Z）：preliminary review fix/log 提交 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 已推送，local、upstream 与 PR #3 ready head 已核对。七项 open ledger entry 均已记录具体实现、回归测试和该 SHA；下一步仅为重新读取会话、逐源 GitHub 回复和关闭内联线程。

计划修订说明（2026-08-19 06:51Z）：已重新读取完整 PR 会话并完成逐源 reconciliation。七项 fixed entry 已设为 resolved，记录 pushed implementation SHA、验证、每个回复 URL 和 resolved thread state；所有 16 个 inline thread 当前均为 resolved。两条 timeout-induced duplicate reply 作为真实 GitHub 审计结果保留并在相应条目中说明。

计划修订说明（2026-08-19 08:56Z）：完整读取 PR #3 会话后发现九个新增 inline 问题。八项属于既有 P2 文件 identity、SQLite durable-state、portable source 和 JSON path 约束；另一项揭示 `SKL-LIB-010` 的 `LimitDetails` acceptance 与 API-v1 独占 `agent_input_limit_exceeded` 之间的未决产品冲突。已先将 ready PR 恢复为 Draft 并确认 GitHub head 不变，随后本 Plan 从 `review/` 逆向移回 `active/`，记录 source-complete open/blocked ledger；不在本次 review workflow 中伪造 API 选择、实现或关闭线程，等待人类决定和 `execute-exec-plan` 重新执行授权。

计划修订说明（2026-08-19 09:00Z）：人类已选择 API-v2 独立 `library_input_limit_exceeded` code 并明确授权执行 active rework。Product Baseline 将 `SKL-LIB-010` 提升为 Revision 4，纳入 API-v2 current-producer cutover 的最小 `SKL-CLI-004`/`005`/`012` Revision 2 范围；API-v1 留作历史规格，不提供双输出 mode。下一步实现全部九项 open review remediation、验证、推送并按正常 ready/review 事务重新进入 review。

计划修订说明（2026-08-19 10:17Z）：已在 active rework 本地完成九项 review remediation、API-v2 catalog/cutover、产品/设计/reference 同步、focused tests、workspace gates 和隔离 CLI smoke。Review Conversation Log 的九项 fixed entry 现在记录具体 changed paths、回归测试和待写入的 preliminary commit SHA；所有线程仍保持 open，直到该 commit 推送、逐源 GitHub 回复并关闭后才设为 resolved。

计划修订说明（2026-08-19 10:21Z）：preliminary remediation commit `03b4aa0de8b05963b0c5a2a3ce7b798684d3a92c` 已推送，local/upstream/Draft PR head 均为该 SHA。九项 open ledger entry 都已记录该 SHA 和对应验证；下一步执行 ready/review 原子事务，再重新读取完整 GitHub 会话并逐源回复/关闭。

计划修订说明（2026-08-19 10:23Z）：在所有 remediation、测试、产品/API/设计/reference 和 active Plan evidence 均已推送后，`gh pr ready` 返回 `isDraft: false` 且 `headRefOid` 与 `73f5634a5b9871bb635f5bf8c4fd36ea81bee816` 一致。本 commit 将唯一 Plan copy 移至 `review/`、设置 `status: review` 并记录 ready evidence；下一步重新执行完整 PR 会话 reconciliation。

计划修订说明（2026-08-19 10:32Z）：review-state 下重新读取完整 GitHub 会话后，九个 remediation thread 均已使用具体 code SHA、验证和中文说明回复，并在 reply 成功后逐个关闭。最终列表显示 32 个 thread 全部 resolved、32 个 Plan source 全部覆盖、无新 top-level/review-body 问题；本次最终 Review Conversation Log commit 推送后再进行一次全量核对。

计划修订说明（2026-08-19 10:36Z）：最终 ledger commit 触发的最新 GitHub review 提出六项新的 ordinary P2 fixes。它们已逐源加入 Review Conversation Log（fixed/open），涵盖 empty-table corruption、singleton revision、API-v2 wording、first staging identity、repository length 和 immutable SHA consistency；当前 Plan/PR 均保持 `review`/ready，等待普通 remediation、验证、push 和 thread closure。

计划修订说明（2026-08-19 10:43Z）：第二轮 ordinary remediation commit `19fe009ac578e8fb6bd1eefc2649eaa1802611bf` 已推送，local/upstream/ready PR head 一致。六项 open ledger entry 均记录该 SHA、具体实现和验证；下一步重新读取会话、逐源回复并关闭 thread。

计划修订说明（2026-08-19 10:48Z）：第二轮 six-thread remediation 已逐源回复并关闭；最终 list 显示 38 个 thread 均 resolved、38 个 Plan source 均已覆盖、无新 top-level/review-body 问题。此 final Review Conversation Log commit 推送后，必须再次确认 PR head、Plan 状态和完整会话没有漂移。

计划修订说明（2026-08-19 11:59Z）：记录第三轮九项 ordinary review remediation 的 code/preliminary ledger commit、focused/完整验证、逐 thread GitHub reply URL 与 resolved state；原因是让 `Review Conversation Log` 与 GitHub 的完整会话在最终 reconciliation 前保持可审计一致。

计划修订说明（2026-08-19 12:42Z）：记录第四轮四项 ordinary review remediation 的已推送 code/preliminary ledger commit、完整验证、共享传输大小决策和每个 open thread 的精确 evidence；下一步仅写入 GitHub 回复、关闭线程并提交最终 reconciliation。
