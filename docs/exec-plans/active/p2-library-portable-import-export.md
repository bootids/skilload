---
plan_id: PLAN-0003
branch: codex/p2-library-portable-import-export
pull_request: https://github.com/bootids/skilload/pull/3
status: active
depends_on: [PLAN-0002]
---

# 建立可移植 Library 导入与导出


本交付让用户能够在不联网、不建立 Trust、也不接触外部 Skill 内容的前提下，将一个受限且版本化的 Library 元数据文件预演导入、原子导入到本机持久状态，并重新导出为确定性文件。用户通过 `skilload library import --input <PATH> [--dry-run]` 和 `skilload library export --output <PATH>` 观察结果；导入后导出的文档只包含可移植的已解析 Skill 证据和用户元数据，不包含本机路径、凭据、Trust、缓存或部署状态。

本 ExecPlan 是活文档。实施过程中必须持续更新 `Progress`、`Surprises & Discoveries`、`Decision Log`、`Outcomes & Retrospective` 与 `Review Conversation Log`。本文件必须遵守仓库根目录的 `docs/PLANS.md`。

## Delivery Metadata


本交付是完成的 `PLAN-0002`（Rust 工作区与配置垂直切片）之后的直接后继。`PLAN-0002` 已在默认分支完成并提供了 Cargo 工作区、`skilload-core` 的内向应用边界、`skilload-cli` 的薄展示适配器、严格 XDG 根解析以及当时的 API-v1 配置渲染模式；本 Plan 的 active rework 将 current producer 统一切换到 API-v2。`PLAN-0001` 是其已完成的文档前提，因而不重复列为直接依赖。

本交付只建立可移植 Library 元数据传输与为它服务的最小 SQLite 持久化边界。它不实现 GitHub 输入解析、网络解析、首次 Trust 审批、Library add/remove/list/search/get/refresh、直接元数据编辑、FTS 查询、缓存、工作区、全局部署、manager、doctor 或数据库迁移/恢复命令。未实现的命令必须继续是 usage error，不能注册占位叶子。本计划在 `plan` 状态及 Draft PR 中仅定义和发布工作；只有后续明确的人类执行授权才可以移动到 `active` 并改动实现。

## Product Baseline


本交付完整实现并验证以下两个原子行为：`SKL-LIB-009` 为 Revision 4，`SKL-LIB-010` 为 Revision 5。

* `docs/product-specs/library.md` 中 Revision 4 的 `SKL-LIB-009` 要求 `library export --output <PATH>` 将确定性、版本化且仅含 Library 来源/元数据的 `LibraryExportData` 原子写入请求路径；文件排除 Trust、全局/工作区状态、profile、绝对本机路径、凭据、缓存内容和操作时间，命令自身仍产生既定人类或 API-v2 结果。它在创建 staging 前拒绝活动 database、DELETE rollback journal、WAL、SHM 或 database lock target；rename 前失败保留旧 output 或无 output，rename 后父目录 sync 失败返回错误且不声称旧 output 仍在。
* 同一文件中 Revision 5 的 `SKL-LIB-010` 要求 `library import --input <PATH> [--dry-run]` 在读取前以 no-follow、nonblocking descriptor 和 `fstat` 拒绝非常规或 identity-drift input，并在构建任何 model 或 `ImportPlan` 前执行流式非模型预验证。它分别限制 67,108,864 bytes、10,000 entry objects、1,000,000 JSON values、八层 object/array、1,048,576-byte string token 和 128-byte number token，拒绝 duplicate object key、invalid JSON、unknown field、wrong type 和 invalid metadata；整个 batch 要么提交、要么不改变 durable Library，alias 或同 batch canonical duplicate 均以规定的 `internal_duplicate` conflict rollback，dry-run 与未变基线上的实际 import 报告相同计划。

  Revision 5 还要求六种 ceiling 以 API-v2 独立 code `library_input_limit_exceeded` 返回 `LimitDetails` 的 first exceeded dimension、无损 measured/allowed decimal 值和 input `PathValue`；该 code 不得重用 API-v1 仅适用于 Agent project-input 的 `agent_input_limit_exceeded`。首次 import 在 data-directory descriptor 内 no-clobber 发布 staging database；`COMMIT` 前失败不报告成功，且只清理仍可由 held descriptor 证明的 staging entry/live link，无法证明 provenance 或 identity 的 database、sidecar、lock 与 directory 可以保留，失败不得声称 state absence；commit 后 durability-sync failure 同样返回错误且不伪称 state 未改变。

Revision 4 的 export 与 Revision 5 的 import 共同要求“同一可移植文档”与严格 input ceiling：完整的 P2 durable Library 必须能被当前 import 读取，import 与 dry-run 必须在 mutation/result 前同时检查 post-import deterministic document 的 10,000-entry 与 67,108,864-byte transfer 上限，export 也必须在 staging 前执行相同检查。它是既有单一 transfer format 的实现闭环与 defect 修复，不引入新命令、字段、API code。

导入文件中的 `ResolvedSkill`、`SourceIdentity`、完整 SHA、完整性摘要、已验证名称、描述和计数必须满足 API-v2 的可移植表示。为防止损坏的本地记录，本交付会复用 `SKL-SRC-002`、`SKL-SRC-007` 与 `SKL-SRC-012` 的 canonical source、名称与摘要约束，并对 alias/category/tag/note 执行 `SKL-LIB-008` 的大小、Unicode 15.1.0、NFC、`White_Space` 裁剪和 C/F 完整默认大小写折叠规则。这些约束的局部复用不表示来源获取、直接元数据命令或完整 Source/Library 行为已经完成；`SKL-SRC-*`、`SKL-LIB-001`、`SKL-LIB-004`、`SKL-LIB-005`、`SKL-LIB-008` 和 `SKL-LIB-011` 仍保持 planned，直到各自完整 acceptance 被独立交付。

Revision 2 的 `SKL-CLI-004`、`SKL-CLI-005` 与 `SKL-CLI-012` 以 API-v2 current-producer cutover 的最小必要范围加入本交付；它不增加命令、双版本协商或 API-v1 compatibility mode。其余 `SKL-CLI-*`、`SKL-OPS-*` 不在本次完成基线中。本交付遵守适用约束：JSON stdout 只写一个 API-v2 信封、常见成功结果正确区分 observed/changed/unchanged、路径用 `PathValue`、读和 dry-run 不联网且不创建 skilload 根、导入写入仅在完整验证之后发生；未知较高 schema 拒绝写入，已识别的数据库损坏绝不被静默替换且必须返回 `database_corrupt` 的 `DatabaseCorruptDetails`。P2 不创建备份或导出位置索引，因此该诊断如实返回空 `backups` 和 `recoverable_exports` 集合、数据库 `PathValue` 与 `database-corruption-v1`；但不会宣称这些跨全产品行为的全部 acceptance 已满足。

完成时的可观测证明是：用户先对合法 regular-file 导入文件运行带 `--dry-run --json` 的命令，得到 `library.import` 的 `observed` 结果且 XDG data/state 根仍不存在；再运行实际导入，得到 `changed` 或 `unchanged`，只建立所需的 data SQLite 文件与写锁；运行 export 后得到确定性 `LibraryExportData` 文件。重复导入不重写数据库；混入无效条目、重复 JSON 键、超限输入、非常规 input、重复 canonical source 或 alias 冲突的批次不产生部分条目或持久写入。首次 import 的 `COMMIT` 前注入失败返回错误且不报告 success；data/state root、durable lock、随机 staging sidecar 或 race replacement 可以保留，只有仍与 held descriptor 一致的 staging entry/live link 才可清理，测试不得断言 state absence。commit 后 sync 失败不报告成功或 absence。export 拒绝 database generation、rollback journal 或 lock target；rename 前输出失败保留旧 target，而 rename 后父目录 sync 失败返回错误且新 target 可能已发布。对损坏数据库的 import/export 返回带路径、空 P2 已知恢复集合和 `database-corruption-v1` 的 `database_corrupt`，并保持原文件及持久状态不变。

任何 P2 已接受的完整 Library 都能导出为不超过 10,000 entries、67,108,864 bytes 的 deterministic `LibraryExportData`，随后由同一二进制重新 import；试图通过多次 individually valid import 累积超过任一 bound 的 batch 在 mutation/plan result 前以 `validation_failed` 的 `library_portable_document_entries` 或 `library_portable_document_bytes` constraint 失败。首次 import 在 lock 内发现另一 importer 已发布 database 时，以同一 document 重新规划 existing state 并正常序列化；staging basename 在 SQLite open 前后都必须绑定到 held file，export 最终 rename 失败清理原 staging 与 publication link 而不触碰未知 replacement。

## Design and Architecture Inputs


`ARCHITECTURE.md` 要求 `skilload-core` 保持可复用 domain/application/ports/adapters 分层，CLI 只负责参数、调度与投影，产品变更由应用服务经显式端口提交。Library 元数据是 durable SQLite 的所有者，外部 Skill 字节、Trust、workspace 文件和缓存不是本交付数据库的替代或副本。有效的 config/data/state/cache application root 必须继续通过现有 `StateRootResolver` 同时解析、检查分离和在写前重新验证。

`docs/design-docs/application-and-persistence.md` 已指定 `data/skilload.db` 为 durable 数据库、查询缺失状态时返回内存空视图、写入仅在输入验证达到持久阶段后创建数据库、以及 Library export 不携带本机数据库行 ID 或操作时间。本交付采用该方向，但只创建 v1 的 `schema_info`、`state_revision`、`library_entries` 和 `library_tags` 最小表；不得假装 Trust、global、profile、workspace、owned link、confirmation token 或 FTS 已有真实业务所有者。

`docs/design-docs/cli-json-and-release.md` 规定每个已注册叶子只映射一个应用请求，CLI 不自行编排仓库调用；本分支已将可移植传输参数澄清为 `--input <PATH>`、`--output <PATH>`，使文件中只有可导入数据，而命令结果仍保留正常 API-v2 信封。`docs/design-docs/application-and-persistence.md` 还要求 P2 以 no-follow、nonblocking input descriptor 维持 scanner resource bound、以 staging database 避免首次失败发布 partial state，并区分 rename 前与 rename 后的 export sync failure。`docs/references/rust-sqlite-unicode-library-foundation.md` 记录了本交付的依赖事实：使用无默认特性的 `rusqlite 0.40.2` 加 `bundled`，以及精确 `unicode-normalization =0.1.23`；后者的表是 Unicode 15.1.0，而当前较新版本是 Unicode 17.0.0，不能使用。

本轮 `review` 内 ordinary remediation 继续遵守这些输入：完整 portable document 的 encoder/validation 同时强制 entry 与 byte transfer ceiling；`ResolvedSkill` 只接受正 evidence count；SQLite adapter 验证 tags table 的唯一 cascade foreign key、将 foreign-key parent-key mismatch 归类为损坏，并在 first staging 与 existing import/export/dry-run `Connection::open` 后、任何 configure/SQL 前以 narrowly audited `SQLITE_FCNTL_HAS_MOVED` FFI 检验 connection 实际 inode；transfer adapter 和 first import 都在 publication-link hook 后、rename 前重验 held staging FD，first import 在全部 sync 后重验 live database entry。`LibraryImportOperation` 以领域 `Observed` outcome 表达 dry-run，CLI 只投影该结果。它们只修复已写明的 P2 atomic transfer、durable corruption、并发、presentation-neutral outcome 与可移植闭环，不触发 review-to-active 逆向事务。

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
- [x] (2026-08-19 12:46Z) 已逐一回复并关闭四个新增 thread；完整 `list --all` 显示 6 条 top-level 触发评论、63 个 review body、51 个 inline thread，所有 thread 均为 resolved，初始 thread source 没有漏记，top-level/review body 均无独立问题。四条 reply URL 和 `thread resolved: true` 已写入下列 ledger；待提交并推送最终 reconciliation。
- [x] (2026-08-19 13:29Z) 已完整读取 PR #3 的 7 条 top-level trigger、9 个非空自动 review body 与 57 个 inline thread；前两类没有独立问题，51 个既有 source 仍 resolved，新增 6 个 source 已按当前正文、路径、thread state 和 Product Baseline 分类：一个 existing-database ABA concern 为 no-fix/open，五项 first-staging identity、portable evidence、complete entry ceiling、tags foreign key 与 export final-sync concern 为 fixed/open，均属 `review` 内 ordinary remediation。
- [x] (2026-08-19 13:39Z) 已在 `review` 状态完成五项 ordinary remediation 及产品/架构/设计/reference 同步：first-staging SQLite connection 的 post-open inode 验证、positive resolved evidence、combined 10,000-entry transfer ceiling、tags foreign-key schema corruption 与 export final-sync output revalidation。focused core 83 tests、`cargo fmt --all --check`、workspace Clippy `-D warnings`、locked all-features workspace tests（11、12、83）和 workspace build 均通过；待审阅 diff、推送 preliminary commit、逐 thread reply/resolve。
- [x] (2026-08-19 13:40Z) code 与 preliminary Review Conversation Log commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送；local、upstream 与 open/ready PR #3 的 `headRefOid` 均为该 SHA。六个 source 保持 open，下一步重新读取会话、逐一回复并关闭 inline thread。
- [x] (2026-08-19 13:45Z) 已重新获取完整会话：7 条 top-level trigger、70 个 review body 与 57 个 inline thread 中，top-level 仅为 `@codex`，所有非空 review body 都是无独立问题的 generic automation；57 个实际 source 与 57 个 Plan heading 完全对应、无 missing/stale source、无 unresolved thread。六个本轮 source 均已有 GitHub reply URL 和 `thread resolved: true`，待提交并推送最终 Review Conversation Log reconciliation。
- [x] (2026-08-19 14:50Z) preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；local、upstream 与 open/ready PR #3 的 `headRefOid` 均为该 SHA。九个 source 保持 fixed/open，本 Plan 的 pushed-code evidence 已写入每项；下一步重新读取完整会话、逐线程回复并关闭，再提交最终 ledger reconciliation。
- [x] (2026-08-19 14:56Z) 已在 `list --all` 的最终会话读取中核对 8 条 top-level trigger、84 个 review body 和 66 个 inline thread：66 个 Plan source 全部覆盖、无 unlogged/missing source、无 unresolved thread；top-level 仍仅为 `@codex`，非空 review body 均为 generic automation。九个新 source 已逐一回复并在 reply 成功后关闭；本 Plan 已记录每个 reply URL（超时重叠调用留下的额外回复也完整保留），待提交/push 最终 ledger reconciliation。
- [x] (2026-08-20) 完整读取当前 PR 会话的 9 条 top-level trigger、85 个 review body 与 71 个 inline thread；前两类没有独立问题，66 个既有 ledger source 仍 resolved，5 个新 source 已按 source、路径、文本与 thread state 登记。
- [x] (2026-08-20) 在 `review` 状态完成四项 ordinary remediation：reversible export/database publication exchange、GitHub owner grammar、SQLite commit-failure sidecar ownership；新增 focused regressions，并通过 core 94 tests、fmt、Clippy、locked workspace tests（11、12、94）、build、CLI import/export smoke 与 diff check。
- [x] (2026-08-20T05:52Z) 人类选择修订恢复合约，而不授权跨 macOS/Linux native/FFI create-and-hold primitive；`SKL-LIB-010` 将升至 Revision 5，首次 import 的 pre-COMMIT failure 不再承诺 data/state roots absence，未知 provenance residual 必须保留。
- [x] (2026-08-20) 四个 fixed source 已使用 `752c0f77b24a5300dffe7edcca952809688fdc1f` 的实现/验证证据逐一回复并在 reply 成功后关闭；`PRRC_kwDOT7YN2s7jWsuw` 已收到 exact decision question，保持 blocked/open。
- [x] (2026-08-20) 四个 ordinary remediation 已由 `282fd97dcd04dea37d0ff30848ecd26be603937f` 推送并逐源回复、关闭；最终读取确认 10 条 top-level trigger 均为 `@codex`、96 个 review body 无独立问题、75 个 inline source 全部有 Plan heading，74 个已解决。唯一 `PRRC_kwDOT7YN2s7jWsuw` 因未决 create-and-hold 方向维持 pending/blocked/open。
- [x] (2026-08-20) 已完整读取 PR #3 当前 11 条 top-level trigger、13 个非空自动 review body 与 80 个 inline thread；前两类没有独立问题，75 个既有 inline source 仍与 Plan log 对应。两个 ordinary export remediation 已由 `0892f3ea7b515f6bdd0f8e371516af71eb390c9a` 推送：focused portable tests 19/19、fmt、Clippy、locked workspace tests（11、12、100）与 build 通过，实际 CLI API-v2 export smoke 证明 symlink-parent `..` 写入 kernel 解析的路径。五个新 source 均已获得 GitHub reply；两个 fixed 与一个 no-fix thread 已关闭，两个 held-file/sidecar provenance source 与既有 directory identity source 均保持 pending/blocked/open，等待人类决定。
- [x] (2026-08-20) 本轮完整会话读取发现 `PRRC_kwDOT7YN2s7jjnNR`、`PRRC_kwDOT7YN2s7jjnNS` 与 `PRRC_kwDOT7YN2s7jjnNU` 三个新的 inline source；均在当前 P2 export/import boundary 内。ordinary remediation、产品/设计/Plan 同步与 focused/core regression 已由 `0dc0b9b3f83ef256c4de19c23186ed9c3816f826` 推送；该提交通过 portable 21、SQLite 39、core 102、`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、102）、workspace build 与隔离 CLI export → dry-run import smoke。三个 fixed source 已逐一回复并在 reply 成功后关闭；三个既有 provenance/directory 决策 source 保持 pending/blocked/open。
- [x] (2026-08-20) 完整读取当前 PR #3 会话：14 条 top-level comment 都是 `@codex`，108 个 submitted review 为空或通用 automation，90 个 inline thread 中 83 个既有问题 source 仍由 Review Conversation Log 覆盖；7 个新增 open source 已按当前代码、产品规范与 thread state 分类。
- [x] (2026-08-20) 已在本地完成六项 `review` 内 ordinary remediation：export serializer 完整 validation、existing-output exchange 后保留 publication replacement、Busy/schema display domain、non-array `entries` schema 分类、以及 per-root created-directory sync attribution；focused regressions 已通过，待完整 gates、preliminary commit/push、GitHub reply/closure。
- [x] (2026-08-20) 六项 local remediation 的 focused regressions、`cargo fmt --all --check`、workspace Clippy `-D warnings`、locked workspace tests（11、12、107）、workspace build 与隔离 XDG CLI dry-run/import/export byte-identical round trip 均通过；`entries: null` CLI JSON error 为 `library_import_schema`。已审阅完整 diff 且 `git diff --check` 通过，待 preliminary commit/push。
- [x] (2026-08-20) code/preliminary ledger commit `4ef1ba205eb323c702bceda830445f44feb4da46` 已推送；local、origin 与 ready PR #3 `headRefOid` 均为该 SHA。六项 fixed source 现有 commit/validation evidence，待重新获取会话、逐源回复并关闭。
- [x] (2026-08-20) 六个 fixed thread 已以 `4ef1ba205eb323c702bceda830445f44feb4da46`、focused/full validation evidence 回复并在 reply 成功后确认 `isResolved: true`；`PRRC_kwDOT7YN2s7jlNzR` 已收到精确产品/架构决策问题并保持 blocked/open。此次 final ledger commit 后必须再次全量读取会话和 PR head。



- [x] (2026-08-20T05:52Z) `PRRC_kwDOT7YN2s7jWsuw`、`PRRC_kwDOT7YN2s7jiT5X`、`PRRC_kwDOT7YN2s7jiT5i` 与 `PRRC_kwDOT7YN2s7jlNzR` 的人类决策已统一为 Revision-5 recovery contract；PR 已恢复为 Draft，Plan 正在回到 `active`，四个 thread 保持 open，直至实现、验证、回复和 closure 完成。

- [ ] (2026-08-20T05:52Z) 在 `active` 实现 Revision 5 的保守 pre-COMMIT cleanup：不再以 pathname/first-observed FD 推断 directory 或 SQLite sidecar ownership，更新产品/持久化设计/reference/测试并重新完成 ready/review transaction。

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
- Observation: bundled SQLite 的 Unix VFS 在非空 main database 进入写 journal 前会自动调用 `SQLITE_FCNTL_HAS_MOVED`，因此 existing-database ABA open 后还原 pathname 时写入返回 `SQLITE_READONLY_DBMOVED` 而不是提交到被替换 inode；zero-length first-import staging 则因 `dbSize == 0` 跳过该内部检查，必须在任意 SQL 前显式验证 connection。
  Evidence: locked `libsqlite3-sys 0.38.2/sqlite3.c` 的 `databaseIsUnmoved`（63747–63763）与 journal-open call site（65474）；本机 ABA probe 对两个非空 database 返回 `ReadOnly` extended code 1032，A/B entry count 均保持 1。

- Observation: SQLite 只在写 journal 前自动执行 nonempty database 的 moved check；read-only export/dry-run 因而不能依赖该写时保护。
  Evidence: `export_rejects_a_read_only_database_aba_open` 在 open 窗口将 connection 指向 B、随后恢复 pathname A；新的显式 `SQLITE_FCNTL_HAS_MOVED` check 在任意 SQL 前返回 `database_identity_drift`，focused core 89 tests 通过。
- Observation: staging basename 的 `-journal`、`-wal` 与 `-shm` 名称不能证明 sidecar 是本调用创建的。
  Evidence: `first_import_precommit_failure_preserves_foreign_staging_sidecar` 在 pre-COMMIT failure 前创建同名 foreign `-shm`，Drop 保留该文件且不删除含它的 data directory。

- Observation: final pathname revalidation 不能撤销 publication source 在随后的 `rename` syscall 前被替换所造成的覆盖。
  Evidence: `export_restores_the_old_output_when_publication_changes_after_final_check` 与 `first_import_restores_absence_when_publication_changes_after_final_check` 证明 `RenameFlags::EXCHANGE` 的 post-exchange identity check 能反向交换并恢复旧 output/absence；`cargo test -p skilload-core` 的 94 tests 通过。
- Observation: POSIX/Darwin/Linux `mkdir`/`mkdirat` 不返回新目录 descriptor，因此 `create_dir` 成功与第一次 pathname metadata/open 之间不能证明该 inode 仍是本调用创建的目录。
  Evidence: `crates/skilload-core/src/adapters/configuration.rs:601-660` 是 `create_dir → symlink_metadata → open` 顺序；同账号替换为空目录可在第一次 identity 采集前发生。该事实与 `ARCHITECTURE.md` invariant 5 和 P2 absent-root cleanup 同时存在时需要人类决定，不能用更多 pathname stat 修复。

- Observation: `rustix 1.1.4` 的 `RenameFlags::NOREPLACE` 在 Linux 映射 `RENAME_NOREPLACE`、在 Apple 映射 `RENAME_EXCL`，可在 held parent descriptor 内将 hidden publication link no-clobber 地发布到 originally absent output。
  Evidence: locked `rustix` platform `RenameFlags` 与 Apple `renameatx_np` backend；`export_keeps_an_absent_output_absent_until_no_clobber_publish`、`export_preserves_native_symlink_parent_dotdot_semantics`、focused portable 19 tests 和实际 CLI smoke 均通过。
- Observation: `linkat` 的 source 名称与 sidecar 的 first-observed FD 都无法在同账号 rename/create race 下证明 creation provenance。
  Evidence: `link_to_absent_database` 的 `linkat` source 仍为 `self.name`，`record_owned_sidecars` 在观察后才记录；`PRRC_kwDOT7YN2s7jiT5X` 与 `PRRC_kwDOT7YN2s7jiT5i` 需要产品/architecture 决定，不能用额外 stat 伪装解决。
- Observation: 活跃 DELETE-mode SQLite writer 的 `skilload.db-journal` 不在既有 export protected-path set；以该路径作为 output 会破坏 rollback recovery。
  Evidence: `output_refuses_a_live_delete_mode_rollback_journal_before_staging` 保持实际 open transaction 的 journal 不变，并确认无 export staging file；这使 `SKL-LIB-009` 必须升至 Revision 4。
- Observation: 首次 import 的失败 cleanup 在 explicit unlock 后 unlink `database.lock`，会把已经打开旧 inode 的等待者与重建 pathname 的后续 contender 分到不同锁域。
  Evidence: `first_import_precommit_failure_retains_the_durable_lock` 证明失败后下一 import 重用同一 inode；`first_import_post_lock_failure_retains_the_durable_lock` 覆盖获得 lock 后立即失败的路径。
- Observation: `read_input` 的完整 `Vec` materialization 位于 `JsonScanner` 前，导致早期 token ceiling 没有机会阻止后续 input chunks。
  Evidence: `scanner_stops_reading_at_first_streamed_number_overage` 在第 129 个 number byte 返回 `library_import_number_bytes`，并证明 scanner 不请求第二个 64 KiB input chunk。
- Observation: `SKL-LIB-010` Revision 4 的 pre-COMMIT absent-root acceptance 与 durable `database.lock` 的 stable-inode concurrency 规则无法同时由当前安全 filesystem API 证明。
  Evidence: `docs/product-specs/library.md:80` 要求 data/state roots 恢复 absent；`docs/design-docs/application-and-persistence.md:170` 要求一旦 contender 可能打开 lock 就不得 unlink/recreate；`sqlite_library.rs` 持续保留 lock 以避免 inode split。
- Observation: existing-output exchange 后的 publication pathname 不能安全地执行 check-then-unlink。
  Evidence: `OutputPublicationGuard::matches` 与 `unlinkat` 是独立 pathname syscall；同账号可在两者之间安装 foreign replacement。`export_preserves_a_replaced_publication_entry_after_exchange` 在该点植入 replacement 并验证它未被删除。
- Observation: 四个 pending review source 揭示安全 Rust/macOS/Linux 接口不能同时证明首次创建 directory、staging source、SQLite sidecar 与 durable lock 的 provenance，并维持 Revision 4 的 pre-COMMIT absent-root assertion。
  Evidence: 2026-08-20T05:52Z 的人类选择将 `SKL-LIB-010` 升至 Revision 5，接受保守保留未知 residual，而不授权 native/FFI primitive。


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


- Decision: export 与 first-import publish 以 final target snapshot、only-if-absent guard（absent destination）、held publication link 和 `RenameFlags::EXCHANGE` 组成可逆发布事务，而不在最终 check 后直接 `rename` pathname。
  Rationale: `EXCHANGE` 保留旧 output 或 absence guard；post-exchange 只有同时匹配 held staging/guard identity 才完成，否则反向交换恢复 target。该设计只使用锁定 `rustix 1.1.4` 的安全 descriptor-relative API，满足 macOS/Linux shared implementation，且不会把检测放在不可逆覆盖之后。
  Date/Author: 2026-08-20 / Codex

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
- Decision: 对 first staging、existing import、export 与 dry-run 的 SQLite connection 都只在 `sqlite_library.rs` 的窄 helper 中调用 bundled `sqlite3_file_control(..., SQLITE_FCNTL_HAS_MOVED, ...)`；core crate 维持 `deny(unsafe_code)`，仅该具备安全论证的 helper 局部允许 FFI。
  Rationale: `rusqlite` 没有安全的 connection-file identity API；empty staging 跳过 SQLite 的自动 moved check，read-only connection 也不会进入写 journal 路径。helper 在任何 configure/SQL 前以 SQLite 自身持有的 main-file handle 检验 pathname；局部审计边界比放宽全 crate 或依赖可替换 `/dev/fd` pathname 更小、更可验证。
  Date/Author: 2026-08-19 / Codex
- Decision: 将 positive `ResolvedSkill` counts 与完整 10,000-entry transfer ceiling 视为 Revision 3/4 已有的可移植证据/唯一 transfer-format 闭环澄清，不增加行为 revision。
  Rationale: valid source 必含非空 `SKILL.md`，零 entry/byte count 不是可解析的 resolved evidence；P2 import scanner 已拒绝第 10,001 entry，先前只在单次输入而未在 combined durable document 强制该同一上限，使本二进制能够产生自身拒绝的 export。修复使用既有 `validation_failed` constraint，不新增 API code、字段或命令。
  Date/Author: 2026-08-19 / Codex
- Decision: SQLite sidecar cleanup 只记录 SQLite `COMMIT` 返回 failure 后以 `O_NOFOLLOW` FD 绑定的 matching `-journal`/`-wal`/`-shm`，不把 pre-commit hook 或未知同名 sidecar 归为 owned。
  Rationale: confirmed SQLite failure 后的 recorded identity 允许恢复本调用遗留 state；没有 SQLite operation evidence 的 basename 仍不能证明所有权。保留 unknown sidecar 比删除 foreign path 更符合 invariant 5。
  Date/Author: 2026-08-20 / Codex
- Decision: `LibraryImportOperation` 使用三态领域 `LibraryImportOutcome`，由 repository 产生 `Observed` dry-run outcome，presentation adapter 不得从 result data 重算。
  Rationale: future CLI/TUI/Web interface 必须复用 application result；将 outcome 写入 domain operation 防止第二个 adapter 错误报告 `unchanged`，且不改变 API-v2 既有 `observed` wire semantics。
  Date/Author: 2026-08-19 / Codex
- Decision: 本轮九项 source 保持 `review` 内 ordinary remediation，不执行 review-to-active 逆向事务。
  Rationale: 每项修复实现现有 Product Baseline 已明确的 no-foreign-mutation、durable corruption、pre-COMMIT cleanup、presentation-neutral outcome 或 current-producer status；没有扩展命令、行为 revision、API schema 或 acceptance scope。
  Date/Author: 2026-08-19 / Codex

- Decision: `PRRC_kwDOT7YN2s7jWsuw` 保持 `pending`/`blocked`，不以额外 pathname revalidation 伪装解决 directory creation identity race。
  Rationale: 在当前 macOS/Linux safe API 与 `deny(unsafe_code)` 边界内，`mkdir` 没有可同时返回新 inode descriptor 的操作；接受 foreign adoption/removal 或放弃 absent-root recovery 都是产品/architecture tradeoff，需人类明确选择。
  Date/Author: 2026-08-20 / Codex

- Decision: first import 直接以 held staging inode 的 descriptor-relative `linkat` 创建缺失的 live database；export 保持其既有 guard/exchange 事务。
  Rationale: `skilload.db` 在 link 成功前保持 absent，成功时已是完整 committed generation；target 已存在时 `linkat` 的 no-clobber failure 保留 foreign database。它消除 zero-byte live guard 和随机 publication link 的崩溃/并发读取窗口，同时保留 held-identity cleanup 与 drift detection。
  Date/Author: 2026-08-20 / Codex
- Decision: absent main database 的 SQLite sidecar 统一报告 `database_corrupt`；staging sidecar 仅在 SQLite persistence operation 的 error exit 或 pre-COMMIT rollback 前按 held FD identity 记录；portable tag sort 先完成全部 fallible normalization。
  Rationale: `-journal`、`-wal` 或 `-shm` 不得与 empty Library 混同；pre-commit hook 之后未知的同名 sidecar 仍不具备 ownership proof。预计算 comparison key 将 public fallible API 的 invalid tag 路径改为 validation error，并避免 comparator panic 与部分 document mutation。
  Date/Author: 2026-08-20 / Codex

- Decision: 对 absent Library export target 改用 hidden publication link 加 `RenameFlags::NOREPLACE`，既有 regular target 保持 `RenameFlags::EXCHANGE` 的 reversible replacement。
  Rationale: no-clobber publish 让 requested name 在完整 document ready 前保持 absent，同时仍通过 held parent descriptor 操作；existing target 仍需要 exchange 才能恢复被 final race 替换的旧内容。该修复实现 `SKL-LIB-009` 的既有 atomic/no-target failure 行为，不增加命令、API 字段或行为 revision。
  Date/Author: 2026-08-20 / Codex
- Decision: `PRRC_kwDOT7YN2s7jiT5X` 与 `PRRC_kwDOT7YN2s7jiT5i` 保持 pending，`PRRC_kwDOT7YN2s7jiT5n` 保持 no-fix。
  Rationale: 前两项在当前跨 macOS/Linux 安全接口与 foreign-path invariant 下需要人类选择 provenance primitive 或 cleanup guarantee；后者要求为 portable export destination 发出 API-v2 尚未定义的 `TargetRef` scope，不能在 review 中发明 schema 语义。
  Date/Author: 2026-08-20 / Codex

- Decision: 将 `SKL-LIB-009` 从 Revision 3 提升为 Revision 4，并把活跃 DELETE rollback journal 作为 export protected target。
  Rationale: 指向 `skilload.db-journal` 从可写变为拒绝是可观察的安全语义，不能伪装为 Revision 3 的文字澄清；它只补全既有“不得破坏活动 database generation”的 export boundary，不新增命令、API 字段或独立 acceptance 范围，因此保留 `review` 的 ordinary remediation 流程。
  Date/Author: 2026-08-20 / Codex

- Decision: 首次 import 的失败 cleanup 保留已建立的 `database.lock` pathname，且 restrictive lock helper 不再删除新创建的 lock。
  Rationale: filename 是并发 contender 的唯一稳定协调 identity；unlock 后 unlink 会允许旧 inode 的等待者与新 pathname 的锁持有者并行。空 lock 不含 Library 数据且可由后续 import 重用，保留它比把所有 state root 恢复为 absent 更符合 durable mutation serialization。
  Date/Author: 2026-08-20 / Codex

- Decision: JSON non-model pass 直接驱动 held regular-file 的 buffered reader，并只在 scan 成功后把收集的 bytes 交给 schema deserialization。
  Rationale: 这使第一个 string/number/depth/value/entry/file-byte ceiling 在读取流中终止，而不是先 materialize 至 byte ceiling；保留既有 scanner 的 duplicate-key、UTF-8、depth 和 measured/allowed error contract。
  Date/Author: 2026-08-20 / Codex

- Decision: `PortableLibraryDocument::serialize_for_transfer` 在编码前执行完整 domain validation。
  Rationale: export 成功必须表示同一 binary 可以 import 的唯一 portable format；只检查 entry/byte ceiling 和 tag sort 无法覆盖公开字段的 format、metadata 或 duplicate-source violation。
  Date/Author: 2026-08-20 / Codex
- Decision: successful existing-output `RenameFlags::EXCHANGE` 保留 hidden publication entry，而非在 identity check 后 unlink。
  Rationale: POSIX/Darwin/Linux 没有将 unlink 条件绑定到 held inode 的 primitive。保留 superseded output/foreign replacement 符合 `ARCHITECTURE.md` invariant 5，避免 check-then-unlink 删除未知 path；这不改变请求 output 的 portable document 或 API shape。
  Date/Author: 2026-08-20 / Codex
- Decision: Busy、SchemaNewer 与 MigrationRequired display text 使用其已有 domain fields；root `entries` 仅在其 value 为 array 时使用 entry ceiling scanner；首次 import 按每个 created directory 的 owning XDG variable 同步。
  Rationale: 这些修复分别让 API-v2 `error.message` 与 structured details 一致、让 valid JSON wrong type 交给 schema layer、并让 environment sync failure 指向实际 root；均不改变 Product Baseline、API schema 或 command surface。
  Date/Author: 2026-08-20 / Codex
- Decision: `PRRC_kwDOT7YN2s7jlNzR` 保持 pending/blocked，不把 prior durable-lock implementation decision 视为对高优先级 `SKL-LIB-010` acceptance 的隐式修订。
  Rationale: 修改产品 revision/acceptance 是重大行为取舍，需人类决定；盲目 unlink/recreate lock 会违反 stable coordination identity，现有跨 macOS/Linux primitive 不能证明同时保留 foreign-path safety 与 absent-root recovery。
  Date/Author: 2026-08-20 / Codex
- Decision: 人类选择将 `SKL-LIB-010` 修订为 Revision 5 的保守 recovery contract，不引入新的跨 macOS/Linux native/FFI create-and-hold primitive。
  Rationale: 当前安全 API 无法证明 directory、staging source 或 sidecar 的 creation provenance。未知 residual 的保留和明确 error 比删除或宣称 absence 更符合 foreign-path safety；实现必须移除基于 pathname 或 first-observed FD 的 cleanup 推断。
  Date/Author: 2026-08-20T05:52Z / 用户确认、Codex 记录


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

2026-08-19 12:46Z 的最终 GitHub reconciliation 确认四条新增 source 均已回复并关闭：`PRRT_kwDOT7YN2s6adTiE`、`PRRT_kwDOT7YN2s6adTiN`、`PRRT_kwDOT7YN2s6adTiT` 与 `PRRT_kwDOT7YN2s6adTiW` 均为 `isResolved: true`。完整会话没有未记录 initial source 或未解决 thread，6 条 top-level `@codex` 触发评论和 8 条非空自动 review body 均没有独立问题。下一步为本次 final Review Conversation Log documentation commit、push 和最后一次完整重读。

2026-08-19 13:39Z 的第五轮 ordinary remediation 已完成本地实现和验收。zero resolved count 现在在 domain constructor 被拒绝；complete durable document 同时受 10,000-entry/byte transfer limits；missing `library_tags` foreign key 是 `database_corrupt`；export 在 final parent sync 后重新证明 output 仍是 held staging inode。first import 的 zero-size ABA window 使用 bundled SQLite main-file `HAS_MOVED` FFI 在所有 SQL 前验证实际 connection，局部 audited exception 之外仍拒绝 unsafe code。core 83 tests、format、Clippy、locked workspace tests（11、12、83）和 build 通过；待 preliminary commit/push、GitHub reply 和 closure。

2026-08-19 13:40Z 的 code/preliminary ledger commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送；local、upstream 与 open/ready PR #3 head 已核对一致。五个 fixed source 的具体路径、回归和完整 validation 已在本 Plan 记录，no-fix source 保留 SQLite source/probe 依据；六个 target thread 仍 open，下一步只进行 GitHub reply/resolve 和最终 reconciliation。

2026-08-19 13:45Z 的 final pre-documentation reconciliation 确认：7 条 top-level trigger 与 70 个 review body 都不含独立问题；57 个 inline problem source 与 57 个 Plan heading 一一对应，全部 resolved。六个本轮 reply URL、`8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 代码证据和验证已写入 ledger；下一步提交/push 本 Plan final reconciliation 后，再执行最后一次完整会话与 PR head 检查。

2026-08-20 的本轮 review remediation 已由 `752c0f77b24a5300dffe7edcca952809688fdc1f` 推送，完成四项现有 P2 contract fix：export 与 first import 都以 reversible `RenameFlags::EXCHANGE` publish protocol 保留/恢复原 target，portable owner 拒绝 impossible GitHub login，SQLite commit-failure sidecar 只按 recorded FD identity 清理。core 94 tests、fmt、Clippy、locked workspace tests（11、12、94）、build、实际 CLI import/export smoke 与 diff check 均通过。`PRRC_kwDOT7YN2s7jWsuw` 仍是未决 architecture/product choice；它保持 `pending`/`blocked` 和 open thread，不把目录 create-and-hold race 误报为完成。


2026-08-20 的 reply reconciliation 显示 9 条 top-level trigger 与 90 个 review body 没有新增独立问题；71 个 inline source 全部在 Review Conversation Log 中有 heading。四个本轮 fixed source 已回复并 `thread resolved: true`；唯一 `PRRT_kwDOT7YN2s6ahTLF` 以 exact decision question 回复，保持 `pending`/`blocked` 与 open 状态。最终 documentation commit 推送后仍需再次全量读取会话和 PR head。

2026-08-20 的最新 ordinary remediation 已由 `282fd97dcd04dea37d0ff30848ecd26be603937f` 推送：`database_exists` 拒绝 orphaned sidecar，first-import staging 在 SQLite error/rollback path 记录 held sidecar，live database 仅由 direct `linkat` 发布，`PortableLibraryDocument` 的 tag sort 传播 validation error；`application-and-persistence.md` 与 Rust/SQLite reference 已同步。core 98 tests、format、Clippy、locked workspace tests（11、12、98）、build 和双隔离 XDG CLI dry-run/import/export byte-identical round trip 均通过。最终会话读取显示 10 条 top-level trigger 均为 `@codex`、96 个 review body 均为空或通用自动化文字、75 个 inline source 均有 Plan heading；四个 fixed thread 均已回复并 resolved，directory identity source 仍按缺少人类决定保持 pending/blocked/open。

2026-08-20 的本轮 local remediation 移除 absent export output 的 zero-byte guard，改以 hidden publication link 和 `RenameFlags::NOREPLACE` 只在完整 document ready 时发布；同时停止词法折叠用户 native output path 中位于 symlink 后的 `..`。`rust-sqlite-unicode-library-foundation.md` 与 `application-and-persistence.md` 已同步该协议及 first-import/sidecar provenance 的未决限制。focused portable 19 tests、fmt、Clippy、locked workspace tests（11、12、100）、build 和隔离 CLI API-v2 smoke 均通过。`PRRC_kwDOT7YN2s7jiT5X`、`PRRC_kwDOT7YN2s7jiT5i` 保持 pending，`PRRC_kwDOT7YN2s7jiT5n` 有 product-contract no-fix rationale；下一步提交/push preliminary ledger 后逐 source 回复并关闭可处理 thread。

2026-08-20 的 reply reconciliation 已确认五个本轮 source 都有 GitHub 回复：absent-target 与 native-path fixed thread、error-category no-fix thread 均在 reply 成功后 `isResolved: true`；first-import publication source identity 与 sidecar provenance 分别收到明确的 primitive-or-guarantee 决策问题，并与先前 directory identity source 一同保持 `pending`/`blocked`/open。最终 Plan ledger commit 推送后必须重新读取全量会话与 PR head，确认 80 个 source 的记录、回复与 thread state 一致。

2026-08-20 的本轮 remediation/preliminary ledger commit `0dc0b9b3f83ef256c4de19c23186ed9c3816f826` 已推送，local、upstream 与 open/ready PR #3 head 已核对一致。它让首次 import failure 不再 unlink durable database lock，export 在 staging 前拒绝活跃 DELETE rollback journal，并让 non-model JSON scanner 从 held regular-file buffered reader 增量运行，在第 129 个 number byte 停止而不请求后续 chunk。`SKL-LIB-009` 因新增可观察的 journal rejection 升至 Revision 4。focused portable（21）、SQLite（39）、core（102）、format、Clippy、locked workspace tests（11、12、102）、workspace build 与隔离 CLI export → dry-run import smoke 均通过。三个 fixed source 已获 GitHub reply 并确认 `thread resolved: true`；三个既有 provenance/directory source 保持 pending/blocked/open，待最终 Plan reconciliation commit 推送后再完整读取会话与 PR head。

2026-08-20 的当前 local remediation 已完成六项 ordinary P2 review fix：transfer serializer 以完整 domain validation 保证 export/import closure；existing-output exchange 保留 publication replacement，避免 check-then-unlink 删除 foreign path；API-v2 error display 采用 Busy/schema 的实际 domain；scanner 将 valid JSON 的 wrong `entries` type 留给 schema；first-import created-directory sync 使用正确 XDG root。focused regressions、format、Clippy、locked workspace tests（11、12、107）、build、CLI round trip 和 diff check 均通过。`PRRC_kwDOT7YN2s7jlNzR` 揭示高优先级 absent-root acceptance 与 durable lock invariant 冲突，连同三个既有 provenance/directory source 保持 pending/blocked/open；本轮不伪造 product revision 或 filesystem primitive。下一步提交并推送 preliminary code/ledger evidence，随后回复并关闭六个 fixed thread，只回复并保留四个 blocked thread。

2026-08-20 的 reply reconciliation 显示 14 条 top-level trigger 均为 `@codex`，117 个 review body 均为空或通用 automation；90 个 inline source 全部在 Review Conversation Log 中有 heading。六个本轮 fixed source 都有 `4ef1ba205eb323c702bceda830445f44feb4da46`、验证、GitHub reply URL 和 `thread resolved: true`；`PRRC_kwDOT7YN2s7jlNzR` 已得到精确 decision question，连同三个既有 provenance/directory source 维持 pending/blocked/open。下一步提交、推送本最终 reconciliation，然后再次读取会话确认没有漂移。
2026-08-20T05:52Z 的 merge preflight 发现四个 `pending`/`blocked` inline thread，故未进入 `completed`。人类选择 Revision-5 recovery contract 后，`gh pr ready --undo` 已将 PR #3 恢复为 open Draft，且 branch/head/依赖重新核对一致。本 Plan 正在从 `review` 回到 `active`；下一步是移除不可证明 provenance 的 cleanup、更新受管文档与回归，再重新完成 ready/review 事务和会话 reconciliation。


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

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAF`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831237)；线程 `PRRT_kwDOT7YN2s6adTiE`，当前已解决。

Problem: `publish_staging` 建立 `.skilload-publish-*` publication hard link 后，如果最终 `renameat` 失败，只清理原 `.skilload-library-*` staging entry，遗留的 publication link 违反 `SKL-LIB-009` 对 rename 前失败清理 staging 的要求。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 在 `publish_staging` 的最终 `renameat` error 分支按 held inode 先清理 `publication_name`，再清理原 staging name；新增 `after_publication_link_before_rename` fault hook 与 `export_removes_publication_link_when_rename_fails`，证明外部创建的 destination directory 保留、两个 skilload staging entry 都不存在。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`export_removes_publication_link_when_rename_fails`、`cargo fmt --all --check`、Clippy `-D warnings`、workspace all-features locked tests（77 个 core tests）与 workspace build 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813039880；thread resolved: true。

### PRRC_kwDOT7YN2s7jQzAQ — portable export/import 字节闭环

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAQ`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831248)；线程 `PRRT_kwDOT7YN2s6adTiN`，当前已解决。

Problem: 单条 note 的 `SKL-LIB-008` 合法上限与多次独立 import 可使 durable Library 的确定性 export 超过 `SKL-LIB-010` 的 67,108,864-byte input ceiling，导致当前二进制成功导出却无法重新导入自身 portable document。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/library.rs` 新增共享 `MAX_PORTABLE_LIBRARY_DOCUMENT_BYTES`、限制写入器和 deterministic encoder；`portable_library.rs` 用它读取/导出，`sqlite_library.rs` 在每个实际 import/dry-run plan 对完整结果调用 consuming size check。`docs/product-specs/library.md`、`docs/design-docs/application-and-persistence.md` 与本 Plan 将其说明为既有唯一可移植文档闭环，行为 revision 不变。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`transfer_encoding_rejects_a_document_over_its_byte_limit` 与 `transfer_encoding_rejects_valid_metadata_beyond_the_import_ceiling`（4,097 个单条合法最大字节 note）均通过，全部 workspace gate 也已通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813041486；thread resolved: true。

### PRRC_kwDOT7YN2s7jQzAb — first-import 锁后重新规划

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAb`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831259)；线程 `PRRT_kwDOT7YN2s6adTiT`，当前已解决。

Problem: 两个首次 import 都在 lock 前观察到 database absent 时，胜者发布后败者拿到 lock，当前 `import_first` 仍将存在的 database 视为 `database_identity_drift`，而不是在锁内以当前 durable entries 重新规划并执行 existing-database 路径。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 将 existing import 的锁内工作提取为 `import_existing_with_lock`；`import_first` 取得 lock 后发现合法 database 时以原 document 调用它重新规划，且保留已经出现的 durable state，不重入 lock。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`first_import_replans_after_a_concurrent_winner_publishes` 证明 winner 发布后 loser 添加自己的 source 且 export 含两条记录，全部 workspace gate 已通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813043162；thread resolved: true。

### PRRC_kwDOT7YN2s7jQzAh — first-import staging 无跟随打开

Source: 内联评论 `PRRC_kwDOT7YN2s7jQzAh`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3812831265)；线程 `PRRT_kwDOT7YN2s6adTiW`，当前已解决。

Problem: `NamedTempFile` 创建后到 `Connection::open` 前，same-account 替换 staging basename 为 symlink 会使 SQLite 以 read-write/create path open 跟随外部 target；最终 publish identity check 太晚，不能撤销 SQL 已对外部数据库造成的写入。

Disposition: fixed

Status: resolved

Resolution: `FirstImportStaging::open_connection` 在 hook 前后验证 held staging inode，并以 `SQLITE_OPEN_READ_WRITE | SQLITE_OPEN_NOFOLLOW`、无 create flag 打开；只有第二次验证成功才配置 connection 或执行 SQL。新增 pre-open symlink replacement regression，外部数据库保持原字节，未知 symlink 不被 cleanup。

Evidence: code/preliminary ledger commit `e8a025208e23e6feac7671714e8657f2e789cdcd` 已推送；`first_import_does_not_follow_a_staging_replacement_before_open` 证明 foreign database 字节不变，完整 workspace gate 已通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813044825；thread resolved: true。

### PRRC_kwDOT7YN2s7jSSdC — existing SQLite connection ABA inode

Source: 内联评论 `PRRC_kwDOT7YN2s7jSSdC`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813222210)；线程 `PRRT_kwDOT7YN2s6aeUM6`，当前已解决。

Problem: 评论担心 same-account process 仅在 `Connection::open_with_flags` 期间将既有 `skilload.db` 替换为 inode B、随后还原 inode A，会让 import 对 B 写入、sync A 并报告 `changed`。

Disposition: no-fix

Status: resolved

Resolution: 不改动运行时代码。`crates/skilload-core/src/adapters/sqlite_library.rs` 的既有 database 是非空 durable database；locked bundled SQLite 在开始写 journal 前以连接实际持有的 main-file handle 调用 `SQLITE_FCNTL_HAS_MOVED`。该 ABA 在 write 前返回 `SQLITE_READONLY_DBMOVED`，不会 commit B 或报告 changed；仍有本次 first-staging 的 zero-size 特例以独立 source 修复。

Evidence: `libsqlite3-sys 0.38.2/sqlite3.c` 的 `databaseIsUnmoved`（63747–63763）和 journal-open call（65474）显示该保护；2026-08-19 本机 A/B ABA probe 返回 `ReadOnly` extended code 1032，两个 database count 均保持 1。无运行时代码修复；preliminary evidence commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送并与 PR head 一致。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813511599；thread resolved: true。

### PRRC_kwDOT7YN2s7jSSdN — first-import SQLite connection ABA inode

Source: 内联评论 `PRRC_kwDOT7YN2s7jSSdN`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813222221)；线程 `PRRT_kwDOT7YN2s6aeUNB`，当前已解决。

Problem: first staging basename 能在 SQLite pathname open 的瞬间指向 inode B、之后恢复 held inode A；现有 entry revalidation 只看 A，可能让 SQL 写 B、最终发布未经写入的 A。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/lib.rs` 将 crate policy 收束为 `deny(unsafe_code)`；`crates/skilload-core/src/adapters/sqlite_library.rs` 只在 `verify_sqlite_connection_identity` 局部允许 bundled `sqlite3_file_control(..., SQLITE_FCNTL_HAS_MOVED, ...)`，并在 `FirstImportStaging::open_connection` 的任何 configure/SQL 前调用。新增 deterministic pre-open/post-open ABA hook regression，恢复 held staging path 后仍拒绝 connection，foreign inode 保持 0 bytes 且不发布 live database。

Evidence: `first_import_rejects_an_aba_staging_open_before_sql`、focused `cargo test -p skilload-core --all-features --locked`（83 passed）、`cargo fmt --all --check`、workspace Clippy `-D warnings`、locked workspace tests（11、12、83）与 build 均通过；code/preliminary ledger commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送并与 PR head 一致。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813513388；thread resolved: true。

### PRRC_kwDOT7YN2s7jSSdR — zero resolved evidence counts

Source: 内联评论 `PRRC_kwDOT7YN2s7jSSdR`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813222225)；线程 `PRRT_kwDOT7YN2s6aeUNF`，当前已解决。

Problem: `ResolvedSkill::new` 接受 `entry_count: 0` 或 `byte_count: 0`，但每个有效 resolved Skill 都包含非空 regular `SKILL.md`，这些 fabricated values 不应进入 portable/durable evidence。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/source.rs` 的 `ResolvedSkill::new` 分别拒绝 zero `entry_count` 与 `byte_count`；`docs/product-specs/library.md` 与 `api-v2.md` 将 positive count 写明为既有 valid resolved evidence，未改变行为 revision。

Evidence: `resolved_skill_rejects_zero_evidence_counts`、focused core 83 tests、workspace Clippy、locked workspace tests（11、12、83）与 build 均通过；code/preliminary ledger commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送并与 PR head 一致。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813515314；thread resolved: true。

### PRRC_kwDOT7YN2s7jSSdY — complete durable entry ceiling

Source: 内联评论 `PRRC_kwDOT7YN2s7jSSdY`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813222232)；线程 `PRRT_kwDOT7YN2s6aeUNK`，当前已解决。

Problem: scanner 只限制单个 input 的 10,000 entries；已有 10,000 durable entries 后再导入一条会产生 10,001-entry export，而当前 importer 必然拒绝该 self-produced document。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/library.rs` 新增共享 `MAX_PORTABLE_LIBRARY_ENTRIES` 和 `library_portable_document_entries` validation，combined post-import plan 与 export serialization 都经相同检查；`sqlite_library.rs` regression 在 10,000 existing entries 加一条时于 mutation/result 前失败。product/design/Plan 同步为既有唯一 transfer-format 闭环。

Evidence: `validation_rejects_more_entries_than_portable_transfer_can_import`、`complete_import_plan_rejects_more_entries_than_portable_transfer_allows`、focused core 83 tests、workspace Clippy、locked workspace tests（11、12、83）与 build 均通过；code/preliminary ledger commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送并与 PR head 一致。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813517046；thread resolved: true。

### PRRC_kwDOT7YN2s7jSSde — missing tags foreign key corruption

Source: 内联评论 `PRRC_kwDOT7YN2s7jSSde`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813222238)；线程 `PRRT_kwDOT7YN2s6aeUNP`，当前已解决。

Problem: `validate_library_tags_schema` 只 probe 三个列；若损坏 table 移除 `library_tags.canonical_source → library_entries.canonical_source` foreign key，`foreign_key_check` 没有声明关系可检查，orphan tag 会被静默忽略。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `validate_library_tags_schema` 除列 probe 外，要求 `PRAGMA foreign_key_list(library_tags)` 恰有一个 `canonical_source → library_entries.canonical_source ON DELETE CASCADE` relation；缺失或变形 relation 返回 `database_corrupt`。

Evidence: `tags_schema_without_entry_foreign_key_is_database_corrupt`、focused core 83 tests、workspace Clippy、locked workspace tests（11、12、83）与 build 均通过；code/preliminary ledger commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送并与 PR head 一致。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813518863；thread resolved: true。

### PRRC_kwDOT7YN2s7jSSdl — export post-sync output identity

Source: 内联评论 `PRRC_kwDOT7YN2s7jSSdl`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813222245)；线程 `PRRT_kwDOT7YN2s6aeUNW`，当前已解决。

Problem: final parent-directory sync 后只重验 parent identity；same-account process 可在 rename 后替换 output entry，命令会 sync replacement 并报告成功，未证明 requested path 仍指向 held staging inode。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 在 final parent-directory sync 和 parent revalidation 后再次调用 `verify_staging_identity` 比较 held staging FD 与 output entry；未知 replacement 不被 cleanup，结果为 typed identity-drift error 而非 success。

Evidence: `export_rejects_an_output_replaced_before_final_parent_sync`、focused core 83 tests、workspace Clippy、locked workspace tests（11、12、83）与 build 均通过；code/preliminary ledger commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送并与 PR head 一致。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3813520501；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpXX — 首次导入最终数据库身份复验

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpXX`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840343)；线程 `PRRT_kwDOT7YN2s6af6FC`，当前已解决。

Problem: 首次导入在发布 `skilload.db` 后同步 data directory 与新建目录时，没有再次比较 `database_name` 和 held staging FD；同账户替换已发布 database 后命令可能报告 `changed`。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的首次导入成功路径于 `sync_created_directory_entries` 后再次 revalidate held data directory 并比较 `database_name` 与 held staging FD；新 hook regression `first_import_rejects_a_database_replaced_after_final_sync` 确认 foreign replacement 不会被报告为 `changed`。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送，且当时 local/upstream/PR head 一致；`first_import_rejects_a_database_replaced_after_final_sync`、`mise exec -- cargo test -p skilload-core --all-features --locked`（89 passed）、workspace fmt/Clippy/all-features locked tests/build 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814088889；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpXh — staging sidecar 所有权

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpXh`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840353)；线程 `PRRT_kwDOT7YN2s6af6FJ`，当前已解决。

Problem: 未发布 staging 的 Drop 会把 cleanup 时首次看到的同名 regular journal/WAL/SHM 当作本调用所有；外部进程可在此前创建该文件并被错误 unlink。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已删除 `FirstImportStaging::Drop` 对 basename-derived sidecar 的 cleanup；Drop 只按 held identity 清理 staging/publication entry，未知 sidecar 保留。`first_import_precommit_failure_preserves_foreign_staging_sidecar` 验证 foreign `-shm` 不被 unlink。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；`first_import_precommit_failure_preserves_foreign_staging_sidecar`、focused core 89 tests 与 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814091103；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpXr — foreign-key mismatch 损坏分类

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpXr`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840363)；线程 `PRRT_kwDOT7YN2s6af6FP`，当前已解决。

Problem: `library_entries.canonical_source` 丢失 parent key 时，`PRAGMA foreign_key_check` 可能产生 `foreign key mismatch` SQL error；现有映射会错误返回 `invalid_state` 而不是 `database_corrupt`。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `database_error` 中将 `foreign key mismatch` 的 `SqliteFailure`/`SqlInputError` 映射为 `database_corrupt`。`foreign_key_parent_key_mismatch_is_database_corrupt` 以损坏 parent key fixture 验证 recovery category。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；`foreign_key_parent_key_mismatch_is_database_corrupt`、focused core 89 tests 与 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814092875；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpX0 — export publication link 最终身份复验

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpX0`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840372)；线程 `PRRT_kwDOT7YN2s6af6FV`，当前已解决。

Problem: export 创建并验证 `.skilload-publish-*.tmp` 后，最终 rename 前没有再次验证该 source entry；替换 publication link 可能覆盖 requested output。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已在 `crates/skilload-core/src/adapters/portable_library.rs` 的 `publish_staging` 中于 `after_publication_link_before_rename` 后重新验证 parent 和 held staging FD/publication entry，再执行 rename。`export_rejects_a_replaced_publication_link_before_rename` 确认旧 output 与 foreign replacement 保持不变。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；`export_rejects_a_replaced_publication_link_before_rename`、focused core 89 tests 与 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814094865 和 https://github.com/bootids/skilload/pull/3#discussion_r3814098125；客户端超时后的重叠调用留下两条审计回复；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpX8 — 数据库 publication link 最终身份复验

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpX8`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840380)；线程 `PRRT_kwDOT7YN2s6af6Fa`，当前已解决。

Problem: 首次导入创建并验证 `.skilload-db-publish-*.tmp` 后，no-clobber rename 前没有最终验证 publication entry；替换该 link 可能将 foreign database 发布为 `skilload.db`。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 first-import publication path 中于 publication-link hook 后重验 data directory 和 held publication entry，再运行 no-clobber rename。`first_import_rejects_a_replaced_publication_link_before_rename` 确认 foreign replacement 不会发布为 `skilload.db`。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；`first_import_rejects_a_replaced_publication_link_before_rename`、focused core 89 tests 与 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814097445 和 https://github.com/bootids/skilload/pull/3#discussion_r3814099886；客户端超时后的重叠调用留下两条审计回复；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpYG — 新建目录即时清理登记

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpYG`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840390)；线程 `PRRT_kwDOT7YN2s6af6Fi`，当前已解决。

Problem: `create_dir` 成功后、`CreatedDirectory` 加入调用者 cleanup list 前，打开、检查或限制新目录失败会遗留 pre-COMMIT directory footprint。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已在 `crates/skilload-core/src/adapters/configuration.rs` 添加 `PendingCreatedDirectory` rollback guard；identity 一经取得即登记，open/metadata/permission failure 仅删除仍匹配的 directory。`created_directory_rolls_back_when_opening_it_fails` 注入 open failure 并确认路径恢复 absent。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；`created_directory_rolls_back_when_opening_it_fails`、focused core 89 tests 与 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814099291；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpYK — read-only SQLite 连接身份绑定

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpYK`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840394)；线程 `PRRT_kwDOT7YN2s6af6Fl`，当前已解决。

Problem: export/dry-run 的 read-only existing database connection 没有检查 SQLite 实际 main-file inode；open 期间 ABA replacement 可让 metadata 检查的是 A、读取的是 B。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已把 `open_existing_database` 收束为 repository instance path，在 pathname open 后、configure/SQL 前调用既有 `verify_sqlite_connection_identity`。`export_rejects_a_read_only_database_aba_open` 验证 read-only ABA connection 被拒绝。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；`export_rejects_a_read_only_database_aba_open`、focused core 89 tests 与 workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814101465；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpYW — 应用层 dry-run outcome

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpYW`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840406)；线程 `PRRT_kwDOT7YN2s6af6Fv`，当前已解决。

Problem: `LibraryImportOperation` 对 dry-run 返回 `Unchanged`，仅 CLI 通过 `data.dry_run` 重算为 `observed`；其他 presentation adapter 会投影错误 outcome。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已在 `crates/skilload-core/src/domain/library.rs` 定义 `LibraryImportOutcome::{Observed, Changed, Unchanged}`，由 SQLite repository 返回；`crates/skilload-cli/src/main.rs` 直接投影 `operation.outcome`，不再读取 `data.dry_run` 重算结果。`dry_run_is_inert_and_first_import_round_trips` 与 CLI contract regression 覆盖该边界。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；focused core 89 tests、`library_import_export_is_portable_atomic_and_inert_when_dry_run`（1 passed）和实际 CLI `library import --dry-run --json` smoke 均观察到 `observed` 且 XDG roots 仍 absent。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814104289 和 https://github.com/bootids/skilload/pull/3#discussion_r3814107567；客户端超时后的重叠调用留下两条审计回复；thread resolved: true。

### PRRC_kwDOT7YN2s7jUpYj — CLI product-spec 实现状态

Source: 内联评论 `PRRC_kwDOT7YN2s7jUpYj`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3813840419)；线程 `PRRT_kwDOT7YN2s6af6F2`，当前已解决。

Problem: `docs/product-specs/README.md` 已声明 API-v2 cutover，`docs/product-specs/cli-contract.md` 的 status 仍称除 P1 三项外全部 planned；同优先级产品规格对 `SKL-CLI-004`/`005`/`012` Revision 2 的实现状态冲突。

Disposition: fixed

Status: resolved

Resolution: `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已更新 `docs/product-specs/cli-contract.md` status，明确 `PLAN-0003` 已实现 API-v2 current-producer 的 `SKL-CLI-004`/`005`/`012` Revision 2；未改变行为正文或 revision。

Evidence: code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送；`docs/product-specs/README.md` 与本 Plan Product Baseline 已声明同一 cutover，workspace gates 通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814107231 和 https://github.com/bootids/skilload/pull/3#discussion_r3814109659；客户端超时后的重叠调用留下两条审计回复；thread resolved: true。

### PRRC_kwDOT7YN2s7jWsuc — export publication source identity race

Source: 内联评论 `PRRC_kwDOT7YN2s7jWsuc`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3814378396)；线程 `PRRT_kwDOT7YN2s6ahTK6`，当前已解决。

Problem: `publish_staging` 在确认 publication link 指向 held staging inode 后，仍以 pathname 调用 `renameat`；同账号进程可在两次 syscall 之间替换该 link，使外来文件覆盖既有 output。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 现在在 final validation 时 snapshot existing output identity 或 absence，再以 `OutputPublicationGuard` 记录 existing target 或 `O_EXCL|O_NOFOLLOW` absence guard。held publication link 与 guard 经 `RenameFlags::EXCHANGE` 发布；post-exchange 不是 held staging/guard 的组合会反向交换恢复旧 output 或 absence。`export_restores_the_old_output_when_publication_changes_after_final_check` 和 `export_does_not_replace_an_output_changed_after_final_validation` 覆盖两个窗口。

Evidence: code/preliminary remediation commit `752c0f77b24a5300dffe7edcca952809688fdc1f` 已推送；focused core 94 tests、`cargo fmt --all --check`、workspace Clippy `-D warnings`、locked all-features workspace tests（11、12、94）、workspace build、实际 CLI dry-run/import/export smoke 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814836953；thread resolved: true。

### PRRC_kwDOT7YN2s7jWsuh — database publication source identity race

Source: 内联评论 `PRRC_kwDOT7YN2s7jWsuh`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3814378401)；线程 `PRRT_kwDOT7YN2s6ahTK9`，当前已解决。

Problem: first import 的 `NOREPLACE` rename 在最后 `verify_entry` 后仍通过可替换 publication pathname 发布；foreign replacement 可成为 authoritative `skilload.db`，随后检查只能报告已发生的损坏。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 在 first-import publish 前以 data-directory descriptor 建立 only-if-absent `FirstImportPublicationGuard`，以 `RenameFlags::EXCHANGE` 替代 final `NOREPLACE` rename，并在 post-exchange 复验 held staging/guard；mismatch 反向交换恢复 absence。`first_import_restores_absence_when_publication_changes_after_final_check` 注入最终检查后的 publication replacement。

Evidence: code/preliminary remediation commit `752c0f77b24a5300dffe7edcca952809688fdc1f` 已推送；focused core 94 tests（含 first-import exchange regression）、`cargo fmt --all --check`、workspace Clippy `-D warnings`、locked all-features workspace tests（11、12、94）、workspace build、实际 CLI smoke 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814838849；thread resolved: true。

### PRRC_kwDOT7YN2s7jWsul — GitHub owner grammar

Source: 内联评论 `PRRC_kwDOT7YN2s7jWsul`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3814378405)；线程 `PRRT_kwDOT7YN2s6ahTK_`，当前已解决。

Problem: `SourceIdentity::validate_owner` 仅检查 lowercase ASCII、数字和 `-`，接受 GitHub 不可能产生的 leading/trailing/consecutive hyphen owner 及超过 39-byte owner，因而可持久化无法与 fresh metadata 对齐的 canonical source。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/source.rs` 将 canonical owner 限制为 1–39 bytes、lowercase ASCII alphanumeric/single-hyphen grammar，并由 `portable_source_rejects_impossible_github_owner_logins` 覆盖 leading/trailing/consecutive hyphen、40-byte rejection 与 39-byte valid boundary。`docs/product-specs/source-and-trust.md` 澄清 `SKL-SRC-002`，`docs/references/github-repository-identity-and-auth.md` 记录验证来源；不改变 Revision 1 语义。

Evidence: code/preliminary remediation commit `752c0f77b24a5300dffe7edcca952809688fdc1f` 已推送；focused core 94 tests、`cargo fmt --all --check`、workspace Clippy `-D warnings`、locked all-features workspace tests（11、12、94）、workspace build 和 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814840641；thread resolved: true。

### PRRC_kwDOT7YN2s7jWsur — first-import SQLite sidecar rollback

Source: 内联评论 `PRRC_kwDOT7YN2s7jWsur`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3814378411)；线程 `PRRT_kwDOT7YN2s6ahTLC`，当前已解决。

Problem: pre-commit/rollback failure 时 `FirstImportStaging::drop` 只删除 staging database 和 publication link；SQLite 自行留下的 matching `-journal`、`-wal` 或 `-shm` sidecar 会使本调用新建 data directory 非空，阻止 absent-root recovery。

Disposition: fixed

Status: resolved

Resolution: `FirstImportStaging` 现在只在 SQLite `COMMIT` 返回 failure 后，以 `O_NOFOLLOW` FD/identity 记录仍存在的 `-journal`、`-wal`、`-shm`，Drop 仅 unlink recorded matching sidecar。`first_import_staging_removes_recorded_sqlite_sidecars` 覆盖 tracked cleanup；既有 `first_import_precommit_failure_preserves_foreign_staging_sidecar` 继续证明 pre-commit foreign sidecar 不会被 basename 猜测删除。

Evidence: code/preliminary remediation commit `752c0f77b24a5300dffe7edcca952809688fdc1f` 已推送；focused core 94 tests、`cargo fmt --all --check`、workspace Clippy `-D warnings`、locked all-features workspace tests（11、12、94）、workspace build 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3814842723；thread resolved: true。

### PRRC_kwDOT7YN2s7jWsuw — newly created directory identity binding

Source: 内联评论 `PRRC_kwDOT7YN2s7jWsuw`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3814378416)；线程 `PRRT_kwDOT7YN2s6ahTLF`，当前未解决。

Problem: `create_restrictive_directory_with_open` 在 successful `create_dir` 后才以 pathname 读取 first metadata；同账号进程若在该间隙替换为空目录，代码会把 replacement 记为 call-owned、修改权限并可能在 rollback 删除。

Disposition: pending

Status: open

Resolution: 人类已于 2026-08-20T05:52Z 选择 Revision-5 recovery contract，而非 native create-and-hold primitive。active rework 将停止由 create 后 pathname metadata 推断 directory ownership，pre-COMMIT failure 可保留无法证明 provenance 的 directory；在代码、回归和产品/设计同步完成前保持 open。

Evidence: 当前 safe macOS/Linux API 仍无 create-and-hold directory descriptor；人类决策、PR Draft 逆向事务和 `PLAN-0003` active transition 已记录。待实现提交与 focused/full validation。

GitHub outcome: 既有回复 https://github.com/bootids/skilload/pull/3#discussion_r3814847446 已提出并获得 recovery-contract 决策；thread resolved: false，待实现后回复并关闭。

### PRRC_kwDOT7YN2s7jcs0L — first-import 全路径 sidecar cleanup

Source: 内联评论 `PRRC_kwDOT7YN2s7jcs0L`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3815951627)；线程 `PRRT_kwDOT7YN2s6alUok`，当前已解决。

Problem: `FirstImportStaging` 仅在显式 `transaction.commit()` 返回 error 后记录 SQLite sidecar；`initialize_schema`、transaction/apply 写入或 pre-COMMIT rollback 路径的 sidecar 可遗留在本调用创建的目录中，阻止 pre-COMMIT absent-root recovery。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 现将 `FirstImportStaging::record_owned_sidecars` 放在 staging connection、schema、transaction/additions 的 SQLite error exit、successful additions 后的 pre-COMMIT rollback window 与 commit error；它仅以 `O_NOFOLLOW` held FD/identity 记录 `-journal`、`-wal`、`-shm`，Drop 只删除仍匹配的 entry。`first_import_staging_removes_recorded_sqlite_sidecars` 与既有 pre-COMMIT cleanup/foreign-sidecar regressions 保持该所有权边界。

Evidence: 修复提交 `282fd97dcd04dea37d0ff30848ecd26be603937f` 已推送；`mise exec -- cargo test -p skilload-core --lib` 通过 98 tests，`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、98）、workspace build、`git diff --check` 与实际 CLI round trip 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3816825331；thread resolved: true。

### PRRC_kwDOT7YN2s7jcs0U — orphaned SQLite sidecar detection

Source: 内联评论 `PRRC_kwDOT7YN2s7jcs0U`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3815951636)；线程 `PRRT_kwDOT7YN2s6alUor`，当前已解决。

Problem: `database_exists` 在 `skilload.db` 缺失时直接返回 false，不检查同目录 `-journal`、`-wal` 或 `-shm`；export/dry-run 因而可能把残留 generation 报为 empty，实际 import 也可能在其旁发布新的 main database。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 的 absent `skilload.db` 分支现在以 no-follow metadata 检查 `-journal`、`-wal`、`-shm`；任一存在都返回现有 `database_corrupt`。因此 export、dry-run 与实际 import 均不会把残留 generation 当作 empty Library 或发布新的 main database；`orphaned_database_sidecars_are_not_an_empty_library` 覆盖三种 suffix。

Evidence: 修复提交 `282fd97dcd04dea37d0ff30848ecd26be603937f` 已推送；`mise exec -- cargo test -p skilload-core --lib` 通过 98 tests，`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、98）、workspace build、`git diff --check` 与实际 CLI round trip 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3816826631；thread resolved: true。

### PRRC_kwDOT7YN2s7jcs0d — atomic first-import publication

Source: 内联评论 `PRRC_kwDOT7YN2s7jcs0d`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3815951645)；线程 `PRRT_kwDOT7YN2s6alUov`，当前已解决。

Problem: first import 在 publish 前把 zero-byte `FirstImportPublicationGuard` 创建为 authoritative `skilload.db`；进程终止或断电落在 guard 与 exchange 之间会把先前 absent Library 留为不可恢复的空 live database，且并发无锁读取可观察到该中间状态。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 已删除 `FirstImportPublicationGuard`、随机 publication link 与 `RenameFlags::EXCHANGE`；held staging inode 直接经 `linkat` 创建 absent `skilload.db`，target collision 返回 identity drift 并保留 foreign target。link 后与 final identity check 后的 replacement 只会保留 replacement，`first_import_publishes_a_committed_database_without_an_empty_guard` 证明 live name 首次出现即带 SQLite header；`docs/design-docs/application-and-persistence.md` 与 `docs/references/rust-sqlite-unicode-library-foundation.md` 同步该协议。

Evidence: 修复提交 `282fd97dcd04dea37d0ff30848ecd26be603937f` 已推送；`mise exec -- cargo test -p skilload-core --lib` 通过 98 tests，`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、98）、workspace build、`git diff --check` 与实际 CLI round trip 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3816827844；thread resolved: true。

### PRRC_kwDOT7YN2s7jcs0f — fallible tag sort

Source: 内联评论 `PRRC_kwDOT7YN2s7jcs0f`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3815951647)；线程 `PRRT_kwDOT7YN2s6alUox`，当前已解决。

Problem: `PortableLibraryDocument::sort_deterministically` 是 public fallible API，却在 comparator 中对 `normalize_tag` 使用 `expect`；外部 `LibraryRepository` 或直接 caller 提供 invalid tag 时，`serialize_for_transfer` 可能 panic 而非返回 `AppError`。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/library.rs` 现在在 mutation 前为全部 tags 预计算 fallible comparison key，再稳定重排 original tag strings；invalid tag 返回 validation error 且 document 保持不变。`sorting_invalid_tags_returns_validation_error_without_mutating_document` 与 `transfer_serialization_propagates_invalid_tag_validation` 覆盖 direct sort 和 transfer serialization。

Evidence: 修复提交 `282fd97dcd04dea37d0ff30848ecd26be603937f` 已推送；`mise exec -- cargo test -p skilload-core --lib` 通过 98 tests，`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、98）、workspace build、`git diff --check` 与实际 CLI round trip 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3816829354 和 https://github.com/bootids/skilload/pull/3#discussion_r3816830459；首次 reply request client timeout 后完成，retry 留下第二条相同审计回复；thread resolved: true。

### PRRC_kwDOT7YN2s7jiT5X — first-import publication source identity

Source: 内联评论 `PRRC_kwDOT7YN2s7jiT5X`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817422423)；线程 `PRRT_kwDOT7YN2s6apE20`，当前未解决。

Problem: `FirstImportStaging::link_to_absent_database` 在 `verify_entry` 后仍将 staging basename 传给 pathname-based `linkat`。同账号进程若在该间隙替换 staging entry，live `skilload.db` 会暂时链接 replacement；后续 identity check 虽报错，但 Drop 不会删除该 foreign database。

Disposition: pending

Status: open

Resolution: 人类已于 2026-08-20T05:52Z 选择 Revision-5 recovery contract，而非 held-file publication primitive。active rework 将保留 link source race 中无法证明 provenance 的 target，并使 pre-COMMIT/identity-drift error 不声称 absence；在文档、代码边界和回归完成前保持 open。

Evidence: locked `rustix 1.1.4` `linkat` 仍按 staging name 解析 source；人类决策、PR Draft 逆向事务和 `PLAN-0003` active transition 已记录。待实现提交与 focused/full validation。

GitHub outcome: 既有回复 https://github.com/bootids/skilload/pull/3#discussion_r3817530466 已提出并获得 recovery-contract 决策；thread resolved: false，待实现后回复并关闭。

### PRRC_kwDOT7YN2s7jiT5c — absent export target visibility

Source: 内联评论 `PRRC_kwDOT7YN2s7jiT5c`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817422428)；线程 `PRRT_kwDOT7YN2s6apE24`，当前已解决。

Problem: absent output 目前由 `OutputPublicationGuard::capture` 在 requested output name 创建 zero-byte guard，再 exchange 完整 staging document；因此并发 reader 或中断可观察/保留无效 zero-byte output，而不是保持 pre-publication absence。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 已将 absent branch 从 `OutputPublicationGuard` 分离：先建立/验证 hidden publication link，再以 `RenameFlags::NOREPLACE` 原子发布，不在 requested name 安装 zero-byte guard；existing output 仍走 reversible exchange。新增 `export_keeps_an_absent_output_absent_until_no_clobber_publish`。

Evidence: code/preliminary review log commit `0892f3ea7b515f6bdd0f8e371516af71eb390c9a` 已推送；focused portable tests 19/19、`cargo fmt --all --check`、workspace Clippy、locked tests（11、12、100）、workspace build 和实际 CLI API-v2 smoke 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3817530991；thread resolved: true。

### PRRC_kwDOT7YN2s7jiT5i — first-import sidecar provenance

Source: 内联评论 `PRRC_kwDOT7YN2s7jiT5i`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817422434)；线程 `PRRT_kwDOT7YN2s6apE28`，当前未解决。

Problem: `record_owned_sidecars` 以 first observed matching FD/identity 将 `-journal`、`-wal`、`-shm` 记为 owned；若同账号进程恰在 scan 前创建同名 regular file，后续 rollback 会 unlink foreign sidecar。现有 regression 只覆盖 sidecar 在前一次 scan 后出现，不覆盖该 adoption window。

Disposition: pending

Status: open

Resolution: 人类已于 2026-08-20T05:52Z 选择 Revision-5 recovery contract，而非 SQLite sidecar provenance primitive。active rework 将移除 first-observed FD 作为 deletion ownership evidence，pre-COMMIT failure 可以保留 matching-name sidecar 与目录；在代码、回归和文档同步完成前保持 open。

Evidence: 当前 no-follow FD 仅证明 observation-time identity；人类决策、PR Draft 逆向事务和 `PLAN-0003` active transition 已记录。待实现提交与 focused/full validation。

GitHub outcome: 既有回复 https://github.com/bootids/skilload/pull/3#discussion_r3817531868 已提出并获得 recovery-contract 决策；thread resolved: false，待实现后回复并关闭。

### PRRC_kwDOT7YN2s7jiT5n — export I/O error category

Source: 内联评论 `PRRC_kwDOT7YN2s7jiT5n`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817422439)；线程 `PRRT_kwDOT7YN2s6apE3A`，当前已解决。

Problem: feedback 建议将 export staging I/O 的 `PermissionDenied` 从当前带 native output `PathValue` 的 `validation_failed` 改为 API-v2 `permission_denied`/`inaccessible` 加 `AccessDetails`。

Disposition: no-fix

Status: resolved

Resolution: `SKL-LIB-009` 未把 export destination access failure 指定为 access error；当前 `validation_failed` 通过 `ValidationDetails.path` 精确投影 native output，且不会报告成功。API-v2 的 `AccessDetails` 强制携带 `TargetRef`，但该 closed scope enum 只有 workspace/global/manager/cache/database，未定义用户选择的 portable export destination；把它伪称为 database 或新增 enum 都会改变 API-v2 product contract，超出当前 Baseline。保留既有 documented validation projection；本次 preliminary commit 不改 code。

Evidence: preliminary review ledger commit `0892f3ea7b515f6bdd0f8e371516af71eb390c9a` 已推送；`docs/product-specs/library.md` `SKL-LIB-009` Revision 3、`docs/product-specs/api-v2.md` 的 `TargetRef`/`AccessDetails` 定义，以及 `export_io`→`ValidationDetails.path` current projection 支持该 no-fix。无代码变更；待 GitHub rationale。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3817532412；thread resolved: true。

### PRRC_kwDOT7YN2s7jiT5s — symlinked-parent native path semantics

Source: 内联评论 `PRRC_kwDOT7YN2s7jiT5s`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817422444)；线程 `PRRT_kwDOT7YN2s6apE3E`，当前已解决。

Problem: `absolute_path` 在通过 filesystem resolution 绑定 parent 前调用 `normalize_absolute`，会将 symlinked ancestor 后的 `..` 按词法折叠到不同目录；export 因而可能成功写入不是用户请求原生路径所指的位置。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 的 `absolute_path` 现只为 relative input prepend current directory，不再折叠 user-supplied `.`/`..`；`validated_output_parent` 继续 canonicalize/hold 实际 filesystem parent。新增 `export_preserves_native_symlink_parent_dotdot_semantics`，证明 export 只出现在 kernel 解析的 target。

Evidence: code/preliminary review log commit `0892f3ea7b515f6bdd0f8e371516af71eb390c9a` 已推送；focused portable tests 19/19、`cargo fmt --all --check`、workspace Clippy、locked tests（11、12、100）、workspace build，以及实际 CLI API-v2 smoke（physical output true、lexical output false）均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3817533169；thread resolved: true。

### PRRC_kwDOT7YN2s7jjnNR — 首次导入失败后的锁 inode 分裂

Source: 内联评论 `PRRC_kwDOT7YN2s7jjnNR`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817763665)；线程 `PRRT_kwDOT7YN2s6ap69p`，当前未解决。

Problem: 首次 import 在 COMMIT 前失败时先显式解锁、随后 cleanup unlink 本调用创建的 `database.lock`。等待者可能在两步之间取得旧 inode，而下一 importer 重建路径并取得新 inode，造成两个 importer 误以为各自持有同一全局锁。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/configuration.rs` 现让 restrictive lock helper 始终保留创建的 durable pathname；`crates/skilload-core/src/adapters/sqlite_library.rs` 移除 first-import cleanup 对 `database.lock` 的 ownership/unlink 路径，并只清理可证明由本调用创建的 data artifacts。`first_import_precommit_failure_retains_the_durable_lock` 证明后续 import 重用同一 inode，`first_import_post_lock_failure_retains_the_durable_lock` 覆盖锁取得后失败。

Evidence: 修复提交 `0dc0b9b3f83ef256c4de19c23186ed9c3816f826` 已推送且 PR #3 head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --lib sqlite_library` 通过 39 tests，`mise exec -- cargo test -p skilload-core --lib` 通过 102 tests；`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、102）与 workspace build 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818118890；thread resolved: true。

### PRRC_kwDOT7YN2s7jjnNS — DELETE rollback journal 导出保护

Source: 内联评论 `PRRC_kwDOT7YN2s7jjnNS`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817763666)；线程 `PRRT_kwDOT7YN2s6ap69q`，当前未解决。

Problem: `library export --output` 的 protected target 清单漏掉活跃 DELETE-mode SQLite transaction 使用的 `skilload.db-journal`；发布 JSON 会移动或删除 rollback journal，可能破坏 writer recovery 或 durable database。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 将 `skilload.db-journal` 纳入 protected paths，并新增 live DELETE transaction journal fixture `output_refuses_a_live_delete_mode_rollback_journal_before_staging`；`docs/product-specs/library.md` 将 `SKL-LIB-009` 提升至 Revision 4，`docs/product-specs/README.md`、持久化设计、SQLite 参考与本 Plan 同步该 active-generation boundary。

Evidence: 修复提交 `0dc0b9b3f83ef256c4de19c23186ed9c3816f826` 已推送且 PR #3 head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --lib portable_library` 通过 21 tests，`mise exec -- cargo test -p skilload-core --lib` 通过 102 tests；`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、102）与 workspace build 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818119966；thread resolved: true。

### PRRC_kwDOT7YN2s7jjnNU — 流式输入上限扫描

Source: 内联评论 `PRRC_kwDOT7YN2s7jjnNU`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3817763668)；线程 `PRRT_kwDOT7YN2s6ap69s`，当前未解决。

Problem: `read_input` 先缓冲完整文件再调用 `JsonScanner`；开头已超出 number/string/depth/value/entry ceiling 的大输入仍被读到 byte ceiling，违反 `SKL-LIB-010` Revision 4 对 streaming non-model pass 和第 129 个 number byte 立即失败的要求。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 的 `JsonScanner` 现直接消费 held regular-file 的 buffered reader，在 scanner pass 中累积成功反序列化所需 bytes；`read_import` 复用该 pass 的 bytes，不再先完整缓冲再扫描。新增 `scanner_stops_reading_at_first_streamed_number_overage`，证明第 129 个 number byte 失败后 scanner 不会请求后续 input chunk。

Evidence: 修复提交 `0dc0b9b3f83ef256c4de19c23186ed9c3816f826` 已推送且 PR #3 head 已核对为同一 SHA；`mise exec -- cargo test -p skilload-core --lib portable_library` 通过 21 tests，`mise exec -- cargo test -p skilload-core --lib` 通过 102 tests；`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、102）与 workspace build 均通过；隔离 CLI export → `--dry-run` import smoke 返回 API-v2 `observed` 且 state root absent。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818121108；thread resolved: true。

### PRRC_kwDOT7YN2s7jlNzO — export 前完整 portable document 验证

Source: 内联评论 `PRRC_kwDOT7YN2s7jlNzO`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3818183886)；线程 `PRRT_kwDOT7YN2s6aq-WJ`，当前已解决。

Problem: `PortableLibraryDocument::serialize_for_transfer` 只检查 entry 数、tag 排序与编码大小；公开字段可让错误 format version、重复 canonical source 或非法 metadata 成功 export，而同一 binary 的 import 会拒绝。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/domain/library.rs` 的 transfer serializer 先对 clone 调用完整 `validate()`，再排序/编码；`transfer_serialization_rejects_documents_import_would_reject` 覆盖错误 format version 与重复 canonical source。该修改已由 `4ef1ba205eb323c702bceda830445f44feb4da46` 推送。

Evidence: pushed remediation commit `4ef1ba205eb323c702bceda830445f44feb4da46`；focused `transfer_serialization_rejects_documents_import_would_reject`、`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、107）、workspace build、隔离 CLI smoke 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818396111；thread resolved: true。

### PRRC_kwDOT7YN2s7jlNzR — durable lock 与 pre-COMMIT absent-root 合约

Source: 内联评论 `PRRC_kwDOT7YN2s7jlNzR`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3818183889)；线程 `PRRT_kwDOT7YN2s6aq-WK`，当前未解决。

Problem: `docs/product-specs/library.md` 的 `SKL-LIB-010` Revision 4 acceptance 要求首次 import 在 `COMMIT` 前失败后 data/state roots 均恢复 absent，但当前 implementation/design 为避免 lock inode 分裂而保留 `database.lock`；低优先级 Plan 不能覆盖该规范冲突。

Disposition: pending

Status: open

Resolution: 人类已于 2026-08-20T05:52Z 选择 Revision-5 recovery contract，允许 pre-COMMIT failure 保留 durable `database.lock` 与其他无法证明 provenance 的 residual；active rework 将以产品规格替换 Revision 4 的 absent-root assertion，并在实现、回归和文档同步完成前保持 open。

Evidence: `database.lock` 的 stable-inode concurrency 规则与 Revision 4 acceptance 冲突；人类决策、PR Draft 逆向事务和 `PLAN-0003` active transition 已记录。待实现提交与 focused/full validation。

GitHub outcome: 既有回复 https://github.com/bootids/skilload/pull/3#discussion_r3818397649 已提出并获得 recovery-contract 决策；thread resolved: false，待实现后回复并关闭。

### PRRC_kwDOT7YN2s7jlNzT — exchange 后 publication entry replacement

Source: 内联评论 `PRRC_kwDOT7YN2s7jlNzT`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3818183891)；线程 `PRRT_kwDOT7YN2s6aq-WM`，当前已解决。

Problem: existing-output `RenameFlags::EXCHANGE` 后，旧 output 在 hidden publication pathname；先 `matches()` 再 `unlinkat` 存在同账号替换窗口，可能删除未知 replacement。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 的 successful exchange 路径不再 pathname-based unlink publication entry，保留该 entry；`export_preserves_a_replaced_publication_entry_after_exchange` 在 exchange 后植入 foreign replacement，证明请求 output 仍写入新 document 且 replacement 保留。`docs/design-docs/application-and-persistence.md` 同步该跨平台 inode-bound unlink 限制；修改已由 `4ef1ba205eb323c702bceda830445f44feb4da46` 推送。

Evidence: pushed remediation commit `4ef1ba205eb323c702bceda830445f44feb4da46`；focused `export_preserves_a_replaced_publication_entry_after_exchange`、`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、107）、workspace build、隔离 CLI smoke 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818398371；thread resolved: true。

### PRRC_kwDOT7YN2s7jlNzX — SQLite busy display domain

Source: 内联评论 `PRRC_kwDOT7YN2s7jlNzX`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3818183895)；线程 `PRRT_kwDOT7YN2s6aq-WQ`，当前已解决。

Problem: SQLite contention 已携带 `lock_domain: "database"`，但 `AppError::Busy` display text 固定为 configuration lock；API-v2 JSON 使用该 display text，误导操作员。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/error.rs` 的 Busy display 改为插值 `lock_domain`；`display_uses_the_recorded_lock_and_schema_domains` 覆盖 database busy text。修改已由 `4ef1ba205eb323c702bceda830445f44feb4da46` 推送。

Evidence: pushed remediation commit `4ef1ba205eb323c702bceda830445f44feb4da46`；focused `display_uses_the_recorded_lock_and_schema_domains`、`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、107）、workspace build、隔离 CLI smoke 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818399165；thread resolved: true。

### PRRC_kwDOT7YN2s7jlmpv — Library schema display domain

Source: 内联评论 `PRRC_kwDOT7YN2s7jlmpv`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3818285679)；线程 `PRRT_kwDOT7YN2s6arO78`，当前已解决。

Problem: Library SQLite schema errors 已携带 `domain: "library"`，但 SchemaNewer/MigrationRequired display text 固定为 configuration schema；API-v2 JSON 因而误报 domain。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/error.rs` 的 SchemaNewer/MigrationRequired display 属性改为插值 `domain`；`display_uses_the_recorded_lock_and_schema_domains` 同时覆盖 newer 与 migration 文本。修改已由 `4ef1ba205eb323c702bceda830445f44feb4da46` 推送。

Evidence: pushed remediation commit `4ef1ba205eb323c702bceda830445f44feb4da46`；focused `display_uses_the_recorded_lock_and_schema_domains`、`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、107）、workspace build、隔离 CLI smoke 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818399972；thread resolved: true。

### PRRC_kwDOT7YN2s7jlmpz — 非 array entries 的 schema 分类

Source: 内联评论 `PRRC_kwDOT7YN2s7jlmpz`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3818285683)；线程 `PRRT_kwDOT7YN2s6arO7-`，当前已解决。

Problem: syntactically valid 的 `entries: null` 被 non-model scanner 强制按 array 解析并错误归类为 `library_import_json`；wrong field type 应交给 schema deserialization，返回 `library_import_schema`。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/portable_library.rs` 仅在 root `entries` value 真为 `[` 时启用 array entry ceiling scanner，否则使用 generic JSON value scanner；`scanner_defers_non_array_entries_to_schema_validation` 覆盖 `entries: null`。修改已由 `4ef1ba205eb323c702bceda830445f44feb4da46` 推送。

Evidence: pushed remediation commit `4ef1ba205eb323c702bceda830445f44feb4da46`；focused `scanner_defers_non_array_entries_to_schema_validation`、隔离 CLI `entries: null` error smoke、`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、107）、workspace build 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818401004 和 https://github.com/bootids/skilload/pull/3#discussion_r3818402209；首次 reply 后的 resolve timeout 造成第二个等价回复，均保留为审计记录；thread resolved: true。

### PRRC_kwDOT7YN2s7jlmp1 — state directory sync root attribution

Source: 内联评论 `PRRC_kwDOT7YN2s7jlmp1`，[GitHub](https://github.com/bootids/skilload/pull/3#discussion_r3818285685)；线程 `PRRT_kwDOT7YN2s6arO8A`，当前已解决。

Problem: first import 的 created directory 集合混合 XDG state 与 data roots，却始终以 `XDG_DATA_HOME` 报告 parent sync failure；state/locks failure 的 structured environment error 因而指向错误 root。

Disposition: fixed

Status: resolved

Resolution: `crates/skilload-core/src/adapters/sqlite_library.rs` 将每个 `CreatedDirectory` 与 owning XDG variable 一起记录，并按原 reverse creation order 分别 sync；`first_import_sync_attributes_state_directory_failure_to_state_root` 覆盖 state-root failure attribution。修改已由 `4ef1ba205eb323c702bceda830445f44feb4da46` 推送。

Evidence: pushed remediation commit `4ef1ba205eb323c702bceda830445f44feb4da46`；focused `first_import_sync_attributes_state_directory_failure_to_state_root`、`cargo fmt --all --check`、workspace Clippy、locked workspace tests（11、12、107）、workspace build、隔离 CLI smoke 与 `git diff --check` 均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/3#discussion_r3818401871 和 https://github.com/bootids/skilload/pull/3#discussion_r3818403580；首次 reply 后的 resolve timeout 造成第二个等价回复，均保留为审计记录；thread resolved: true。

## Context and Orientation


仓库是一个 Rust Cargo workspace。`crates/skilload-core` 负责 domain、application、ports 和 adapters；`crates/skilload-cli` 是唯一进程入口，负责 clap 参数、终端文本和 API-v2 JSON。P2 已在 `domain/source.rs`、`domain/library.rs`、`domain/unicode_15_1.rs`、`application/library.rs`、`ports/library.rs`、`adapters/portable_library.rs` 与 `adapters/sqlite_library.rs` 实现可移植传输；CLI 的 `args.rs`、`main.rs`、`json.rs`、`human.rs` 除 `config get|set|unset|list` 外只支持 `library import` 与 `library export`。任何后续 Library 行为仍必须沿相同内向方向加入，command handler 不得直接操作 SQLite 或文件。

Library 是本机可搜索的来源元数据集合；在本交付中它只保存一个可移植记录：`ResolvedSkill` 的 canonical source、数字 repository ID、40 位 commit、`sha256:` integrity、验证过的 name/description 和正 entry/byte count，加上可选 alias/category/tags/note。canonical source 是带有小写 owner/repository、规范化 Skill path 和完整 branch/tag/SHA ref intent 的字符串身份；它不是 URL、缓存路径或 Trust 凭据。导入的记录永远没有 Trust；未来 Trust 查询可以把它投影为 `missing`，但 P2 不创建 Trust 表或命令。

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

新增 `crates/skilload-core/src/adapters/sqlite_library.rs`。它必须从已有 `StateRootResolver` 取得 data/state roots：没有 `data/skilload.db` 且没有同目录 `-journal`、`-wal`、`-shm` 时 export 和 dry-run 返回空库且不建目录；任一 orphaned sidecar 都返回 `database_corrupt`。实际 import 仅在全部文件/schema/domain 验证与冲突规划完成后创建 data root 与 `state/locks/database.lock`，并沿用配置的两秒有界锁等待/typed busy 行为。该 lock pathname 是持久协调身份；一旦可能有其他 contender 打开它，失败路径不得 unlink 或重建它。若 live database 原先不存在，adapter 在同一 data directory 建立 restrictive、唯一 staging database，完整建立 schema、执行 transaction、记录已证明由 SQLite persistence error/rollback 产生的 held-FD sidecar、sync staged file、重验 roots 后才从 held staging entry 经 descriptor-relative `linkat` 直接创建 absent `data/skilload.db`，随后 sync 父目录；不得先以 live name 创建 empty guard。commit 前失败关闭并移除仅由本调用创建且 identity 仍匹配的 staging database/sidecar/空 data directory，但可保留空 durable lock；commit 后 file 或 parent sync failure 返回 typed error，不报告 success 或 state absence。数据库路径、锁和已有文件都必须拒绝 symlink/非预期文件类型，创建目录和数据库使用 restrictive current-user permissions，并在提交前重验根绑定。

初始 schema 是一份明确的 v1 事务：`schema_info` 固定版本、`state_revision` 保存单调语义 revision、`library_entries` 以 canonical source 为主键并存储全部 portable resolved/metadata 标量、`library_tags` 以 `(canonical_source, comparison_key)` 唯一并通过外键关联 entry。开启 foreign keys，使用一个 SQLite transaction 计算并写入导入 plan；不创建 FTS 表、Trust 表或未来 ownership 表。现有 source 默认进入 kept；新 source 与现有/同批 alias 冲突必须在事务开始提交前返回 `conflict` 的 `ConflictDetails`，每个被拒绝 entry 使用 `internal_duplicate`、其 alias 为 `name`、其 source 为 `source`，且 `agent`/`path` 均为 null。对同一 batch 中后出现的相同 canonical source，同样在 transaction 前返回 `internal_duplicate`，但 `name` 为 null、`source` 为该后出现 entry 的 source；两类冲突都不修改任何行。实际新增时递增 revision。数据库已有更高 schema 返回 `schema_newer` 的 `SchemaDetails`；已识别的损坏返回 `database_corrupt` 的 `DatabaseCorruptDetails`，其中 database 是 `PathValue`、`backups`/`recoverable_exports` 因 P2 无恢复资产而为空且 `recovery_procedure` 为 `database-corruption-v1`；非普通文件返回 `invalid_state`，不能…

同一 adapter 同时负责 export：在一个只读一致性事务中按 canonical source 和 comparison key 获取记录，构建不含 Trust/local-state 的 `LibraryExportData`；在创建 staging 前，`LibraryTransferStore` 必须以 no-follow inspection、root revalidation 与 file identity 比较拒绝指向 live `data/skilload.db`、DELETE rollback `skilload.db-journal`、WAL、SHM 或 `state/locks/database.lock` 的 output。对其他 output，它在既有、真实父目录中建临时文件、写入完整 JSON、sync 文件并原子 rename，再 sync 父目录；directory 和 symlink output 一律拒绝。rename 前失败保留既有普通 output 或无 output 且清理 staging；rename 后 parent sync 失败返回 typed error，new output MAY 已发布，不能承诺旧 output 仍在。输出失败绝不改变数据库。P2 不注册 `--replace`，因此 `updated` 始终空。

### 里程碑 3：接入应用、CLI 与双投影


在 `crates/skilload-core/src/application/library.rs` 增加 `Application::library_import` 和 `Application::library_export`，并更新 `application/mod.rs`。`Application` 必须同时接收 configuration 和 Library ports，取消只接收一个 configuration store 的构造签名，并完整迁移 `crates/skilload-cli/src/main.rs` 及 `crates/skilload-core/src/adapters/configuration.rs` 测试中的所有 `Application::new` 调用。构造不会打开数据库；生产 composition 使用 `FileConfigurationStore`、`SqliteLibraryRepository` 和 `PortableLibraryTransferStore`。应用在 dry-run 时只读取/规划，在 commit 时只调用原子 repository import，并返回展示中立的 Library data/outcome。

扩展 `crates/skilload-cli/src/args.rs`：注册 `library import --input <PATH> [--dry-run]` 和 `library export --output <PATH>`，不注册任何其他 library 叶子、别名或隐藏 shortcut。将仅识别 configuration 的 JSON-operation 预扫描泛化为所有已实现叶子，使 Library 参数错误在 `--json` 下仍使用正确的 `library.import` 或 `library.export` operation。更新 parser/help 测试，证明这两个叶子存在、未实现的 Library 名称仍失败，且 `--input`/`--output` 不会被错误放到其他叶子。

扩展 `main.rs` 的 `Projection` 和 dispatch，使 CLI 只转换参数并调用 application。扩展 `json.rs` 以投影 `SourceIdentity`、`ResolvedSkill`、`PortableLibraryEntry`、`LibraryExportData` 与 `LibraryImportData`；成功 envelope 始终是单一 API-v2 JSON 值。为本交付新增的限制、alias 冲突、路径校验和数据库状态错误补足与 API-v2 catalog 对应的 `LimitDetails`、带 `internal_duplicate` 字段约束的 `ConflictDetails`、`ValidationDetails`、`SchemaDetails`、`DatabaseCorruptDetails` 或 `InvalidStateDetails`，不能把数字限制或恢复证据塞进散文错误字符串。扩展 `human.rs`，保持英文主要输出和既有注入安全字段编码；人类 import 输出 dry-run/changed/unchanged 加集合计数，人类 export 输出写入的安全引用路径和条目计数。绝不把输入文件、输出文件或异常数据未经编码写到 terminal。

### 里程碑 4：同步文档并完成可观察验收


实施后更新 `docs/product-specs/README.md`、`docs/product-specs/library.md`、`api-v1.md` 历史契约和新的 `api-v2.md` current catalog，使状态 prose 准确列出 `SKL-LIB-009` Revision 4、`SKL-LIB-010` Revision 4 和 API-v2 的 `SKL-CLI-004`/`005`/`012` Revision 2；同步 `docs/product-specs/database-recovery.md` 的显式 export output 调用与 salvage heading；除新的明确产品决定外，不得再修改这些行为正文或 revision。同步 `ARCHITECTURE.md` 的当前实现模块/SQLite ownership 描述，以及 `docs/design-docs/application-and-persistence.md`、`docs/design-docs/cli-json-and-release.md` 的当前实现状态、P2 module names 和真实测试路径。若实现发现本计划中的文件传输语义或字段与 authoritative specification 冲突，先修正实现或在得到明确产品决定后更新产品规格和 Plan baseline；不得静默降低 acceptance。

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

    pub enum LibraryImportOutcome { Observed, Changed, Unchanged }

    pub struct LibraryImportOperation {
        pub outcome: LibraryImportOutcome,
        pub data: LibraryImportResult,
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

计划修订说明（2026-08-19 12:46Z）：四项 ordinary remediation 均已由 `e8a025208e23e6feac7671714e8657f2e789cdcd` 和通过的完整 workspace gate 证明；本次记录每条 GitHub reply URL、resolved state 和全量会话 reconciliation。此 final Review Conversation Log commit 推送后必须再次读取完整会话与 PR head。

计划修订说明（2026-08-19 13:29Z）：完整会话重读发现六个新增 inline source。一个 existing nonempty SQLite ABA concern 由 locked bundled SQLite 的 write-time `HAS_MOVED` 检查证明为 no-fix；其余五项是既有 P2 portable transfer、durable schema 和 identity guarantees 内的 ordinary remediation，已逐源以 fixed/open 预登记，等待代码、测试、preliminary commit、GitHub reply 与 closure。

计划修订说明（2026-08-19 13:39Z）：第五轮 ordinary remediation 已完成代码、focused/full validation 与产品/架构/设计/reference 同步。五个 fixed/open ledger entry 现在含实际路径、回归名称和 passing evidence；no-fix entry 保留 bundled SQLite write-time moved-check 的可复核理由。下一步审阅完整 diff、推送 preliminary evidence，再逐一回复并关闭 thread。

计划修订说明（2026-08-19 13:40Z）：code/preliminary ledger commit `8cec7fd1d1e4c79c801215e23af54095e1f83bf5` 已推送，并已核对 local、upstream 与 ready PR head 一致。每个 fixed source 现引用该 SHA 和测试；no-fix source 记录无代码改动及同一 preliminary evidence。下一步重新读取会话、回复每个 source、关闭可关闭 thread，再提交最终 reconciliation。

计划修订说明（2026-08-19 13:45Z）：六个本轮 source 的 disposition/status、pushed code evidence、验证、GitHub reply URL 和 resolved state 已最终同步。final pre-documentation fetch 验证所有 57 个 actual problem source 均有 Plan heading，且没有未解决 thread、top-level 问题或 review-body 问题；本 documentation commit 推送后必须再次完整读取会话和 PR head。

计划修订说明（2026-08-19 14:47Z）：完整会话重读发现九个新的 open inline source。它们均在当前 `review` Product Baseline 内：本修订已完成代码、回归、产品状态/架构/设计/reference 同步，focused core（89）、CLI contract（1）、workspace fmt/Clippy/all-features locked tests（11、12、89）/build 与实际 CLI dry-run smoke 均通过。下方九项 ledger entry 保持 fixed/open，待检查 diff、创建并推送 preliminary code/ledger commit 后才回复并关闭对应 GitHub thread。

计划修订说明（2026-08-19 14:50Z）：code/preliminary remediation commit `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 已推送，local/upstream/open ready PR head 已核对为该 SHA。本次台账更新将九个 fixed/open source 的具体 changed path、测试和 pushed-code evidence 写入；待该 documentation commit 推送后，重新读取会话并逐 source 回复/关闭。

计划修订说明（2026-08-19 14:56Z）：九个 fixed source 均已使用 `5cc7d52012f81e75e4fb83aad67bff0c13e9678c` 的 code/validation evidence 回复并关闭。最终 `list --all` reconciliation 显示 66 个 inline source 与 66 个 Plan heading 完全对应、全部 resolved；8 条 top-level trigger 与 84 个 review body 没有独立问题。客户端超时造成的四个 source 双回复均保留实际 URL；本 documentation commit 推送后必须再读取会话与 PR head。

计划修订说明（2026-08-20）：本轮四项 fixed/open review remediation 的代码、回归、产品/设计/reference 同步和 preliminary ledger 已由 `752c0f77b24a5300dffe7edcca952809688fdc1f` 推送，local/upstream/open ready PR head 一致。`PRRC_kwDOT7YN2s7jWsuw` 已记录为 pending/blocked，未创建虚假修复；下一步重新获取完整会话，逐 source 回复/关闭四个 fixed thread，并让 blocked thread 保持 open。

计划修订说明（2026-08-20）：四个 fixed source 已以 `752c0f77b24a5300dffe7edcca952809688fdc1f`、通过的 focused/workspace/CLI evidence 和各自 GitHub reply URL 回复并关闭；`PRRT_kwDOT7YN2s6ahTLF` 已回复 exact decision question，保持 blocked/open。reply reconciliation 显示 71 个 inline source 与 71 个 Plan heading 对应，新增的五个 review body 为空；本次 final ledger commit 推送后必须再次完整读取会话和 PR head。

计划修订说明（2026-08-20）：再次完整读取 PR #3 后发现四个新的 ordinary P2 implementation 缺口；它们均未改变 Product Baseline，已在 Review Conversation Log 以 fixed/open 记录具体代码路径、验证和 GitHub 状态。directory identity source 仍因缺少人类产品/architecture 选择保持 pending/blocked；本轮只处理四项普通修复。

计划修订说明（2026-08-20）：四个新 inline source 的 ordinary remediation 已在 `review` 状态完成本地代码、回归、设计/reference 同步与全量验证。first-import 发布改为 direct `linkat`，sidecar 与 tag error 边界按当前 Product Baseline 收紧；未改变命令、API schema、行为 revision 或 acceptance scope。四项保持 fixed/open，等待 preliminary commit/push 后才回复和关闭；directory identity source 继续 blocked/open。

计划修订说明（2026-08-20）：修复提交 `282fd97dcd04dea37d0ff30848ecd26be603937f` 推送后，四个 fixed thread 均获得具体 commit/validation reply 并确认 `isResolved: true`；tag source 的 timeout-induced duplicate reply URL 已如实保留。最终 reconciliation 覆盖全部 10 条 top-level trigger、96 个 review body 与 75 个 inline source；没有未记录或未回答的非 blocked 问题。唯一 directory identity source 保持 pending/blocked/open，等待明确的人类产品/architecture 决定。

计划修订说明（2026-08-20）：`0dc0b9b3f83ef256c4de19c23186ed9c3816f826` 已推送，处理 database lock inode 分裂、活跃 DELETE rollback journal export target 与流式 non-model input scan；产品、设计、SQLite 参考、验证证据和三个 fixed/open Review Conversation Log 条目已同步，待 GitHub 回复、线程 closure 和最终 reconciliation。

计划修订说明（2026-08-20）：三个 `0dc0b9b3f83ef256c4de19c23186ed9c3816f826` fixed source 已分别以验证证据回复并关闭；回复 URL、`thread resolved: true` 和三个仍待人类决定的 pending/blocked source 已与 `list --all` reconciliation 对齐。

计划修订说明（2026-08-20）：本轮完整读取 PR #3 的 14 条 top-level、117 个 review body 与 90 个 inline source；六项 ordinary remediation 已由 `4ef1ba205eb323c702bceda830445f44feb4da46` 推送，并以具体回归、workspace gates、CLI smoke、GitHub reply URL 和 resolved thread state 写回 Review Conversation Log。`PRRC_kwDOT7YN2s7jlNzR` 因 `SKL-LIB-010` absent-root acceptance 与 durable lock identity 的未决产品/架构取舍保持 pending/blocked/open，与三个既有 provenance/directory source 一同等待人类选择；没有伪造修复或修改 Product Baseline。最终 Plan push 后必须再次完整读取 GitHub 会话和 PR head。

计划修订说明（2026-08-20T05:52Z）：merge preflight 完整读取 14 条 top-level、117 个 review body 与 90 个 inline thread，发现四项 pending/blocked 议题，因而没有进入 `completed`。人类选择将 `SKL-LIB-010` 修订为 Revision 5 的保守 recovery contract，明确不授权跨 macOS/Linux native/FFI primitive；PR #3 已恢复 Draft，Plan 移回 `active`。本修订将四项 ledger source 改为 open/pending，并把下一步限定为移除不可证明 provenance 的 cleanup、同步规范/设计/reference/测试、重新 ready/review 及会话 reconciliation。
