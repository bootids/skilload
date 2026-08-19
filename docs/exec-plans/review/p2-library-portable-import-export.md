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


本交付是完成的 `PLAN-0002`（Rust 工作区与配置垂直切片）之后的直接后继。`PLAN-0002` 已在默认分支完成并提供了 Cargo 工作区、`skilload-core` 的内向应用边界、`skilload-cli` 的薄展示适配器、严格 XDG 根解析以及当前 API-v1 的配置渲染模式；`PLAN-0001` 是其已完成的文档前提，因而不重复列为直接依赖。

本交付只建立可移植 Library 元数据传输与为它服务的最小 SQLite 持久化边界。它不实现 GitHub 输入解析、网络解析、首次 Trust 审批、Library add/remove/list/search/get/refresh、直接元数据编辑、FTS 查询、缓存、工作区、全局部署、manager、doctor 或数据库迁移/恢复命令。未实现的命令必须继续是 usage error，不能注册占位叶子。本计划在 `plan` 状态及 Draft PR 中仅定义和发布工作；只有后续明确的人类执行授权才可以移动到 `active` 并改动实现。

## Product Baseline


本交付完整实现并验证以下两个原子行为，均为 Revision 3。

* `docs/product-specs/library.md` 中 Revision 3 的 `SKL-LIB-009` 要求 `library export --output <PATH>` 将确定性、版本化且仅含 Library 来源/元数据的 `LibraryExportData` 原子写入请求路径；文件排除 Trust、全局/工作区状态、profile、绝对本机路径、凭据、缓存内容和操作时间，命令自身仍产生既定人类或 API-v1 结果。它在创建 staging 前拒绝活动 database、WAL、SHM 或 database lock target；rename 前失败保留旧 output 或无 output，rename 后父目录 sync 失败返回错误且不声称旧 output 仍在。
* 同一文件中 Revision 3 的 `SKL-LIB-010` 要求 `library import --input <PATH> [--dry-run]` 在读取前以 no-follow、nonblocking descriptor 和 `fstat` 拒绝非常规或发生 identity drift 的 input，并在构建任何模型或 `ImportPlan` 前执行流式、非模型预验证。它必须分别限制输入到 67,108,864 字节、10,000 个条目对象、1,000,000 个 JSON 值、八层对象/数组嵌套、每个字符串 1,048,576 UTF-8 字节和每个数字 128 字节；拒绝重复对象键、无效 JSON、未知字段、错误类型和无效元数据。整个批次要么提交、要么不改变 durable Library；默认保留只出现一次的已有 canonical source，alias 或同一 batch 的 canonical source 冲突使整个批次以 `ConflictDetails` 中的 `internal_duplicate` 失败，后者以 null `name` 和被拒绝 source 表示。首次 import 的 commit 前失败清理仅由调用创建的 state，commit 后 durability-sync 失败返回错误但不伪称 state 未改变；dry-run 必须报告与同一未变基线上的实际导入相同的 added/updated/kept/conflicts 集合且不创建 state。

导入文件中的 `ResolvedSkill`、`SourceIdentity`、完整 SHA、完整性摘要、已验证名称、描述和计数必须满足 API-v1 的可移植表示。为防止损坏的本地记录，本交付会复用 `SKL-SRC-002`、`SKL-SRC-007` 与 `SKL-SRC-012` 的 canonical source、名称与摘要约束，并对 alias/category/tag/note 执行 `SKL-LIB-008` 的大小、Unicode 15.1.0、NFC、`White_Space` 裁剪和 C/F 完整默认大小写折叠规则。这些约束的局部复用不表示来源获取、直接元数据命令或完整 Source/Library 行为已经完成；`SKL-SRC-*`、`SKL-LIB-001`、`SKL-LIB-004`、`SKL-LIB-005`、`SKL-LIB-008` 和 `SKL-LIB-011` 仍保持 planned，直到各自完整 acceptance 被独立交付。

`SKL-CLI-004`、`SKL-CLI-005`、`SKL-CLI-007`、`SKL-CLI-009`、`SKL-CLI-012`、`SKL-OPS-002`、`SKL-OPS-003`、`SKL-OPS-004`、`SKL-OPS-005` 和 `SKL-OPS-008` 同样不在本次完成基线中。本交付会遵守它们适用于新增叶子的部分：JSON stdout 只写一个 API-v1 信封、常见成功结果正确区分 observed/changed/unchanged、路径用 `PathValue`、读和 dry-run 不联网且不创建 skilload 根、导入写入仅在完整验证之后发生；未知较高 schema 拒绝写入，已识别的数据库损坏绝不被静默替换且必须返回 `database_corrupt` 的 `DatabaseCorruptDetails`。P2 不创建备份或导出位置索引，因此该诊断如实返回空 `backups` 和 `recoverable_exports` 集合、数据库 `PathValue` 与 `database-corruption-v1`；但不会宣称这些跨全产品行为的全部 acceptance 已满足。

完成时的可观测证明是：用户先对合法 regular-file 导入文件运行带 `--dry-run --json` 的命令，得到 `library.import` 的 `observed` 结果且 XDG data/state 根仍不存在；再运行实际导入，得到 `changed` 或 `unchanged`，只建立所需的 data SQLite 文件与写锁；运行 export 后得到确定性 `LibraryExportData` 文件。重复导入不重写数据库；混入无效条目、重复 JSON 键、超限输入、非常规 input、重复 canonical source 或 alias 冲突的批次不产生部分条目或持久写入。首次 import 的 commit 前注入失败后 data/state 根恢复为 absent；commit 后 sync 失败不报告成功或 absence。export 拒绝 database generation/lock target；rename 前输出失败保留旧 target，而 rename 后父目录 sync 失败返回错误且新 target 可能已发布。对损坏数据库的 import/export 返回带路径、空 P2 已知恢复集合和 `database-corruption-v1` 的 `database_corrupt`，并保持原文件及持久状态不变。

## Design and Architecture Inputs


`ARCHITECTURE.md` 要求 `skilload-core` 保持可复用 domain/application/ports/adapters 分层，CLI 只负责参数、调度与投影，产品变更由应用服务经显式端口提交。Library 元数据是 durable SQLite 的所有者，外部 Skill 字节、Trust、workspace 文件和缓存不是本交付数据库的替代或副本。有效的 config/data/state/cache application root 必须继续通过现有 `StateRootResolver` 同时解析、检查分离和在写前重新验证。

`docs/design-docs/application-and-persistence.md` 已指定 `data/skilload.db` 为 durable 数据库、查询缺失状态时返回内存空视图、写入仅在输入验证达到持久阶段后创建数据库、以及 Library export 不携带本机数据库行 ID 或操作时间。本交付采用该方向，但只创建 v1 的 `schema_info`、`state_revision`、`library_entries` 和 `library_tags` 最小表；不得假装 Trust、global、profile、workspace、owned link、confirmation token 或 FTS 已有真实业务所有者。

`docs/design-docs/cli-json-and-release.md` 规定每个已注册叶子只映射一个应用请求，CLI 不自行编排仓库调用；本分支已将可移植传输参数澄清为 `--input <PATH>`、`--output <PATH>`，使文件中只有可导入数据，而命令结果仍保留正常 API-v1 信封。`docs/design-docs/application-and-persistence.md` 还要求 P2 以 no-follow、nonblocking input descriptor 维持 scanner resource bound、以 staging database 避免首次失败发布 partial state，并区分 rename 前与 rename 后的 export sync failure。`docs/references/rust-sqlite-unicode-library-foundation.md` 记录了本交付的依赖事实：使用无默认特性的 `rusqlite 0.40.2` 加 `bundled`，以及精确 `unicode-normalization =0.1.23`；后者的表是 Unicode 15.1.0，而当前较新版本是 Unicode 17.0.0，不能使用。

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

- [x] (2026-08-19 08:03Z) 新增七项 review remediation 已由 `078d85f7582bad5f4c81b9f0d7944c069a5b558e` 推送；逐一回复后，PR #3 的 23 个 inline thread 均为 resolved。首次七线程批量写入在 30 秒客户端超时后仍继续，human/revision 两项各留下两条字节相同的回复；两个 URL 均已记录，未删除审计证据。待提交并推送本 Review Conversation Log 的最终 reconciliation。

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


## Outcomes & Retrospective


P2 implementation 与完整验证已完成，PR #3 已于 2026-08-19 05:57Z 转为 ready for review；ready transaction 的实现头为 `47f22f8a1687d5e46b9d787503565e1badad141a`，GitHub 已返回 `isDraft: false` 与相同 `headRefOid`。P2 提供仅含 portable resolved Library evidence 的 `data/skilload.db`：dry-run/absent export 不创建 XDG roots，实际 import 在所有 scanner/schema/domain/conflict planning 后才 staging/publish，existing canonical source 保持 kept，alias/canonical duplicate 以规定 `internal_duplicate` rollback，export 以稳定顺序写出独立 JSON 文件。`mise exec -- cargo fmt --all --check`、`mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings`、`mise exec -- cargo test --workspace --all-features --locked`（6、12、41 tests passed）和 `mise exec -- cargo build --workspace --all-features --locked` 均已通过；实际 `target/debug/skilload` smoke 在两个隔离 XDG root 中验证 dry-run observed/no-state、changed import、portable-only export 与 byte-identical second import/export。下一步是人类 review 与所需会话处理；只有明确人类 merge 授权才可进入 completed。

首次 review-state commit `b30afe3aa7a772f1ccf1885eb041006528f10c24` 推送后，PR #3 当时仍为 open、ready，`headRefOid`、local HEAD 与 origin branch 一致，且 `docs/exec-plans/review/p2-library-portable-import-export.md` 是唯一的 current Plan copy。

2026-08-19 06:42Z 的 review remediation 已由 `73d30857c5aa6281bec1bddf7004efe5f7e654c5` 推送：first-import database publish 改为 `persist_noclobber`、creation identity 从 restrictive lock helper 传给 RAII cleanup guard、SQLite durable row decode 错误映射为 `database_corrupt`，并收紧 portable source 的 repository/name/path/ref 重验证。local、upstream 与 PR #3 ready head 已核对为该 SHA；focused source（5）和 SQLite（9）测试、workspace all-features locked tests（6、12、46）、格式、Clippy 和 build 均通过；GitHub thread 回复于 06:51Z 完成。

2026-08-19 06:51Z 的最终会话核对确认：所有 16 个 inline thread 均为 resolved；本轮七个问题的每个 source 都有相应的 pushed-fix reply。source-path/ref 的批量请求超时留下的重复回复已在 Review Conversation Log 保留两个 URL，未影响修复、验证或 thread 状态。

2026-08-19 07:58Z 的新增 review remediation 已完成本地验收：export 使用持有父目录 descriptor 的安全 `rustix::fs::renameat`，既有数据库不再通过可创建或可跟随的路径打开，缺列/越界 schema 的错误保持 API-v1 可表示的 `database_corrupt`，first-import cleanup 保留 identity-mismatched directory，state revision 在写 entry 前受限递增，human import 输出枚举已计划 source。focused 与 workspace gates 均通过，实际 dry-run CLI smoke 已显示 quoted source；下一步仅为 preliminary commit、push、逐线程回复和关闭。

2026-08-19 08:03Z 的全量会话 reconciliation 确认：3 条 top-level `@codex` 触发评论和 31 个 review bodies 未提出新的独立问题，23 个 inline thread 全部为 resolved；新增七个 source 均有 code commit、验证、GitHub reply URL 和 close state。本 Log 的最终文档提交待推送，随后必须再读一次会话和 PR head。

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

## Context and Orientation


仓库是一个 Rust Cargo workspace。`crates/skilload-core` 负责 domain、application、ports 和 adapters；`crates/skilload-cli` 是唯一进程入口，负责 clap 参数、终端文本和 API-v1 JSON。P2 已在 `domain/source.rs`、`domain/library.rs`、`domain/unicode_15_1.rs`、`application/library.rs`、`ports/library.rs`、`adapters/portable_library.rs` 与 `adapters/sqlite_library.rs` 实现可移植传输；CLI 的 `args.rs`、`main.rs`、`json.rs`、`human.rs` 除 `config get|set|unset|list` 外只支持 `library import` 与 `library export`。任何后续 Library 行为仍必须沿相同内向方向加入，command handler 不得直接操作 SQLite 或文件。

Library 是本机可搜索的来源元数据集合；在本交付中它只保存一个可移植记录：`ResolvedSkill` 的 canonical source、数字 repository ID、40 位 commit、`sha256:` integrity、验证过的 name/description 和 entry/byte count，加上可选 alias/category/tags/note。canonical source 是带有小写 owner/repository、规范化 Skill path 和完整 branch/tag/SHA ref intent 的字符串身份；它不是 URL、缓存路径或 Trust 凭据。导入的记录永远没有 Trust；未来 Trust 查询可以把它投影为 `missing`，但 P2 不创建 Trust 表或命令。

可移植文档是恰好一个 JSON 对象：顶层 `format_version: 1` 和 `entries` 数组；每个元素是 API-v1 `PortableLibraryEntry`。export 按 canonical source 的二进制字节序排序 entries，按 tag 的 Unicode-15.1 comparison key 排序 tags，并使用稳定 JSON 序列化。导入 parser 先以 no-follow、nonblocking descriptor 和 `fstat` 确认 native input 是同一 regular file，随后才将其路径用于 `PathValue` 错误；每个 source identity 必须能重新渲染为与其 `canonical` 相同的字符串，`repository_display` 可保留当前显示拼写，但不能改变小写 identity。每个 batch 的 canonical source 只允许一个 entry，后出现的相同 source 作为 null-name `internal_duplicate` conflict 使整个 batch 失败。

“原子导入”表示在一次 SQLite 事务中写入全部新增 entries、tags 和递增的 `state_revision`，或一个也不写；对原本不存在的 database，它还表示只在 staging database commit 后发布 live file，并在 commit 前失败时清理本调用创建的 database/sidecar/lock/空目录。commit 后 durability-sync failure 不承诺 absence。“预验证”表示 scanner 先在不建立 portable domain model 或 ImportPlan 的情况下、从已验证的 regular-file descriptor 验证 JSON 语法、键唯一性和每个资源限制，随后才以 `serde` 的 closed schema 解析同一已验证字节。无数据库时 export 和 dry-run 的读取返回内存空库，绝不创建 XDG data/state/config/cache 根。

## Plan of Work


### 里程碑 1：固定依赖、Unicode 数据与 Library domain


先扩展根 `Cargo.toml` 的 workspace dependencies：新增 `rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }`、精确 `unicode-normalization = "=0.1.23"` 以及直接 `libc = "0.2.189"`，后者只为现有 Unix adapter 风格中的 `O_NOFOLLOW`/`O_NONBLOCK` 打开标志提供稳定常量；`crates/skilload-core/Cargo.toml` 仅引用实际使用的三项。运行 Cargo 更新后只提交解析到的 `Cargo.lock`，不顺带升级 P1 已锁定依赖。新增 `crates/skilload-core/build.rs` 与版本控制的 Unicode 15.1.0 `CaseFolding.txt`、`PropList.txt` 输入文件及其许可证说明；build script 只能读取这些本地文件，以 `cargo:rerun-if-changed` 生成 OUT_DIR Rust 表。它必须抽取 White_Space 代码点和 CaseFolding 的 `C`/`F` 映射，拒绝意外数据格式，绝不联网。

在 `crates/skilload-core/src/domain/` 新增 `source.rs`、`library.rs`、`unicode_15_1.rs`，并更新 `mod.rs` 和 `lib.rs` 的有意导出。`source.rs` 定义 `RefIntent`、`SourceIdentity`、`ResolvedSkill`、Integrity/SHA/decimal验证及 canonical parse/render；只接受已序列化的 canonical form，不在本交付中接受 GitHub URL 或连接网络。`library.rs` 定义可移植 entry/document、受验证 metadata、tag display/comparison key、import 请求、计划和结果。Unicode helper 的算法是：按 15.1 White_Space 裁剪、NFC、拒绝空/控制/双向格式字符、应用完整 C/F case fold、再 NFC；保存首次 display spelling 和 comparison key。所有长度用 checked UTF-8 字节和 Unicode scalar 计数，绝不以 `String::len` 代替 scalar 限制。

这个里程碑完成后，domain 单元测试必须能证明相同 canonical source 不会因 `@`、`/`、branch/tag namespace 混淆；` Review ` 与 `review` 给出一个保留首次 display 的 key；组合形式 `café` 与 `cafe` 后接 U+0301 的等价拼写共享一个 key；Turkish locale 不影响比较；`unicode_normalization::UNICODE_VERSION` 为 `(15, 1, 0)`；非法名称、SHA、integrity、超长或控制 metadata 均在持久化前失败。

### 里程碑 2：实现有界文件传输与 SQLite Library repository


在 `crates/skilload-core/src/ports/` 新增 `library.rs`，定义 `LibraryRepository` 和 `LibraryTransferStore`。repository 接收并返回经过 domain 验证的值；transfer store 负责 native input/output 路径、受限读、严格解析、同目录原子输出及 output 与 skilload-owned database generation/lock 的碰撞拒绝。不要将 `rusqlite::Connection`、SQL 行或未验证 JSON 泄漏给 application/domain。必要时把 `adapters/configuration.rs` 中可复用的目录创建、锁获取、XDG 重验和安全 I/O 错误映射抽到一个专门 adapter 模块，并让现有 configuration 测试保持原有行为；不能复制第二套 XDG 或锁策略。

新增 `crates/skilload-core/src/adapters/portable_library.rs`。它必须先以 `OpenOptionsExt` 和 `libc::O_NOFOLLOW | libc::O_NONBLOCK` 打开 input，先后比对 no-follow path metadata 与 descriptor `fstat` 的 file identity，并在任何读取前拒绝 symlink、directory、FIFO、socket、device 或 identity drift；regular descriptor 保持有界读取且错误携带 input `PathValue`。随后以增量字节状态机完成第一个 JSON pass：检查 UTF-8/JSON 语法，统计对象、数组、字符串、数字、Boolean、null 的总值，检查嵌套深度、顶层 `entries` 的对象数、字符串解码后 UTF-8 字节数、数字 token 字节数，以及每一个对象中的重复键。它必须在第一个越界点返回 `agent_input_limit_exceeded` 的 `LimitDetails`，以 `limit_kind` 区分 `library_import_bytes`、`library_import_entries`、`library_import_values`、`library_import_depth`、`library_import_string_bytes` 和 `library_import_number_bytes`，并报告 measured、allowed 与 input `PathValue`。通过 scanner 后，用同一受限字节缓冲的严格 `#[serde(deny_unknown_fields)]` 结构解析 `format_version: 1` 与所有嵌套对象；不能让 serde 的默认未知字段忽略或最后键覆盖行为绕开首 pass。

新增 `crates/skilload-core/src/adapters/sqlite_library.rs`。它必须从已有 `StateRootResolver` 取得 data/state roots：没有 `data/skilload.db` 时 export 和 dry-run 返回空库且不建目录；实际 import 在全部文件/schema/domain 验证与冲突规划完成后才创建 data/state root 和 `state/locks/database.lock`。锁的等待和 typed busy 行为沿用配置的两秒有界策略。若 live database 原先不存在，在同一 data directory 创建 restrictive、唯一的 staging database，完整建立 schema、执行 transaction、处理 sidecar、sync staged file、重验 roots 后才原子 rename 为 `data/skilload.db` 并 sync 父目录；commit 前任何失败都关闭并移除仅由本调用创建的 staging database/sidecar/lock/空目录，绝不触碰预先存在或 identity 不匹配的路径。commit 后 file 或 parent sync 失败返回 typed error，不报告 success 或 state absence。数据库路径、锁和已有文件都必须拒绝 symlink/非预期文件类型，创建目录和数据库使用 restrictive current-user permissions，并在提交前重验根绑定。

初始 schema 是一份明确的 v1 事务：`schema_info` 固定版本、`state_revision` 保存单调语义 revision、`library_entries` 以 canonical source 为主键并存储全部 portable resolved/metadata 标量、`library_tags` 以 `(canonical_source, comparison_key)` 唯一并通过外键关联 entry。开启 foreign keys，使用一个 SQLite transaction 计算并写入导入 plan；不创建 FTS 表、Trust 表或未来 ownership 表。现有 source 默认进入 kept；新 source 与现有/同批 alias 冲突必须在事务开始提交前返回 `conflict` 的 `ConflictDetails`，每个被拒绝 entry 使用 `internal_duplicate`、其 alias 为 `name`、其 source 为 `source`，且 `agent`/`path` 均为 null。对同一 batch 中后出现的相同 canonical source，同样在 transaction 前返回 `internal_duplicate`，但 `name` 为 null、`source` 为该后出现 entry 的 source；两类冲突都不修改任何行。实际新增时递增 revision。数据库已有更高 schema 返回 `schema_newer` 的 `SchemaDetails`；已识别的损坏返回 `database_corrupt` 的 `DatabaseCorruptDetails`，其中 database 是 `PathValue`、`backups`/`recoverable_exports` 因 P2 无恢复资产而为空且 `recovery_procedure` 为 `database-corruption-v1`；非普通文件返回 `invalid_state`，不能…

同一 adapter 同时负责 export：在一个只读一致性事务中按 canonical source 和 comparison key 获取记录，构建不含 Trust/local-state 的 `LibraryExportData`；在创建 staging 前，`LibraryTransferStore` 必须以 no-follow inspection、root revalidation 与 file identity 比较拒绝指向 live `data/skilload.db`、WAL、SHM 或 `state/locks/database.lock` 的 output。对其他 output，它在既有、真实父目录中建临时文件、写入完整 JSON、sync 文件并原子 rename，再 sync 父目录；directory 和 symlink output 一律拒绝。rename 前失败保留既有普通 output 或无 output 且清理 staging；rename 后 parent sync 失败返回 typed error，new output MAY 已发布，不能承诺旧 output 仍在。输出失败绝不改变数据库。P2 不注册 `--replace`，因此 `updated` 始终空。

### 里程碑 3：接入应用、CLI 与双投影


在 `crates/skilload-core/src/application/library.rs` 增加 `Application::library_import` 和 `Application::library_export`，并更新 `application/mod.rs`。`Application` 必须同时接收 configuration 和 Library ports，取消只接收一个 configuration store 的构造签名，并完整迁移 `crates/skilload-cli/src/main.rs` 及 `crates/skilload-core/src/adapters/configuration.rs` 测试中的所有 `Application::new` 调用。构造不会打开数据库；生产 composition 使用 `FileConfigurationStore`、`SqliteLibraryRepository` 和 `PortableLibraryTransferStore`。应用在 dry-run 时只读取/规划，在 commit 时只调用原子 repository import，并返回展示中立的 Library data/outcome。

扩展 `crates/skilload-cli/src/args.rs`：注册 `library import --input <PATH> [--dry-run]` 和 `library export --output <PATH>`，不注册任何其他 library 叶子、别名或隐藏 shortcut。将仅识别 configuration 的 JSON-operation 预扫描泛化为所有已实现叶子，使 Library 参数错误在 `--json` 下仍使用正确的 `library.import` 或 `library.export` operation。更新 parser/help 测试，证明这两个叶子存在、未实现的 Library 名称仍失败，且 `--input`/`--output` 不会被错误放到其他叶子。

扩展 `main.rs` 的 `Projection` 和 dispatch，使 CLI 只转换参数并调用 application。扩展 `json.rs` 以投影 `SourceIdentity`、`ResolvedSkill`、`PortableLibraryEntry`、`LibraryExportData` 与 `LibraryImportData`；成功 envelope 仍是单一 JSON 值。为本交付新增的限制、alias 冲突、路径校验和数据库状态错误补足与 API-v1 catalog 对应的 `LimitDetails`、带 `internal_duplicate` 字段约束的 `ConflictDetails`、`ValidationDetails`、`SchemaDetails`、`DatabaseCorruptDetails` 或 `InvalidStateDetails`，不能把数字限制或恢复证据塞进散文错误字符串。扩展 `human.rs`，保持英文主要输出和既有注入安全字段编码；人类 import 输出 dry-run/changed/unchanged 加集合计数，人类 export 输出写入的安全引用路径和条目计数。绝不把输入文件、输出文件或异常数据未经编码写到 terminal。

### 里程碑 4：同步文档并完成可观察验收


实施后更新 `docs/product-specs/README.md` 与 `docs/product-specs/library.md` 的 status prose，使其准确列出 `PLAN-0003` 仅完成 `SKL-LIB-009`/`SKL-LIB-010` Revision 3；同步 `docs/product-specs/database-recovery.md` 的显式 export output 调用与 salvage heading；除新的明确产品决定外，不得再修改这两个行为正文或 revision。同步 `ARCHITECTURE.md` 的当前实现模块/SQLite ownership 描述，以及 `docs/design-docs/application-and-persistence.md`、`docs/design-docs/cli-json-and-release.md` 的当前实现状态、P2 module names 和真实测试路径。若实现发现本计划中的文件传输语义或字段与 authoritative specification 冲突，先修正实现或在得到明确产品决定后更新产品规格和 Plan baseline；不得静默降低 acceptance。

在 active Plan 中记录每个完成里程碑、所有发现和实际验证。完成实现后，先提交并推送代码、测试、锁文件、文档和 active Plan，再按 `docs/PLANS.md` 的 ready/review 原子事务转换 Draft PR。不要在计划状态中实施任何代码。

## 评审实施补充

2026-08-19 的最终 review remediation 细化而未扩大 Product Baseline：不增加命令、行为 ID 或产品 revision。除本 Plan 原有的 `rusqlite`、`unicode-normalization` 与 `libc` 外，workspace 直接声明已锁定的 `rustix 1.1.4`（`fs`），以安全 descriptor-relative `renameat` 完成 export publish。`portable_library.rs` 在创建 staging 后及最终 publish 前验证持有父目录的 identity；`sqlite_library.rs` 对既有 database 使用 no-create/no-follow 打开、对 schema/API 表示和 state revision 进行边界校验，并在 cleanup 保留 identity-mismatched directory；`human.rs` 逐项投影导入计划的 canonical source。所有这些修复都以现有 `SKL-LIB-009`/`SKL-LIB-010` 的原子性、恢复诊断和双投影 acceptance 为界。

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

第二组 parser tests 对每个上界生成精确输入和超一输入。它们检查不建立 `PortableLibraryDocument` 或 `ImportPlan` 的错误路径、重复键不被“最后键覆盖”、JSON string escape 的解码后 UTF-8 计数正确、每一对象层级的重复 key 都被发现，且 JSON error 的 `LimitDetails` 同时有 `limit_kind`、decimal measured/allowed 与 input `PathValue`。同组以 FIFO、directory、symlink、device（可用时）和 lstat/open identity swap 证明 input descriptor 在 scanner 前被拒绝，且测试不依赖 writer/EOF 才完成。

第三组 repository tests 从无数据库开始：空 export/dry-run 返回空集合且不创建根；合法 commit import 创建 schema 和 entries/tags；第二次相同 import 返回 unchanged 且数据库内容和文件 identity 不变化；一批中一个 invalid record、alias conflict 或 canonical source duplicate rollback 全部，分别以规定的 `internal_duplicate` alias/name/source 或 null-name/source `ConflictDetails` 失败；首次 import 的 schema/write/commit 前 fault injection 后 data/state 根恢复为 absent，而 commit 后 sync fault 返回错误且不声称 absence。output 目录中的临时写失败不会改数据库或留下最终半文件；database/WAL/SHM/database-lock target 在 staging 前被拒绝；rename 前失败保留旧 output，rename 后 parent sync failure 返回错误且允许新 output 已存在。外部创建的 symlink/非普通 database、lock 或 output 不能被接受。损坏 SQLite fixture 必须保留原文件且返回 `database_corrupt`：database `PathValue`、空 `backups`/`recoverable_exports` 和 `database-corruption-v1` 都与 API catalog 一致。测试还要以 bundled connection 创建临时 FTS5 virtual table 或等价 compile-option probe，证明架构要求的嵌入式能力，而不是把宿主 SQLite 当作依据。

最后执行实际 CLI smoke，所有 XDG 根使用临时绝对路径、网络被禁止：

    skilload library import --input ./portable-library.json --dry-run --json
    skilload library import --input ./portable-library.json --json
    skilload library export --output ./round-trip.json --json

第一条必须是 `library.import`、`ok: true`、`outcome: "observed"`、`dry_run: true`，且不创建 skilload 状态；第二条只能是 `changed` 或在完全相同既有状态下 `unchanged`；第三条必须是 `library.export` 的 observed envelope，并在 `round-trip.json` 中写出仅含 `format_version` 与 `entries` 的可移植 document。用第二个隔离 XDG home 导入该文件并重新导出，规范化 JSON 必须相等，Trust/缓存/绝对路径字符串不得出现。人为插入无效 entry、alias collision、重复 canonical source、重复 JSON key、超限文件或 FIFO input 时，CLI 必须非零退出、JSON stdout 仍只有一个合法 error envelope，并且导入前后 SQLite 可导出数据相同；alias collision 的 envelope 必须含规定的 `internal_duplicate`、alias 与被拒绝 source，canonical duplicate 的 envelope 必须有 null `name`。将 `--output` 指向 database/WAL/SHM/database lock 必须在 staging 前失败且不改动 target；模拟 rename 后 parent sync failure 必须失败且不假称旧 output 保留。损坏数据库 fixture 必须返回 `database_corrupt` 的 `DatabaseCorruptDetails` 且不改写数据库。

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
