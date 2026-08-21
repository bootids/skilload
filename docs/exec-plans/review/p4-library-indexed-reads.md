---
plan_id: PLAN-0005
branch: codex/p4-library-indexed-reads
pull_request: https://github.com/bootids/skilload/pull/5
status: review
depends_on: [PLAN-0004]
---

# 实现 Library 索引化离线读取


本交付让用户在导入 Library 后，直接通过 `library list`、`library search` 和 `library get` 离线浏览本地元数据；搜索覆盖名称、描述、alias、tags、category、note 与 repository，并且不会把 SQLite 查询语言暴露给调用者。已有 schema v1 用户可先继续 list/get/export，再用显式 `doctor --fix` 生成可验证 backup 并升级到带 FTS5 索引的 schema v2；用户可从 JSON/human 结果、确定性分页、迁移证据和实际二进制 smoke 看到它工作。

本 ExecPlan 是 living document。执行期间必须持续维护 `Progress`、`Surprises & Discoveries`、`Decision Log`、`Outcomes & Retrospective` 与 `Review Conversation Log`。

仓库根目录的 `docs/PLANS.md` 是本计划内容、状态迁移和失败恢复的权威规则；本文件必须始终与其保持一致，并能让第一次进入仓库的实现者只依赖当前 worktree 和本文件完成交付。

## Delivery Metadata


本计划直接依赖 `PLAN-0004`。该前置计划已在默认分支的 `docs/exec-plans/completed/p3-library-metadata-mutations.md` 中完成，并通过传递依赖取得 `PLAN-0003` 的 portable Library、SQLite schema v1、API-v2 producer、Unicode 15.1.0 tag 规则和 `PLAN-0002` 的 Rust/configuration 基础；本计划不重复列出传递依赖。

交付只增加 Library 的三个离线读取叶子、支持它们的 FTS5 schema v2、一次 v1→v2 forward migration，以及对当前 durable database 的真实 `doctor [--fix]` 路径。它不实现 `library add|remove|refresh`、GitHub resolution、Trust mutation、cache、workspace、global、manager、deployment、HTTP、TUI、Web 或完整未来 doctor domain inventory。未实现命令继续是 usage error，不能注册 placeholder。Planning baseline 已关联 Draft PR https://github.com/bootids/skilload/pull/5；初始提交 `88eec453bbb7a08dea160601fa66093398be9c72` 已推送且 GitHub 返回同一 `headRefOid`、`isDraft: true`、head `codex/p4-library-indexed-reads`、base `main`。在后续明确执行授权前，Plan 必须保持 `plan` 且 PR 必须保持 Draft。

## Product Baseline


本交付完整实现并验证以下原子行为。

* `docs/product-specs/library.md` 中 `SKL-LIB-004` Revision 2：使用 embedded FTS5 索引 verified name、description、alias、tag display spelling、tag comparison key、category、note 和 repository。用户查询是纯文本词项 AND，不是 raw FTS5 expression；Unicode 15.1.0 `White_Space` 分词、NFC 原文/完整默认大小写折叠 alternatives、完整 FTS string quoting 与空查询错误都属于本 revision。Base metadata保留原始 UTF-8；非 NFC free-text FTS projection保留原值并以 ASCII newline 追加 NFC representation，使 normalized query term可命中规范等价的值而不改变 list/get/export。该 Revision 2 由 2026-08-20 的产品选择确定，替代尚未实现且未规定 query language 的 Revision 1。
* 同一文件中 `SKL-LIB-005` Revision 1：`library list`、`library search` 和 `library get` 只读本地 durable metadata且不联网、不刷新、不写 derived state。List/search 在分页前按 canonical source binary order 排序；仅二者接受 limit 1..=1,000（默认 100）与完整 `u64` offset（默认 0），并返回 requested page、returned count 和 pre-page total。
* 同一文件中 `SKL-LIB-011` Revision 1：完成已有 import/export/metadata mutation 与本交付 list/indexed-search/get 的 10,000-entry 组合证据。代表性 release-build exact get、第一页 list 和 full-text search 各使用 10 秒本地验收预算；fixture 构造和 schema migration 不计入单次 query 计时，永久 CI tests 验证语义而不使用容易抖动的 wall-clock assertion。
* `docs/product-specs/cache-and-operations.md` 中 `SKL-OPS-003` Revision 1：当前唯一 forward migration 在任何 live schema write 前生成 standalone、durable、带 manifest 的 online backup，再以一个 SQLite transaction 从 v1 升到 v2；失败留下 prior readable v1 加完整 backup，或完整 durable v2，绝不留下被报告为成功的部分 schema。Unknown newer schema 和 downgrade 保持 write refusal。
* `docs/product-specs/library.md` 中 `SKL-LIB-009` Revision 5：portable export 在创建 staging 前必须拒绝任何已发布 v1→v2 migration backup 的 `.db`、sibling `.manifest.json` 或同一 inode alias，保持 recovery asset 不受后续 JSON export 覆盖。该安全收紧由本轮 final-review 发现并在同一 Plan 的 Product Baseline 内验证。

以下行为约束本切片，但本计划不把跨产品尚未满足的 acceptance 误报为完成项。

* `SKL-CLI-001` 仍为 planned，因为 50 个 canonical leaves 尚未全部实现；本切片只把 `library list|search|get` 与真实 `doctor [--fix]` 从 usage error 切换到 application operations，不能增加 alias 或 scaffold。
* `SKL-CLI-004`、`SKL-CLI-005` 与 `SKL-CLI-012` Revision 2 已由 `PLAN-0003` 完成。本交付必须扩充现有 API-v2 producer：`library.list` 返回 `LibraryEntriesData`，`library.search` 返回 `LibrarySearchData`，`library.get` 返回 `LibraryEntry`，`doctor` 返回 `DoctorData`；success/error stdout仍是一个 JSON value，read outcome 为 `observed`，路径仍用 `PathValue`，pagination 的 `offset`/`total` 仍用 lossless decimal strings。
* `SKL-OPS-005` 与 `SKL-OPS-008` 要求 absent reads/default doctor 不创建任何 root且全部离线。`SKL-LIB-001` 允许但不要求 derived-name convenience selector；本切片的 `library get` 只接受完整 canonical source，避免在 alias/name precedence 与 ambiguity candidates 尚无完整产品契约时猜测。
* `SKL-OPS-002` 仍未完成，因为同一 durable database 的 Trust、global、manager、workspace 等 future tables 尚不存在；本切片只加入其要求的 FTS5 derived index而不声称完整 ownership model。
* `SKL-OPS-004`、`SKL-CACHE-006` 与 `SKL-CACHE-007` 仍未完成，因为完整 corruption restore、cache/deployment/workspace/manager inspection和所有 future repair 尚不可达。本切片只实现 real current-database doctor：只读 schema/base/FTS diagnosis、v1 migration和 FTS-only rebuild。Base corruption继续返回 `database_corrupt`并指向既有 `database-corruption-v1` operator procedure；不得增加隐藏 reset/restore 命令。

完成时的可观察路径是：在 isolated XDG roots 中导入两个条目；list 按 source 排序并分页；search 通过每个索引字段命中且把 `OR`、`NOT`、`*`、引号和 `name:...` 当普通文本；get 对 canonical source返回 entry、对 missing source返回 `not_found`。默认 doctor在 absent/healthy/v1/FTS-drift/corrupt/WAL-sidecar fixtures 上不改变 filesystem bytes或 timestamps；corrupt与 WAL-sidecar generation返回 typed `database_corrupt` error而不是成功 DoctorData。对 v1 fixture运行 `doctor --fix` 后，可看到一对验证通过的 backup/manifest和 `migrate` action，schema成为 v2、base rows与 `state_revision` 不变、search开始工作；对 FTS-only drift运行 fix只产生 `repair` action且不改 base metadata。
当前 review remediation 还必须证明：仅 `library_fts` schema SQL malformed 时，base-only list/get/export 保持可读、search/write 以 `library_fts_invalid` 拒绝且 `doctor --fix` 可重建；仅 operational metadata 损坏而 portable entries/tags 完整时，`database_corrupt` details 列出 `library.export`，该 export 成功返回 deterministic document；所有 import/metadata mutation 在 product write 前运行 FTS5 special integrity check；human corruption error 显示 validated backups 与 recovery export。
`SKL-OPS-004` Revision 1 的本轮澄清：已存在 current database 的 `-journal`、`-wal`、`-shm` sibling 都是 observed generation 的成员；read/default doctor 先在 pre-open gate 盘点，再在 held descriptor 的 SHARED read snapshot 建立后、读任何 Library schema/data 前复核。任一盘点发现 companion 都返回含 recovery details 的 `database_corrupt`；snapshot 后的 writer 不能混入已持有 generation。此处只明确既有 corruption/recovery 边界，不改变 behavior revision。

规划基线已经把 Revision 2 query semantics同步到 `docs/product-specs/README.md`、`docs/product-specs/library.md`，把技术选择同步到 `docs/design-docs/application-and-persistence.md`、`docs/design-docs/cli-json-and-release.md`，并新增 `docs/references/sqlite-fts5-library-search.md`。本轮 final-review 将 `SKL-LIB-009` 从 Revision 4 提升至 Revision 5，且将 SQLite malformed-derived-schema research 同步到 `docs/references/sqlite-backup-and-corruption-recovery.md`；执行状态与实际证据必须持续写回这些文件、`ARCHITECTURE.md`（若边界变化）和本 Plan。

## Design and Architecture Inputs


`ARCHITECTURE.md` 要求依赖向内：CLI 只解析参数、调用一个 application operation并渲染；application通过 focused ports协调；domain不能导入 CLI、SQLite、filesystem或process；SQLite adapter独占 SQL、XDG path、database identity、lock、backup、migration、sync和 FTS maintenance。Library只拥有 source/metadata；FTS是从 `library_entries` 与 `library_tags` 重建的派生索引，不能成为第二个 owner，也不能授予 Trust。当前无 Trust table，所有可达 `LibraryEntry.trust_state` 继续如实为 `missing`。

`docs/design-docs/application-and-persistence.md` 固定 `data/skilload.db`、`state/locks/database.lock`、pairwise-disjoint XDG roots、no-follow main-file identity gate、DELETE journal mode、一个 global durable-database mutation lock、transactional state mutation 与 descriptor-bound durability sync。当前 schema v1 有 `schema_info`、`state_revision`、`library_entries` 和 `library_tags`，没有 FTS。Schema v2只新增普通 content-bearing `library_fts` virtual table；不重建 base tables，也不引入 integer surrogate identity。每个 FTS row保存 unindexed canonical source与八类 indexed text columns；adapter在 import/metadata mutation 的同一 transaction中显式维护，migration/doctor从 base rows完整重建。

FTS tokenizer固定为 bundled SQLite 的 `unicode61 remove_diacritics 0`。Domain 使用 `crates/skilload-core/src/domain/unicode_15_1.rs` 的固定 `is_white_space`、NFC与 `full_case_fold`生成逻辑词项；adapter只负责把每个 literal中的 `"`写成`""`并包围双引号，然后将同一词项的 raw/folded alternatives以括号内 OR组合成一个 group，不同词项的 group之间以显式 `AND` 连接。FTS5 的 implicit AND只存在于裸 quoted phrases之间；括号表达式与后续 phrase或另一个 group之间不存在隐式组合，`("Review" OR "review") "code"` 是 syntax error，因此组合必须显式。用户字符串永远不作为 FTS grammar拼接。每个非 NFC 的 free-text FTS column保留原文并以 ASCII newline 追加 NFC projection；base row不变，newline是 tokenizer separator。Tag display strings与 comparison keys分别用 ASCII newline聚合到不同列，不改变 tag storage。

`docs/references/sqlite-fts5-library-search.md` 记录 FTS5 string quoting、bare-phrase implicit AND的语法边界与显式 `AND` 组合、`unicode61`、content-bearing index、special `integrity-check`/`rebuild` 与 rusqlite backup API事实。`docs/references/sqlite-backup-and-corruption-recovery.md` 规定 live WAL generation不能靠复制 main file备份；migration必须用 SQLite online backup得到 standalone snapshot，并记录 read-only SQLite 打开 WAL-mode generation会创建 `-shm`/`-wal` sidecar、`immutable=1` 会忽略 WAL 内容的实验事实。为 `rusqlite 0.40.2` 启用 `backup` feature，并加入 `sha2 0.11.0`（`default-features = false`）流式计算 SHA-256；不得引入 async runtime、ORM、外部 search service或通用 migration framework。
Review remediation 固定以下实现约束：`writable_schema=ON` 只在 descriptor-bound connection 上临时容忍 malformed derived schema，以验证 base/portable projection；任何 mutation 继续在 write 前执行 special `integrity-check` 并只将 confirmed derived drift 映射为 `library_fts_invalid`；repair 删除 FTS/shadow schema rows 后以 `RESET` reload schema；recovery export 不依赖 `schema_info`/`state_revision`，但必须验证 entries/tags schema、integrity、foreign keys 和 portable domain document。Published backup-pair target protection 属于 portable adapter，不把 SQLite backup validation helper 反向暴露给 CLI。
本轮 remediation 还要求：unknown newer schema 在任何当前 base-table validation 前以 `schema_newer` 分类；共用 absent-database probe 在 roots 重验前后执行，不能把 resolution 后替换的空 root 当成功 empty state。物理 FTS shadow 损坏先提交 schema detach、在 durable lock 内无 open transaction 时 `VACUUM` 回收不可达 pages并重验 database identity，随后用新 transaction 重建；中断状态保持 FTS missing/invalid 而非带 orphan pages 的伪 healthy。

Read兼容性是显式边界。完整 v1 base rows可供 list/get/export只读；search和所有 database writes返回现有 API-v2 `migration_required`，直到 `doctor --fix`。Default doctor从 identity-bound read-only source向内存 SQLite destination做 online backup，在副本上执行需要 writable connection的 FTS5 special check，因此 live XDG state保持不变。该不变性不能仅由 read-only flag推出：read-only SQLite connection打开 WAL-mode generation时会在可写 data directory创建并保留 `-shm`（header 为 WAL 而 sidecar缺失时连 `-wal` 一并创建），`immutable=1` 虽不创建 sidecar却忽略 WAL 内容。因此所有对已存在 live database的 read-only opens（list/get/search/export/doctor inspect）先在 root anchors 前后持有并重验 `data/skilload` 的 no-follow directory descriptor，再从该 descriptor 以 `openat(..., O_RDONLY|O_NOFOLLOW|O_NONBLOCK)` 打开单组件 main-file；`fstat` regular type、relative entry identity、100-byte header和相对 `-journal`/`-wal`/`-shm` 盘点均针对同一 held directory generation。journal-mode bytes（偏移18/19）非 (1,1) 或存在任一 sibling的 generation不属于任何 skilload 二进制可能发布的 DELETE-journal state，不经 SQLite open 即返回 `database_corrupt`；directory 或 main-file identity/ABA replacement 返回 `database_identity_drift`，symlink、FIFO及其他 non-regular entry 一律拒绝，且 FIFO 不会等待 writer。SQLite read-only connection仅从 held main-file descriptor的 `/dev/fd/<fd>` 打开；transaction 前后持续重验 held directory、relative entry和 pathname identity。
DELETE-mode `-journal` 同样不能与 descriptor-bound open 分离：SQLite 官方文档说明 active writer 的 RESERVED lock 与 hot journal 的 crash recovery 有不同含义，但 `/dev/fd/<fd>` 不能安全关联 pathname journal。当前 Plan 选择可证明 filesystem-inert 的保守边界：任意在 gate 时已观察到的 `-journal`、`-wal`、`-shm` 都在 SQLite 前按 `database_corrupt` 拒绝；这牺牲 active writer 期间的 read availability，避免误读可能未恢复的 main image。
同一 root-binding 规则也适用于 absent return：`database_exists_with_details` 在 pathname absence probe 前后重验 `ResolvedRoots`，所以目录在 `resolve_roots` 与返回 empty/not_found 之间被替换时返回 root identity error，不采纳 replacement generation。

## Purpose / Big Picture


当前用户可以通过 portable import建立 Library并修改 metadata，但只能重新 export才能查看全集，也不能按 note/tag/name搜索。完成后，用户可在网络关闭时列出、分页、检索和精确读取本地条目；human 与 API-v2显示同一 entries、query和 page metadata。已有 v1 database不会被读取命令暗中升级：用户先看到 doctor finding，再显式 fix并保留可验证 backup。任何 migration或 FTS repair失败都不会把 base Library或用户 metadata当作派生索引牺牲品。

## Progress


- [x] (2026-08-20 11:26Z) 从 clean、updated `main` 建立 `codex/p4-library-indexed-reads`；核对所有四个 completed Plans、产品规格、架构、设计、references与当前 Rust实现；确认没有现有 PLAN-0005、同名 branch或 Draft PR。
- [x] (2026-08-20 11:26Z) 取得产品决定：`library search`采用纯文本词项 AND，不采用完整短语或 raw FTS5 language；将 `SKL-LIB-004`提升为 Revision 2并同步规划设计/reference。
- [x] (2026-08-20 11:26Z) 创建本 `plan`-status ExecPlan；当前尚未提交、推送或创建 Draft PR。
- [x] (2026-08-20 11:36Z) 以 `88eec453bbb7a08dea160601fa66093398be9c72` 提交并推送 planning baseline，创建 Draft PR https://github.com/bootids/skilload/pull/5；GitHub 已验证 `isDraft: true`、head/base 正确且 `headRefOid` 等于该提交。本 metadata update 将 canonical URL、Progress 与 publication evidence 作为第二个 planning commit 推送；随后等待明确 human execution trigger。
- [x] (2026-08-20 12:03Z) 处理 PR #5 首轮规划评审的三个 inline 问题（FTS group 间显式 `AND`、base corruption 走 `database_corrupt` error、pre-open generation gate 防 sidecar）：修订本 Plan、`docs/design-docs/application-and-persistence.md` 与两个 reference 文档并推送；未改动任何运行时代码，Plan 保持 `plan`、PR 保持 Draft。
- [x] (2026-08-20 12:20Z) 收到执行授权（human 触发 `execute-exec-plan`）：验证 `PLAN-0004` 在 `origin/main` 为 `completed`、PR #5 为 Draft（`isDraft: true`、head `0d1a97e66840ecdb91c79cf8aaa2dff37c31e386` 与本地 HEAD 一致、base `main`）、worktree clean；执行 `plan → active` 迁移并推送，随后开始实现 milestones。
- [x] (2026-08-20 13:36Z) Milestone 1：`domain/library.rs` 新增 `LibraryPage`/`LibrarySearchQuery`/`LibrarySearchTerm`/`LibraryEntriesPage`/`LibrarySearchPage`（pinned `is_white_space` 分词、NFC raw + full-case-fold alternatives、`library_search_query_empty`）；新增 `domain/doctor.rs`、`ports/doctor.rs`（`DatabaseMaintenance`）、`application/doctor.rs`；`LibraryRepository` 增加 `list`/`search`/`get`；`Application::new` 增加 `Arc<dyn DatabaseMaintenance>` 并迁移全部 caller（CLI composition 复用同一 `SqliteLibraryRepository`）。
- [x] (2026-08-20 13:36Z) Milestone 2：`SCHEMA_VERSION=2`，`initialize_schema` 创建固定 SQL 的 `library_fts`；validation 拆为 base（含 domain rows）与 derived（fixed-SQL 比对 + 内容一一相等）两层；`open_existing_database` 先经 pre-open generation gate（header magic/journal-mode bytes (1,1)、`-wal`/`-shm` sibling 盘点）；list/get/export 接受 v1/newer（newer 仅 export），search 在 v1 返回 `migration_required`；list/search 用 CTE + 单次 LEFT JOIN 组装分页；`apply_additions`/`apply_metadata_change` 同事务维护 FTS。
- [x] (2026-08-20 13:36Z) Milestone 3：workspace 为 `rusqlite 0.40.2` 启用 `backup` feature 并加入 `sha2 =0.11.0`；`SqliteLibraryRepository` 实现 `DatabaseMaintenance`（absent/healthy/v1/newer/FTS-drift 诊断、online backup 到 `:memory:` 后运行 FTS5 `integrity-check`）；`doctor --fix` 在 durable lock 下发布 `data/backups/` 的 standalone backup + completed manifest（SHA-256/size/source identity/epoch-ns，no-clobber linkat 发布）后执行单事务 v1→v2 migration，或对 FTS-only drift 做 rebuild；state revision 不变；保守 prune。
- [x] (2026-08-20 13:36Z) Milestone 4：CLI 注册 `library list/search/get`（clap range parser 拒绝 limit 0/1001/负数，u64 parser 拒绝 offset 溢出）与 `doctor [--fix]`；json.rs 增加 `LibraryEntriesData`/`LibrarySearchData`/`LibraryEntry`/`DoctorData`（含 `TargetRef` projection、DecimalU64 strings）；human.rs 增加 terminal-safe 渲染；core adapter tests（13 个新增：分页/逐字段搜索/操作符 literal/v1 门控/migration+backup+manifest/FTS drift/doctor 惰性/WAL gate/迁移 failpoints/并发快照/newer/corrupt backups）与 cli_contract tests（4 个新增）全部通过；10,000-entry release 测量见 Artifacts；debug binary smoke 通过；产品/架构/设计文档已同步。
- [x] (2026-08-20 13:44Z) Implementation、acceptance、documentation与 retrospective已全部提交并推送：implementation commit `7f9fd769b12eb75f051c1f29aaece9dd4a292c6b`（29 files，+4985/−1367）。执行 `gh pr ready` 后验证 `isDraft: false`、`state: OPEN`且 `headRefOid` 等于该提交；本 Plan 随即移入 `docs/exec-plans/review/`、`status` 改为 `review`并推送本 status commit。
- [x] (2026-08-20 14:39Z) 处理 PR #5 第二轮实现评审的 7 个 inline 问题（FTS shadow 分类、backups 目录项同步、backup digest/symlink 校验、prune 保护当前 backup、mutation 路径 corruption 补全、锁内 FTS 重诊断、doctor identity 重验）并回复/resolve 全部 thread。
- [x] (2026-08-21) final review 第三轮的 4 个 inline 问题已在 Product Baseline 边界内实现并通过 focused 与 workspace validation：只读 descriptor-bound SQLite open、保守保留 migration backups、MATCH-derived corruption mapping、no-follow backup validation；修复与 preliminary Review Conversation Log 已以 `7e5a7bda7ce2dc3804a687a4e7249944a7908980` 推送，四个 GitHub replies 已成功写入且 threads 均 resolved；final log reconciliation 已推送，完整会话读取未发现未记录、未回答或 blocked 的实际问题。
- [x] (2026-08-21) 第四轮 remediation 已以 `648fb40323f2d35ac1dba6331501d0e03f7ecc6a` 推送：backup inventory 现验证 manifest/schema/standalone base、migration lock 后重新诊断、manifest 读取受 4 KiB 上限约束、orphaned FTS shadow tables 可重建；focused 70 tests 与 workspace fmt/clippy/test/build 已通过。四个 GitHub replies 已写入且 inline threads 均 resolved，最终 Review Conversation Log 记录如下。
- [x] (2026-08-21) PR #5 第五轮 final-review 的 4 个 inline 问题已由 `ffeea3a7e850712db8b4b89c19dd6bfddf84136b` 修复并通过 focused/workspace validation；4 个 GitHub replies 已成功写入、对应 threads 均 resolved，最终会话 reconciliation 与本 Review Conversation Log 已同步。
- [x] (2026-08-21) 第六轮 final-review 的 5 个 inline 问题已由 `b581acb63df42a882e0f02d5167a931fdf6e47f0` 修复并推送：malformed FTS schema derived-only repair、recoverable Library export diagnostics、mutation special integrity gate、human recovery assets 与 backup-pair export protection；`SKL-LIB-009` 已提升至 Revision 5。focused tests、core 153 tests、CLI 13+17 tests 与 workspace fmt/clippy/test/build 已通过；5 个 GitHub replies 已写入且 threads 均 resolved。最终完整 conversation reconciliation 现确认 7 个 top-level comments、33 个 reviews、27 个 threads 均已完整记录或不含独立问题。
- [x] (2026-08-21) 第七轮 final-review 的两个 inline 问题已由 `0a1cad3897588623b77c69b0fe90279a9d770257` 修复并推送：read-only generation gate 现绑定已解析 `data/skilload` directory descriptor并用 relative nonblocking no-follow open 验证 main file；FIFO race 被拒绝而不等待 writer。新增两项 adapter regressions与既有 ABA/WAL regressions通过，workspace fmt/clippy/test/locked build 通过；两个 GitHub replies 已写入并 resolve，final conversation reconciliation 已记录。
- [x] (2026-08-21) 第八轮 final-review remediation 完成：backup companion rejection、snapshot-bound live-sidecar recheck 与 corruption recovery inventory root binding 已由 `8a0d84dc1e6de9959c0423f99273aa214c4f38b8` 推送；三个 GitHub replies 已写入，三个 inline threads 均 resolved。最终完整 reconciliation 读取为 9 个 top-level comments、40 个 reviews、32 个 threads；所有 actual inline source 都有本 Log 条目、reply 与 resolved state，无 pending 或 blocked source。finalized Review Conversation Log 由当前 documentation commit 提交。
- [x] (2026-08-21) 第九轮 final-review 的三个 inline 问题已由 `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 修复并推送：三个新 regression 已由 red→green 证明，workspace fmt/clippy/test/build 全部通过；三个 GitHub reply 已写入、对应 thread 均 resolved，最终 complete conversation reconciliation 无未记录或 blocked actual problem。
- [x] (2026-08-21) 第十轮 final-review 的两个 inline defect 已由 `9dc0fd058d54cf67f4d9e3edea5e9d7cdabc34f0` 推送：共享 FTS projection 为 non-NFC free-text 保留 raw 加 NFC alternative，read snapshot 在 callback 返回 error 前重验 generation；两项新增 adapter regression red→green，workspace fmt/clippy/test/locked build 均通过。两个 GitHub reply 已成功写入且 threads 均 resolved；最终 reconciliation 无未记录、未回答或 blocked source，final Review Conversation Log documentation commit 已推送。
- [x] (2026-08-22) 第十一轮 final-review 的三个 inline defect 已在现有 Product Baseline 内完成 remediation：portable export 对不可完整枚举的 migration backup inventory fail closed、writable SQLite connection 在最终 generation revalidation 后再次执行 `SQLITE_FCNTL_HAS_MOVED`、FTS reference 记录 detach commit → `VACUUM` → fresh rebuild transaction。两项新增 regression、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（13+17+164）与 `cargo build --workspace --locked` 已通过；预备 remediation commit `f032f9c1f087fa72b7ca55666e8b5d92e3149f27` 已推送，待回复并 resolve 三个 thread。

## Surprises & Discoveries


- Observation: `PLAN-0004` 已明确把 search排除，因为它需要 query semantics、derived index、schema migration、backup和 doctor repair；这不是可安全塞入 metadata mutation PR的小补丁。
  Evidence: `docs/exec-plans/completed/p3-library-metadata-mutations.md` 的 Decision Log与范围段落明确列出这五项独立工作。
- Observation: 当前实现完全没有 list/search/get application或 repository method，唯一 FTS行为是测试 bundled compile option。
  Evidence: repository search仅命中 `sqlite_library.rs` 对 `ENABLE_FTS5` 的断言；`ports/library.rs` 只有 export/import/mutate，`args.rs` 未注册三个读取叶子。
- Observation: FTS5 `integrity-check` 使用 special INSERT syntax；read-only connection会以 readonly error拒绝它，即使检查成功不改变产品数据。
  Evidence: SQLite FTS5官方文档规定 `INSERT INTO ft(ft) VALUES('integrity-check')`；规划期 isolated SQLite实验确认 read-only connection拒绝该语句，而 MATCH与 `PRAGMA integrity_check`仍可读。Default doctor因此必须在内存副本上运行 special command。
- Observation: 当前 v1 base schema不需要为 FTS引入 surrogate integer key。
  Evidence:普通 content-bearing FTS table可保存 `canonical_source UNINDEXED`并通过稳定文本身份join；避免 external-content rowid与 `VACUUM`稳定性问题，也避免重建现有 base/foreign-key schema。
- Observation: `sha2 0.11.0` 是 2026-08-20 crates.io 当前 stable，声明 Rust 1.85且 MIT OR Apache-2.0，可被仓库 Rust 1.97.1使用。
  Evidence: `docs/references/sqlite-fts5-library-search.md`保存 crates.io metadata与来源。
- Observation: FTS5 没有“括号表达式 + 后续 phrase/group”的 implicit AND；`("Review" OR "review") "code"` 与两个括号 group 并排都是 syntax error，显式 `AND` 才有效。
  Evidence: 2026-08-20 planning 实验（SQLite 3.53.4）：`"code" "review"` 正常；`("Review" OR "review") "code"` 返回 `fts5: syntax error near ""code""`；`("Review" OR "review") AND "code"` 正常命中。原 implicit AND 组合在任何 raw≠folded 词项上会违反 `SKL-LIB-004` Revision 2 “不触发 FTS grammar error” 的 acceptance。
- Observation: read-only SQLite connection打开 WAL-mode generation会在可写目录创建并保留 `-shm`（header 为 (2,2) 且无 sidecar 时连 `-wal` 也一并创建）；`immutable=1` 不创建 sidecar 但忽略 WAL 内容，会产生错误诊断。
  Evidence: 2026-08-20 planning 实验证实三种情形的 sidecar 文件清单变化与 `immutable=1` 的 “no such table” 误读；结论已固化到 `docs/references/sqlite-backup-and-corruption-recovery.md`。

- Observation: rusqlite 0.40.2 的 `Backup::run_to_completion` 以 `assert!(pages_per_step > 0)` 拒绝负值；SQLite 文档允许 -1 表示单步复制，但该 Rust 封装不支持。
  Evidence: `rusqlite-0.40.2/src/backup.rs:299` 的 panic "pages_per_step must be positive"；实现固定为 512 pages/step。
- Observation: SQLite 在 `sqlite_master.sql` 中逐字保存 `CREATE VIRTUAL TABLE` 语句文本，使 fixed-SQL 相等比对成为可靠的 derived-shape 校验，无需解析 FTS5 内部表。
  Evidence: 诊断输出 `stored sql: "CREATE VIRTUAL TABLE library_fts USING fts5(canonical_source UNINDEXED, … tokenize = 'unicode61 remove_diacritics 0')"` 与创建常量逐字节相等。
- Observation: macOS 上 `tempdir()` 返回 `/var/folders/...` 而 XDG resolver 通过 symlink canonicalize 为 `/private/var/folders/...`；测试对 doctor action target 的路径断言必须比较 canonicalize 后的路径。
  Evidence: 断言失败输出 left `/private/var/...` right `/var/...`；改用 `database.canonicalize()` 后通过。
- Observation: 测试夹具中把 skill name 改成与 path basename 不一致的值会被 import 接受（struct 构造绕过 `ResolvedSkill::new`），随后读取按 domain-invariant 违反归类为 `database_corrupt`——这正是外部篡改行应得的分类，但夹具本身必须用合法条目。
  Evidence: 同名共存夹具改用共享 basename `skills/one/review` 与 `skills/two/review` 后通过；`same_name_sources_coexist_and_search_orders_by_source` 断言两者 name 相等。
- Observation: `PRAGMA integrity_check(<table>)` 的 table-name 形式（含 `'sqlite_master'`）在 bundled 3.53.4 上可用且只检查该表；仅损坏 FTS shadow b-tree 时整库检查报告 `Tree 9 page 9 cell 1: Extends off end of page` 而全部 base 逐表检查仍为 `ok`，page-1 base corruption 仍被 `sqlite_master` 逐表检查捕获。
  Evidence: 2026-08-20 scratch 实验（`PRAGMA integrity_check('...')` 五组输出 + `library_fts_data` root page 尾部字节翻转后的整库/逐表对照）；回归测试 `fts_shadow_corruption_stays_doctor_fixable` 与 `corrupt_base_keeps_typed_details_with_known_backups`。
- Observation: 物理 damaged 的 FTS5 shadow b-tree 无法用任何 SQL 清除——`DROP TABLE library_fts`、`DROP TABLE library_fts_data`、`DELETE FROM library_fts_data` 全部以 `SQLITE_CORRUPT` 失败，且失败发生在 schema-modifying 语句中途时会毒化整个 transaction（后续 commit 也失败）。
  Evidence: 2026-08-20 scratch probe 输出（drop/delete/commit 均 `Err(DatabaseCorrupt)`）；detected damage 必须在任何 DROP 尝试之前完成，因此 `rebuild_derived_index` 先检测再选择手术路径。
- Observation: 直接删除 FTS virtual/shadow schema rows后在同一 transaction重建，旧 b-tree pages不会进入 freelist；即使 FTS projection/special integrity 都通过，整库 `PRAGMA integrity_check` 仍报告 `Page …: never used`，会把损坏数据库误报 healthy。
  Evidence: 新增 `fts_shadow_corruption_stays_doctor_fixable` red run 显示 `Page 9` 至 `Page 13: never used`；将 repair 拆为 committed detach、transaction-external `VACUUM`、second-transaction rebuild 后，该 regression 与 bundled SQLite 返回 `ok`。SQLite 3.51.0 local probe 也得到 detach 后 `never used`、VACUUM 后 `ok` 且 inode 不变。
- Observation: 使用仍持有的 `/dev/fd/<fd>` 打开 bundled SQLite connection 能使 gate 与实际读取的 inode 相同；在 hook 把 pathname 替换为 WAL generation 后，read 返回 `database_identity_drift` 且 replacement 未生成 `-wal`/`-shm`。
  Evidence: `read_only_open_never_creates_sidecars_for_a_replaced_wal_generation` 与 `export_uses_checked_generation_when_a_read_only_aba_is_restored` 均通过；后者还证明 replacement 被还原时 export 使用已检查 generation 而非短暂 ABA replacement。
- Observation: 在跨 macOS/Linux 的可移植 API中，验证后的 pathname unlink 无法原子约束到已验证 inode；保留 backup pair 比删除可能被替换的 entry 安全。
  Evidence: `migration_retains_all_validated_backup_pairs` 预置三个排序更旧的 valid pairs 后仍观察到四个 complete pairs；旧 prune 实现会删除一个旧 pair。
- Observation: SQLite 官方 locking 文档区分 active rollback journal（writer 持有 RESERVED lock，普通 pathname reader 可以共存）与 crash 留下的 hot journal（必须先 rollback）；但 descriptor-bound `/dev/fd/<fd>` open 无法安全使用 pathname 的 journal。
  Evidence: 2026-08-21 核验 [SQLite File Locking and Concurrency](https://www.sqlite.org/lockingv3.html) 与 [SQLite Database File Format](https://www.sqlite.org/fileformat2.html)；`reads_reject_live_rollback_journals_before_descriptor_opens` 证明当前 adapter 对 active journal 也保守拒绝，`rollback_journal_generation_is_rejected_before_sqlite_opens` 证明 hot/foreign journal 不会被 bypass。
- Observation: 普通 SQLite schema load 会因单个 malformed `library_fts` SQL text 阻断完整 base proof；read-only connection 启用 `writable_schema=ON` 后可读取 base tables，而 `OFF` 保留当前 cache、`RESET` 则重新解析并在未删除 malformed row 时失败。
  Evidence: 2026-08-21 isolated SQLite probe（ordinary/read-only）与 bundled `malformed_fts_schema_stays_derived_and_doctor_fixable` regression；结论已同步至 `docs/references/sqlite-backup-and-corruption-recovery.md`。
- Observation: held main-file descriptor 本身不能证明其父 `data/skilload` 仍是最初解析的 root；同时 `O_NOFOLLOW` 不会阻止 FIFO open 等待 writer。
  Evidence: `read_only_open_rejects_a_replaced_data_directory` 在 directory swap 后返回 `database_identity_drift` 而不导出 replacement generation；`generation_gate_rejects_fifo_without_waiting` 在 regular-file precheck 后把 main file换成 FIFO 仍立即返回 typed error。两者以及既有 ABA/WAL regressions 均通过。
- Observation: 只在 descriptor-bound SQLite open 前盘点 companion 仍有 race；在同一 transaction 的最小 `PRAGMA schema_version` read 后重验，可以在任何 Library schema/data query 前拒绝已经出现的 sibling，而已建立的 SHARED snapshot 阻止随后 writer 的 EXCLUSIVE main-image update 混入结果。
  Evidence: `read_snapshot_rejects_a_journal_created_after_generation_gate` 在 gate 后 hook 创建 `skilload.db-journal`，覆盖 list/search/get/export/default doctor 全部返回 `database_corrupt`；SQLite 官方 locking 文档说明 SHARED reader、RESERVED journal writer 与 EXCLUSIVE commit 的锁边界。
- Observation: pathname absence 本身也是 generation decision；只在 `resolve_roots` 后检查 `skilload.db` 会把随后替换的空 `data/skilload` 误报为 empty Library。
  Evidence: `absent_read_rejects_data_root_replaced_after_resolution` 在 pre-fix 返回空 list；`database_exists_with_details` 在 probe 前后 revalidate 后返回 `XDG_DATA_HOME` identity error。
- Observation: `SchemaGeneration::Newer` 不能先应用 current v1/v2 base invariants。
  Evidence: `newer_schema_precedes_current_base_validation` 将 `library_entries` 重命名并把 version 设为 9；pre-fix list/search/get/mutation/import 不是 `schema_newer`，generation-first validation 后全部返回 version 9 的 typed refusal，doctor 报 `library_schema_newer`。


- Observation: FTS5 `unicode61 remove_diacritics 0` 不会 canonical-normalize arbitrary free-text；query 已 NFC 而 index仅含 decomposed bytes 时，literal MATCH 不命中。读取 snapshot 的 callback error 也必须先完成 generation identity revalidation，否则旧 inode 的 `not_found`/migration/schema error会被错误地归属为 replacement pathname。
  Evidence: `search_matches_nfc_forms_of_normalizable_free_text_fields` pre-fix 对 composed `café` 查询返回空集而非四个含 decomposed description/alias/category/note 的 entries；`failed_read_revalidates_database_generation_before_returning_error` pre-fix 返回 callback 的 `not_found`，两者修复后均通过。

- Observation: 缺失的 `data/backups` 目录代表尚未发布 recovery asset，可以视为空集合；已存在目录的打开或任一 iterator entry 失败则不能安全声称完整保护集合。另一个 writable SQLite pathname ABA gap 出现在首次 `SQLITE_FCNTL_HAS_MOVED` 与最终 held-root/main-entry revalidation 之间，必须在后者完成后再次验证 connection handle。
  Evidence: `output_rejects_an_unreadable_migration_backup_inventory_before_staging` 以不可读 backup directory 返回 `validation_failed`、保留 recovery file 且不创建 staging；`writable_open_rejects_an_aba_generation_restored_after_initial_handle_check` 在 replacement 被首次 handle check 后替换回原路径时返回 `database_identity_drift`，原路径与 displaced replacement inode 不同。

## Decision Log


- Decision: 使用 `PLAN-0005`、`codex/p4-library-indexed-reads`、`docs/exec-plans/plan/p4-library-indexed-reads.md`，唯一直接依赖为 `PLAN-0004`。
  Rationale: PLAN-0004 是当前 Library schema和 metadata mutations的直接前置；更早 Plans均为传递依赖，四个状态目录和 GitHub均无 PLAN-0005或 P4 delivery。
  Date/Author: 2026-08-20 / Codex
- Decision: 一个 PR同时交付 list/search/get、FTS schema v2、v1→v2 backup migration和 current-database doctor，而不加入 source/network/deployment domain。
  Rationale: 三个 reads形成独立用户闭环；persistent search无法在已有 v1 database上正确工作而不提供 migration和 FTS repair。P3已把这组工作识别为同一较大但独立的 reviewable delivery。
  Date/Author: 2026-08-20 / Codex
- Decision: `library search`采用纯文本词项 AND；不采用完整短语，也不公开 raw FTS5 expression。
  Rationale: 用户在 2026-08-20明确选择该产品语义。它允许常见多关键词搜索、避免相邻/顺序过窄，也避免把 SQLite operators、syntax errors和未来 grammar变化变成兼容契约。
  Date/Author: 2026-08-20 / User and Codex
- Decision: Query按固定 Unicode 15.1.0 `White_Space`切分；每个词项使用 NFC raw与 NFC full-case-fold alternatives并完全 quoted；空词项集合返回 `validation_failed/library_search_query_empty`。
  Rationale:同一 pinned Unicode数据满足 tag canonical/case equivalence，quoted encoding使所有用户字符保持数据身份；显式空查询错误避免 SQLite syntax error或把 search静默变成 list。
  Date/Author: 2026-08-20 / Codex
- Decision: Schema v2使用普通 content-bearing `library_fts`，由 adapter helper显式维护；不使用 external-content trigger、contentless table或第二套 tag aggregation。
  Rationale:普通 table允许 unindexed canonical identity和完整重建，不需要修改 v1 base schema。显式同事务维护比 tag triggers更容易复用现有 domain ordering/normalization，并使 doctor repair不改变 base owner。
  Date/Author: 2026-08-20 / Codex
- Decision: v1 list/get/export保持 read-only可用；v1 search和所有 writes要求显式 `doctor --fix`，read operations绝不自动迁移。
  Rationale:用户在升级前保留观察和 portable escape；显式 fix同时满足 lazy/read-only约束与 backup-before-upgrade。隐式 read migration会直接违反 `SKL-LIB-005`、`SKL-OPS-005`和 doctor design。
  Date/Author: 2026-08-20 / Codex
- Decision: Default doctor把 live read-only database online-backup到内存后运行 FTS special check；`doctor --fix`才取得 durable lock并创建 backup或写 live state。
  Rationale:FTS special integrity command要求 writable connection。内存副本提供完整诊断而不创建 XDG files、journal或修改 live timestamps。
  Date/Author: 2026-08-20 / Codex
- Decision: `library get`只做 exact canonical-source lookup；本计划不启用 alias或 derived-name selector。
  Rationale:`SKL-LIB-001`只把 derived-name selector设为 MAY，而 alias/name precedence和 candidate error尚未规范。Exact identity已可独立验收且不会猜测。
  Date/Author: 2026-08-20 / Codex
- Decision:代表性 10,000-entry release-build list/search/get每项预算10秒，永久 tests不把共享 runner wall clock作为 pass/fail。
  Rationale:该预算与 P3 scale acceptance一致并远宽于本地 metadata query预期；记录实测值可识别数量级回归，语义 tests则保持 deterministic/full-suite-safe。
  Date/Author: 2026-08-20 / Codex
- Decision:为 locked `rusqlite 0.40.2`增加 `backup` feature，并加入 exact `sha2 =0.11.0`且关闭 default features；不引入 time、hex或migration framework crate。
  Rationale:SQLite online backup是已有设计的唯一安全 live-generation snapshot；SHA-256可按固定buffer流式计算，epoch nanoseconds加 tempfile随机后缀足以生成无碰撞 UTC-derived backup名称，hex可直接写入固定数组/formatter。
  Date/Author: 2026-08-20 / Codex
- Decision: 不同词项的 FTS group以显式 `AND` 连接，不使用 implicit AND 组合。
  Rationale: 2026-08-20 规划评审（PRRT_kwDOT7YN2s6ay0q9）指出并经 isolated 实验证实：FTS5 只在裸 quoted phrases间支持 implicit AND，括号 OR group后接 phrase/group都是 syntax error；raw≠folded 词项必然产生 group，原方案会让多词查询违反 `SKL-LIB-004` Revision 2 的 no-grammar-error acceptance。
  Date/Author: 2026-08-20 / Codex
- Decision: 所有已存在 live database的 read-only opens前执行 pre-open generation gate；WAL-mode header或 `-wal`/`-shm` sibling直接按 `database_corrupt` 拒绝，不经 SQLite 打开。Doctor 对 base corruption返回 typed `database_corrupt` error（`DatabaseCorruptDetails`）而不是 DoctorData finding；`fix` 同样不对其返回 `unchanged`。
  Rationale: 2026-08-20 规划评审（PRRT_kwDOT7YN2s6ay0rD、PRRT_kwDOT7YN2s6ay0rJ）指出并经实验证实：read-only 打开 WAL-mode generation会创建并保留 `-shm`/`-wal`，`immutable=1` 则忽略 WAL 内容；同时 `SKL-OPS-004`、`docs/product-specs/database-recovery.md` 第1步与 API-v2 catalog都要求 doctor 以 `database_corrupt` details报告 base corruption，`DoctorFinding`（severity/code/message）无法携带 backup list、recoverable exports或 `database-corruption-v1`。skilload 只发布 DELETE-journal database，此类 generation只能来自外部，按 P2 既有 sidecar-hygiene 先例归入 corruption class。

- Decision: Base validation 不再运行整库 `PRAGMA integrity_check`，改为对 `sqlite_master` 与四个 v1 base 表逐表检查；FTS shadow b-tree 健康完全交给 derived 层（内容比对 + doctor 内存副本上的 FTS5 `integrity-check`）。
  Rationale: 2026-08-20 实现评审（PRRT_kwDOT7YN2s6a1M6h）指出整库 integrity_check 会把 FTS-only shadow 损坏提前升级为 `database_corrupt`，使 `doctor --fix` 拒绝可 rebuild 的损坏。逐表检查让 base 分类只取决于 base 对象；失去的唯一整库信号是 freelist/orphan-page 记账（"page is never used" 类），这不影响 base records 完整性证明，符合 `SKL-OPS-004` 的边界。
  Date/Author: 2026-08-20 / Codex

- Decision: FTS-only drift 在读取/写入路径上按 typed `invalid_state`（`library_fts_invalid`，exit 4）报告，而不是 `database_corrupt`；只有 base 层失败（schema shape、integrity/foreign-key、domain rows）与 pre-open gate 失败才返回 `database_corrupt`。list/get/export 只要求 base 验证，search 与所有 writes 额外要求 derived 一致。
  Rationale: `SKL-OPS-004` 把 `database_corrupt` 限定为 base records 不能证明完整的情形；派生索引可由 `doctor --fix` 修复，把它报告为 corruption 会错误阻止 operator 使用 doctor 修复路径，也会错误触发 database-corruption-v1。
  Date/Author: 2026-08-20 / Codex
- Decision: `library_fts` 的 `repository` 列索引 `skill.source.repository_display`（verified fresh spelling），而不是 canonical lowercase repository。
  Rationale: 用户按显示拼写搜索；对 ASCII 而言 unicode61 tokenizer 的大小写折叠使两种拼写产生相同 token，仅非 ASCII display 差异会有区别，而该差异正是 verified display 的可见内容。tag 的 display/key 双列已覆盖规范化等价需求。
  Date/Author: 2026-08-20 / Codex
- Decision: migration backup staging 使用 tempfile，最终以 no-clobber `linkat`（link 失败即保留 foreign entry 并报错）发布 `skilload-db-v1-to-v2-<epoch_ns>.db` 与 `.manifest.json`，随后仅 unlink 仍匹配 held descriptor identity 的 staging 名。
  Rationale: 与 first-import 的 linkat 发布先例一致且跨 macOS/Linux 可用（`RenameFlags::NOREPLACE` 是 Linux-only）；identity 比对防止误删外部替换。
  Date/Author: 2026-08-20 / Codex
- Decision: 所有 list/search/get/export/doctor 入口对错误路径统一执行 `enrich_database_corruption`：`DatabaseCorrupt` 若无 backups，则重新枚举 `data/backups/` 的完整 manifest pair 填充 details。
  Rationale: gate 与 base 校验在远离 roots 的代码路径抛出空 backups 的错误；`DatabaseCorruptDetails` 契约要求列出已知 backups，单点 enrichment 保证每条路径完整。
  Date/Author: 2026-08-20 / Codex
- Decision: 测试用 v1 fixture 由 v2 `initialize_schema` + `DROP TABLE library_fts` + `UPDATE schema_info SET version = 1` 生成；CLI 契约测试通过 `rusqlite` dev-dependency 直接执行 v1 DDL。
  Rationale: base 表结构在 v1/v2 完全相同，drop-then-downgrade 得到与 P2/P3 二进制产出逐字节同构的 v1 database，避免维护两份 DDL（core 内）且让 CLI 级迁移 smoke 使用真实 binary。
  Date/Author: 2026-08-20 / Codex

- Decision: physical FTS shadow damage、missing virtual table或任一 shadow schema/integrity 缺口先以 `writable_schema` 删除六条 FTS schema rows并 commit；在同一 durable lock 的无 transaction connection 上运行 `VACUUM`，重验 database identity后以第二笔 transaction重建。
  Rationale: PRRT_kwDOT7YN2s6bF0oR 证明同一 transaction直接重建把 detached b-tree pages遗留为整库 integrity `never used`，而 derived-only checks可能误报 healthy。`VACUUM` 是 SQLite 官方的 page-repacking path且不能在 open transaction 中运行；将 rebuild 后移使中断点保持 FTS missing/invalid，可由下一次 doctor 重试，不改变 base rows、`canonical_source` identity或 `state_revision`。
  Date/Author: 2026-08-21 / Codex

- Decision: read-only SQLite connection 从通过 generation gate 的 held regular-file descriptor 的 `/dev/fd/<fd>` 路径打开，而不是在 gate 后再次按 durable database pathname 打开。
  Rationale: PRRT_kwDOT7YN2s6a3XAK 证明 gate 与 pathname open之间的同账号 replacement 窗口可让 SQLite 在 later identity-drift error前为 WAL replacement 创建 sidecar。descriptor 在 SQLite open完成前保持持有，使实际读取的 inode与已检查 DELETE-journal generation相同；pathname replacement仍由既有 revalidation拒绝。
  Date/Author: 2026-08-21 / Codex

- Decision: 不自动 prune migration backup pairs；保留所有 complete validated pairs。
  Rationale: PRRT_kwDOT7YN2s6a3XAV 指出验证后按 pathname `unlink` 无法跨 macOS/Linux 原子绑定已验证 inode，可能删除 foreign replacement，违反 `ARCHITECTURE.md` 的 exact-owned removal不变式。备份数量不是本 Plan Product Baseline 的行为契约；保留比冒险删除安全，未来 cleanup需要独立的 ownership-bound filesystem protocol。
  Date/Author: 2026-08-21 / Codex

- Decision: base 与 FTS content validation 成功后的 `MATCH` corruption 统一映射为 `library_fts_invalid`。
  Rationale: PRRT_kwDOT7YN2s6a3XAd 指出 FTS inverted-index shadow b-tree可在 content projection仍可读时仅于 count/paged MATCH 失败。此时 base records已证明完整，按 `database_corrupt` 误导用户进入恢复 procedure；typed derived-index error与 `doctor --fix` repair contract一致。
  Date/Author: 2026-08-21 / Codex

- Decision: backup manifest 与 database validation 只读取 held backup-directory descriptor相对的 `openat(..., O_NOFOLLOW)` regular-file descriptors，并在返回验证结果前比较 directory-entry identity。
  Rationale: PRRT_kwDOT7YN2s6a3XAk 指出分离的 `symlink_metadata` 与 pathname read允许 symlink replacement race。一个 no-follow opened descriptor同时提供实际读取内容、file type、length与 streamed digest；final identity comparison拒绝在 validation期间已替换的 advertised entry。
  Date/Author: 2026-08-21 / Codex
- Decision: corruption diagnostics 只列出与当前 binary 兼容的 standalone migration backup；private manifest 最大 4 KiB；已完成的 v1→v2 migration 在等待 durable lock 的 contender 上按当前健康状态返回 `unchanged`；无 virtual table 但有 FTS shadow schema 的状态走现有 `writable_schema` 分离/rebuild。
  Rationale: PRRT_kwDOT7YN2s6bAVQR、PRRT_kwDOT7YN2s6bAVQT、PRRT_kwDOT7YN2s6bAVQV 与 PRRT_kwDOT7YN2s6bAVQX 分别证明 digest-only inventory、stale migration diagnosis、unbounded manifest read 和 orphaned FTS schema 都会破坏 recovery/repair contract。四项均是 Product Baseline 内 adapter hardening，不改变用户可见 behavior revision。
  Date/Author: 2026-08-21 / Codex
- Decision: 将任意 gate 时已观察到的 `skilload.db-journal` 与 `-wal`/`-shm` 一律归为 non-standalone generation，在 descriptor-bound read/default doctor 前返回 `database_corrupt`。
  Rationale: hot rollback journal 的 main image 可能需要 SQLite rollback；descriptor-bound file name无法安全关联 pathname sibling。虽然 active DELETE writer 的 normal SQLite pathname reader 可共存，当前 CLI 不在读取时冒险重新走 pathname open，也不引入跨进程/同进程 lock-protocol discrimination；保留 generation evidence 比该瞬时 read availability 安全。
  Date/Author: 2026-08-21 / Codex
- Decision: schema version 只接受 `1..=API_V1_UINT_MAX`；derived validation 只将 `DatabaseCorrupt` 映射为 `library_fts_invalid`，其他 typed operational error 原样传播；human Library read 统一投影完整 `LibraryEntry`。
  Rationale: 零版本违反 durable schema CHECK 的基础不变量，不能伪装成 newer schema；busy/I/O/memory 不证明 derived drift，不能诱导 `doctor --fix`；架构要求 human 与 JSON 表达相同 application outcome，所有 repository/user text 必须继续 terminal-safe quoted。
  Date/Author: 2026-08-21 / Codex

- Decision: 对只含 malformed `library_fts` schema SQL 的 state 使用 connection-local `writable_schema=ON` 完成 base-only validation，并把特殊 FTS repair 限定为删除 FTS/shadow rows 后 `RESET` reload。
  Rationale: PRRT_kwDOT7YN2s6bB2FR 证明普通 schema parser 会在 base proof 前失败，使可修复 derived failure误报为 base corruption；2026-08-21 SQLite probe 表明该 tolerance 可在 read-only connection 读取 intact base table，且不写 database。只有 base validation 通过才允许 repair，base rows 与 `state_revision` 始终不碰。
  Date/Author: 2026-08-21 / Codex

- Decision: 将 portable Library projection 与完整 operational base validation 分离；export/recovery diagnostics 只在 entries/tags schema、integrity、foreign keys 与 domain document 已证明完整时使用该 projection。
  Rationale: PRRT_kwDOT7YN2s6bB2FX 指出 `state_revision` 或 `schema_info` 损坏不必然损失可移植 Library 数据，而 `SKL-OPS-004` 要求列出每个仍可读 export。分离后 list/get/write 仍受完整 base validation 约束，只有 recovery-oriented export 绕过无关 operational metadata。
  Date/Author: 2026-08-21 / Codex

- Decision: 将 `SKL-LIB-009` 提升到 Revision 5，拒绝将 portable export 发布到已发布 migration backup pair 或同 inode alias。
  Rationale: PRRT_kwDOT7YN2s6bB2Fm 证明此前允许 JSON export 覆盖 `SKL-OPS-003` 必需 recovery asset，属于可观察但低风险的安全收紧；产品规格、Plan baseline、adapter tests 与 persistence design 同步为同一 revision。
  Date/Author: 2026-08-21 / Codex

- Decision: mutation precondition 复用 FTS5 special `integrity-check`，并让 human `database_corrupt` output 显示与 JSON 相同的 recovery assets。
  Rationale: PRRT_kwDOT7YN2s6bB2Fb 证明 visible FTS projection 不能覆盖 docsize-shadow drift；已可写 transaction 可以在 product row mutation 前执行该 check。PRRT_kwDOT7YN2s6bB2Fg 指出只在 JSON 暴露 backup 时 human operator 无法安全执行 recovery procedure；两项均不改变 API-v2 shape。
  Date/Author: 2026-08-21 / Codex
- Decision: read-only existing-database gate 必须在 root anchors 前后持有并重验 `data/skilload` directory descriptor，再以该 descriptor 相对的 `openat(..., O_RDONLY|O_NOFOLLOW|O_NONBLOCK)` 打开 main file；read-only SQLite 保持从 held file descriptor 的 `/dev/fd/<fd>` 打开，write path 继续使用 pathname + `HAS_MOVED`。
  Rationale: PRRT_kwDOT7YN2s6bDCYF 证明 main-file-only gate 会在 directory replacement 时采纳 foreign root generation；PRRT_kwDOT7YN2s6bDCYM 证明 `O_NOFOLLOW` 单独不能防止 FIFO block。directory/entry/path 三层重验把 read generation绑定到 root，nonblocking open 保持 CLI bounded；read-only `/dev/fd` 不使用 `HAS_MOVED`，因为 Linux SQLite 报告 temporary descriptor source而非 planned pathname。
  Date/Author: 2026-08-21 / Codex

- Decision: descriptor-bound read-only path 采用 pre-open 与 SHARED snapshot-bound 两次 companion inventory；backup pair validation 在同一 held-directory snapshot 内镜像该检查，`database_corrupt` recovery inventory始终从 held data-directory generation相对枚举。
  Rationale: PRRT_kwDOT7YN2s6bD4zh 证明仅 pre-open inventory 仍有 journal race；PRRT_kwDOT7YN2s6bD4zc 证明 standalone backup也不能忽略 companion；PRRT_kwDOT7YN2s6bD4zj 证明 error-path root re-resolution会混合 recovery evidence。SQLite locking documentation说明 SHARED reader与 RESERVED writer/EXCLUSIVE commit的边界，使 snapshot 内复核能够拒绝早到 companion并固定随后 reader返回的 main generation。
  Date/Author: 2026-08-21 / Codex

- Decision: 共用 absent-database probe 在 pathname 检查前后 revalidate 已解析 roots。
  Rationale: PRRT_kwDOT7YN2s6bF0oG 证明已存在 database 的 descriptor-bound gate不能覆盖其前面的 absent early return；共享 helper 让 list/search/get/export/default doctor 及 mutation path都拒绝 `resolve_roots` 后的 root replacement，同时保留 truly absent state 的 lazy/no-create behavior。
  Date/Author: 2026-08-21 / Codex

- Decision: 任一可读 schema version大于当前 `SCHEMA_VERSION` 时，在 v1/v2 base validation 前分类为 `SchemaGeneration::Newer`。
  Rationale: PRRT_kwDOT7YN2s6bF0oA 指出未来 schema 可合法替换当前 base table；若旧 binary先验证已知 table shape，会把兼容性拒绝误报 corruption。generation-first helper保持 safe export 的独立 projection，同时让 read/mutation/doctor/migration/repair paths一致拒绝 unknown newer write。
  Date/Author: 2026-08-21 / Codex


- Decision: 在 shared FTS row projection 中，仅当 indexed free-text value不是 NFC 时保留原值并以 ASCII newline追加 NFC representation；tag的 display/key 继续使用既有 normalized aggregation，base metadata不作改写。
  Rationale: `SKL-LIB-004` Revision 2 已规定 query term的 NFC alternative；raw plus NFC 使该 term命中所有可接受的非 NFC free-text，同时不改变 list/get/export 的原始值、FTS row identity或第二 owner。`unicode61 remove_diacritics 0` 不提供 canonical normalization。
  Date/Author: 2026-08-21 / Codex

- Decision: `run_read_snapshot` 保存 callback `Result`，在返回其成功或错误之前重验 held data-directory/main-file generation；successful path保留既有 commit 前后 revalidation。
  Rationale: descriptor-bound snapshot只有在结果归属的 pathname仍指向同一 generation时才可安全返回。若 callback 已得到 `not_found`、`migration_required` 或 `schema_newer` 后 pathname被替换，identity drift必须优先，不能向用户归因旧 inode的结果。
  Date/Author: 2026-08-21 / Codex

- Decision: 将 `data/backups` 的 NotFound 作为空 recovery inventory，其他目录或 entry 枚举失败统一返回 `library_export_protected_inventory_unavailable` validation error；所有 export protection gate 复用这一个 fallible inventory。
  Rationale: `SKL-LIB-009` Revision 5 要求绝不覆盖已发布 migration backup pair 或同 inode alias。不存在目录时无 asset 可保护，但 partial inventory 不足以作出安全覆盖决定，必须 fail closed。
  Date/Author: 2026-08-22 / Codex

- Decision: writable `open_existing_database` 保留 open 后的早期 `SQLITE_FCNTL_HAS_MOVED` 检查，并在 held directory entry/path identity revalidation 后重复同一 check。
  Rationale: 早期检查快速拒绝 stable replacement；第二次检查覆盖 replacement 在第一次通过后被攻击者恢复原 pathname 的 ABA 窗口。所有 existing import、metadata mutation、migration 和 FTS repair 都通过此 helper，因此不新增调用方分支。
  Date/Author: 2026-08-22 / Codex

## Outcomes & Retrospective


实现已完成（2026-08-20 13:36Z），全部 Product Baseline 行为已交付并验证。`SKL-LIB-004` Revision 2：嵌入式 FTS5 索引八类字段（含 tag display/key 双列），纯文本词项 AND 查询经 pinned Unicode 15.1.0 分词 + NFC/case-fold alternatives + 完整 FTS5 string quoting 编码（同词项括号 OR group、跨词项显式 `AND`），操作符/引号/列过滤全部保持 literal，空查询在 SQLite 前以 `validation_failed/library_search_query_empty` 拒绝。`SKL-LIB-005` Revision 1：`library list/search/get` 离线读取 canonical-source binary order、确定性分页（limit 1..=1000 默认 100、全量 u64 offset 默认 0、offset≥total 返回空页）、不创建任何 root、不改变 database bytes/mtime。`SKL-LIB-011` Revision 1：10,000-entry release 实测 exact get 0.06s、first list page 0.06s、full-text search 0.12s、deep-offset search 0.12s（Apple M4 Pro，预算各 10 秒），count/order 确定性由永久 tests 断言。`SKL-OPS-003` Revision 1：v1→v2 migration 在任何 live write 前发布 standalone online-backup pair（`data/backups/` + completed manifest：SHA-256/size/source identity/epoch-ns）并验证 digest/schema/base/revision，随后单事务创建+填充 FTS+更新 schema version，state revision 前后不变；failpoint 证据区分"v1+完整 backup"与"durable v2 但命令报错"；unknown newer schema 与 downgrade 保持拒绝。Doctor：默认只读（online backup 到 `:memory:` 运行 FTS5 integrity-check，filesystem 完全惰性，覆盖 absent/healthy/v1/FTS-drift/corrupt/WAL fixtures），`--fix` 交付 migrate/repair action 并可重复（healthy 重复 fix 返回 unchanged 且不产生第二个 backup）。Base corruption 与 WAL/sidecar generation 返回 typed `database_corrupt`（含已验证 backups 与 `database-corruption-v1`）；FTS-only drift 是 fixable finding。验证：`cargo test --workspace` 135 core + 28 CLI（11 bin + 17 contract）全部通过；focused 过滤覆盖 domain query（`library_search`）、repository/migration（`sqlite_library`）与 CLI（`library_reads`、`doctor`）；debug binary smoke 与 10k release 测量记录于 Artifacts。遗留：无——本计划范围内的所有 acceptance 均已满足；跨产品尚未满足的行为（`SKL-CLI-001` 完整 50 leaves、`SKL-OPS-002` 完整 ownership、`SKL-CACHE-006/007` 跨域 doctor）保持 planned 且未被误报为完成。

2026-08-21 review remediation 已完成：PRRT_kwDOT7YN2s6a3XAK、PRRT_kwDOT7YN2s6a3XAV、PRRT_kwDOT7YN2s6a3XAd 与 PRRT_kwDOT7YN2s6a3XAk 均由 `7e5a7bda7ce2dc3804a687a4e7249944a7908980` 修复并通过 focused/workspace validation。四个 GitHub replies 已成功写入、对应 inline threads 均为 resolved；完整会话读取验证所有 14 threads resolved、17 submitted reviews均已记录或不含问题。Plan 保持 `review`、PR 保持 ready。

2026-08-21 第四轮 review remediation 已完成：PRRT_kwDOT7YN2s6bAVQR、PRRT_kwDOT7YN2s6bAVQT、PRRT_kwDOT7YN2s6bAVQV 与 PRRT_kwDOT7YN2s6bAVQX 由 `648fb40323f2d35ac1dba6331501d0e03f7ecc6a` 修复。四个 inline replies 均写入并 resolved；新的 coverage 验证 backup compatibility/size cap、migration stale diagnosis 和 orphaned FTS shadow rebuild，完整 validation 为 focused 70 tests、workspace fmt/clippy/test（11+17+146）和 locked build。

最终 reconciliation 读取确认 PR #5 有 5 个 top-level comments、22 个 submitted reviews 与 18 个 review threads；所有 18 inline threads 都 resolved。五个 top-level source（`IC_kwDOT7YN2s8AAAABPzQF5Q`、`IC_kwDOT7YN2s8AAAABPz7oxg`、`IC_kwDOT7YN2s8AAAABPz_YTg`、`IC_kwDOT7YN2s8AAAABP1xCTg`、`IC_kwDOT7YN2s8AAAABP7Xvuw`）均是 bot trigger/notification，无独立问题；review body 的实际问题全部由本 Log 的 18 个 inline entries 覆盖。

2026-08-21 第五轮 review remediation 已完成：`ffeea3a7e850712db8b4b89c19dd6bfddf84136b` 修复 rollback journal generation gate、zero schema version classification、complete human `LibraryEntry` projection 与 derived validation operational-error propagation；`53aac2f625ca246fcaf00fc2865f636a60bab5e7` 推送预备 Plan evidence。四个 inline replies 均已写入并 resolved。final complete conversation read 观察到 6 个 top-level comments、27 个 submitted reviews、22 个 review threads；22 个实际 inline problem source 均有本 Plan entry、reply URL 与 resolved state，top-level trigger/notification、自动化 review wrapper 与 empty reply containers 不含独立问题。Plan 保持 `review`，PR 保持 ready。

2026-08-21 第六轮 final-review remediation 已由 `b581acb63df42a882e0f02d5167a931fdf6e47f0` 推送：malformed `library_fts` schema 现保持 derived-only、corruption diagnostics 仅在可验证 portable projection 时列出 `library.export`、writes 先运行 special FTS integrity check、human error 显示 recovery assets，且 `SKL-LIB-009` Revision 5 保护已发布 migration backup pair。focused coverage、core 153 tests、CLI 13+17 tests 与 workspace fmt/clippy/test/build 均通过；五个 inline reply URLs 与 resolved state 已逐项记录。最终完整会话读取为 7 个 top-level comments、33 个 reviews、27 个 threads；所有 threads resolved、无未记录/未回答/blocked actual problem。新增 trigger `IC_kwDOT7YN2s8AAAABP8V_-g`、自动化 wrapper `PRR_kwDOT7YN2s8AAAABKWXnaw` 及五个空 `@bootids` reply containers（`PRR_kwDOT7YN2s8AAAABKWmKEw`、`PRR_kwDOT7YN2s8AAAABKWmM3Q`、`PRR_kwDOT7YN2s8AAAABKWmPgw`、`PRR_kwDOT7YN2s8AAAABKWmSXg`、`PRR_kwDOT7YN2s8AAAABKWmVMA`）均未提出独立问题。

2026-08-21 第七轮 final-review remediation 已完成：`0a1cad3897588623b77c69b0fe90279a9d770257` 将 existing-database read gate 绑定到 root-validated `data/skilload` directory descriptor，并以 `O_NOFOLLOW|O_NONBLOCK` relative open 拒绝 FIFO；`read_only_open_rejects_a_replaced_data_directory` 与 `generation_gate_rejects_fifo_without_waiting` 覆盖两个 race，既有 ABA/WAL regressions 继续通过。两个 inline replies 已写入并 resolved。最终完整会话读取为 8 个 top-level comments、36 个 submitted reviews 与 29 个 review threads；所有 threads 均 resolved，且无未记录、未回答或 blocked actual problem。Plan 保持 `review`、PR 保持 ready。

2026-08-21 第八轮 final-review remediation 已完成：`8a0d84dc1e6de9959c0423f99273aa214c4f38b8` 在 Product Baseline 内修复 backup pair companion、descriptor-read sidecar race 与 corruption error-path root mixing。`backup_inventory_rejects_pairs_with_sqlite_sidecars`、`read_snapshot_rejects_a_journal_created_after_generation_gate`、`corruption_details_reject_a_replaced_data_directory` 通过；workspace fmt、clippy、test（13 + 17 + 158）与 locked build 均通过。三条 GitHub 回复 URL 与 `thread resolved: true` 已逐项写入本 Log。最终完整会话读取为 9 个 top-level comments、40 个 review bodies 与 32 个 threads；全部 32 个 actual inline sources 已记录、回复并 resolved，top-level trigger/notification 与 automated wrapper 未提出独立问题。Plan 保持 `review`、PR 保持 ready。

2026-08-21 第九轮 final-review remediation 已完成。runtime/design/reference/preliminary-log commit `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 使 version 9 的 replaced-base fixture按 `schema_newer`/`library_schema_newer` 分类、共用 absence probe 拒绝 resolved root replacement，并将 FTS repair 拆为 detach commit、`VACUUM` 和 rebuild transaction；damaged-shadow repair 后整库 integrity 为 `ok`。三项 focused regression 先 red 后 green，workspace fmt check、clippy、13+17+160 tests 与 locked build 通过。三个 reply URLs 与 resolved state 已逐项记录；最终完整会话读取为 10 个 top-level comments、44 个 reviews 与 35 个 threads，全部 thread resolved、所有 35 个 actual inline source 已记录；新增三个空 review containers 无独立问题。Plan 保持 `review`、PR 保持 ready。
2026-08-21 第十轮 final-review remediation 已完成：`9dc0fd058d54cf67f4d9e3edea5e9d7cdabc34f0` 让 non-NFC free-text FTS row 同时保存 raw/NFC search projection，并在任何 read snapshot callback error 返回前重验 held generation。`search_matches_nfc_forms_of_normalizable_free_text_fields` 与 `failed_read_revalidates_database_generation_before_returning_error` 都完成 red→green；workspace fmt、clippy、test（13 + 17 + 162）与 locked build 通过。两条 reply URL 和 resolved state 已逐项记录。最终完整会话读取为 11 个 top-level comments、47 个 reviews 与 37 个 threads；全部 thread resolved、所有 actual inline source 已在本 Log 记录，top-level trigger/notification 与十个同一自动化 wrapper body 不含独立问题。Plan 保持 `review`、PR 保持 ready。


## Review Conversation Log


2026-08-20 12:03Z 首轮规划评审（Codex bot review，commit `375e2e85c7ce085b562044808a0c792c79b0ce9d`）：top-level 评论 `IC_kwDOT7YN2s8AAAABPzQF5Q`（`@codex` bot 触发）与 review body `PRR_kwDOT7YN2s8AAAABKPf_-A`（自动化包装文本）未提出独立问题，无 resolvable thread 需要回复。三个 inline thread 的问题均已按 planning 边界处置并全部 resolved，详见以下条目。

### PRRT_kwDOT7YN2s6ay0q9 - FTS group 间需要显式 AND


Source: PRRT_kwDOT7YN2s6ay0q9 / PRRC_kwDOT7YN2s7jw2UK（https://github.com/bootids/skilload/pull/5#discussion_r3821233418）

Problem: Plan 第164行原规定不同词项“仅以空格连接形成 implicit AND”，但每个词项的 raw/folded alternatives以括号 OR group编码后，`("Review" OR "review") "code"` 被 FTS5 拒绝为 syntax error；FTS5 只在裸 quoted phrases间支持 implicit AND，任何 raw≠folded 多词查询都会违反 `SKL-LIB-004` Revision 2 “不触发 FTS grammar error” 的 acceptance。

Disposition: fixed

Status: resolved

Resolution: 本 Plan 的 Design Inputs 与词项定义段（评审时行号54/56/164）、`docs/design-docs/application-and-persistence.md`（Durable SQLite Model 段）与 `docs/references/sqlite-fts5-library-search.md` 已改为：同词项 alternatives以括号内 OR组合成一个 group，不同词项的 group之间以显式 `AND` 连接；reference 文档补充了实验证据。产品语义（纯文本词项 AND）不变，属技术组合规则修正，无需提升 behavior revision。

Evidence: isolated SQLite 实验证实 `("Review" OR "review") "code"` 返回 `fts5: syntax error near ""code""` 而 `("Review" OR "review") AND "code"` 正常命中；修订以 `24eb239a2fa056516a003b3439ec52155ab0a733` 推送到 PR head，`git diff --check` 无输出。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3821390563；thread resolved: true。

### PRRT_kwDOT7YN2s6ay0rD - base corruption 必须保留 database_corrupt


Source: PRRT_kwDOT7YN2s6ay0rD / PRRC_kwDOT7YN2s7jw2UT（https://github.com/bootids/skilload/pull/5#discussion_r3821233427）

Problem: Milestone 3 原文让 schema/base validation发现的 base corruption进入成功 `observed` DoctorData（unfixable finding），`fix` 对其返回 `unchanged`；这与本 Plan Product Baseline 第41行、`SKL-OPS-004`（`database_corrupt` diagnostics MUST 携带 backup/export/`database-corruption-v1`）及 `docs/product-specs/database-recovery.md` 第1步（`skilload doctor --json` 返回 `database_corrupt` details）矛盾，`DoctorFinding`（severity/code/message）无法携带 `DatabaseCorruptDetails` 的必需字段。

Disposition: fixed

Status: resolved

Resolution: 本 Plan Milestone 3 已改为：base corruption（schema/base validation 任何失败）不进入 DoctorData，而是按既有 error mapping返回 typed `database_corrupt` 错误，`DatabaseCorruptDetails` 含 database `PathValue`、已验证 backup manifests、仍可读 portable exports 与 `recovery_procedure: "database-corruption-v1"`；`fix` 在同一 diagnosis 阶段即返回该错误，绝不进入 action/`unchanged` 路径。newer schema 保持 unfixable `library_schema_newer` finding + `unchanged`。Validation 段补充 corrupt fixture 的 typed error 与 filesystem 不变性断言。

Evidence: `SKL-OPS-004`、`docs/product-specs/api-v2.md`（`database_corrupt` → `DatabaseCorruptDetails`）、`docs/product-specs/database-recovery.md` 第1步与本 Plan 第41行 baseline 一致；修订以 `24eb239a2fa056516a003b3439ec52155ab0a733` 推送到 PR head，`git diff --check` 无输出。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3821392730；thread resolved: true。

### PRRT_kwDOT7YN2s6ay0rJ - 只读 doctor 不得创建 WAL sidecars


Source: PRRT_kwDOT7YN2s6ay0rJ / PRRC_kwDOT7YN2s7jw2UZ（https://github.com/bootids/skilload/pull/5#discussion_r3821233433）

Problem: live database 为 WAL-mode（或带 `-wal`/`-shm` sibling）时，普通 read-only SQLite connection会在可写 data directory创建并保留 `skilload.db-shm`（实验还证实 header 为 WAL 而 sidecar缺失时会连 `-wal` 一起创建），破坏 default doctor 的 filesystem-inert 契约并污染待恢复 generation；`immutable=1` 虽不创建 sidecar但忽略 WAL 内容，不能作为诊断通道。

Disposition: fixed

Status: resolved

Resolution: 本 Plan 固化 pre-open generation gate：所有已存在 live database的 read-only opens（list/get/search/export/doctor inspect）先用既有 no-follow held descriptor直接读取 main file 100-byte header并盘点 `-wal`/`-shm` sibling；journal-mode bytes 非 (1,1) 或存在任一 WAL sibling的 generation不经 SQLite 打开，直接按 base corruption返回 `database_corrupt`（skilload 只发布 DELETE-journal database，该状态只能来自外部，且与 P2 “sidecar 统一报告 database_corrupt” 先例一致）。Design Inputs、Milestone 2/3、Validation 与 Product Baseline 可观察路径均已同步；实验事实固化到 `docs/references/sqlite-backup-and-corruption-recovery.md`。

Evidence: 2026-08-20 planning 实验：WAL+`-wal` fixture 以 `mode=ro` 打开后出现并保留 `skilload.db-shm`；WAL header 无 sidecar fixture 打开后出现 `-shm`+`-wal`；`immutable=1` 无 sidecar但 `no such table`（WAL 内容被忽略）。修订以 `24eb239a2fa056516a003b3439ec52155ab0a733` 推送到 PR head，`git diff --check` 无输出。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3821395106；thread resolved: true。

2026-08-20 13:50Z 第二轮实现评审（Codex bot review `PRR_kwDOT7YN2s8AAAABKQk3WA`，commit `7f9fd769b12eb75f051c1f29aaece9dd4a292c6b`）：top-level 评论 `IC_kwDOT7YN2s8AAAABPz7oxg`（`@codex` bot 触发）与 `IC_kwDOT7YN2s8AAAABPz_YTg`（"Didn't find any major issues" 通知）未提出独立问题，无 resolvable thread 需要回复。review body 本身为自动化包装文本，问题全部在 7 个 inline thread 中，以下逐条记录；本轮全部在 `review` 状态的 Product Baseline 边界内以普通修复处置。

### PRRT_kwDOT7YN2s6a1M6h - FTS shadow 损坏必须留在可修复 doctor 路径


Source: PRRT_kwDOT7YN2s6a1M6h / PRRC_kwDOT7YN2s7j0Wov（https://github.com/bootids/skilload/pull/5#discussion_r3822152239）

Problem: `validate_base_database` 使用整库 `PRAGMA integrity_check`，它会一并检查 FTS5 shadow 表；当仅 FTS shadow b-tree 损坏而 `library_entries`、`library_tags` 与 FTS content rows 完好时，base validation 在 `derived_index_is_consistent` 分类前就返回 `database_corrupt`，`doctor --fix` 因此拒绝本应可 rebuild 的 FTS-only 损坏，违反本 Plan Decision（FTS-only drift 归 `library_fts_invalid`）与 `SKL-OPS-004` 把 `database_corrupt` 限定为 base records 不能证明完整的边界。

Disposition: fixed

Status: resolved

Resolution: 已实现：`validate_base_database`（`crates/skilload-core/src/adapters/sqlite_library.rs`）改为对 `sqlite_master` 与四个 v1 base 表逐表运行 `PRAGMA integrity_check('<table>')`（新增 `BASE_INTEGRITY_TABLES`），FTS shadow 健康不再影响 base 分类；`derived_index_is_consistent` 中内存副本上的 FTS5 `integrity-check` 失败改为返回 `false`（可修复 finding）而不是向上传播。由于物理 damaged 的 shadow b-tree 连 `DROP TABLE`/`DELETE` 都以 `SQLITE_CORRUPT` 失败（实验证实），`rebuild_derived_index` 新增 damage 检测（`fts_shadow_btree_is_damaged`：逐 shadow `PRAGMA integrity_check` + 存在性检查）与 `writable_schema` schema-row 手术（`detach_damaged_fts_schema`：同一 transaction 内删除 vtab + 5 个 shadow 的 schema 行、bump `schema_version` 强制重解析后重建），使 `doctor --fix` 对物理 shadow 损坏也真正可 rebuild；旧 shadow pages 成为 orphan pages，不影响逐表 base 验证。逐表检查失去的唯一整库信号是 freelist/orphan-page 记账，不影响 base records 完整性证明。

Evidence: 新增回归测试 `fts_shadow_corruption_stays_doctor_fixable`：损坏 `library_fts_data` root page 尾部 cell 字节后，先断言 fixture 前提（整库 `PRAGMA integrity_check` 非 `ok`，输出 `Tree 9 page 9 cell 1: Extends off end of page`），再验证 `inspect()` 返回单个 `library_fts_invalid` finding（非 `database_corrupt`）、`fix()` 返回 `repair` action、search 恢复命中、`inspect()` 健康。既有 135 个 core tests（含 `corrupt_base_keeps_typed_details_with_known_backups` 的 page-1 corruption 仍被 `sqlite_master` 逐表检查捕获）全部通过；`cargo test --workspace` 11+17+141（debug 与 release）、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cargo build --workspace --locked` 均通过。修复以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head（`git diff --check` 无输出）。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3822533520；thread resolved: true。

### PRRT_kwDOT7YN2s6a1M6o - 新建 backups 目录的目录项必须同步


Source: PRRT_kwDOT7YN2s6a1M6o / PRRC_kwDOT7YN2s7j0Wo3（https://github.com/bootids/skilload/pull/5#discussion_r3822152247）

Problem: 首次 v1 migration 时 `data/backups` 通常不存在，但 `publish_validated_backup` 丢弃了 `ensure_restrictive_directory` 返回的 `CreatedDirectory` 记录；fsync backups 目录只持久化其内容，不会让其在新父目录 `data/skilload` 中的新目录项 crash-durable，掉电后可能出现 schema v2 live database 却丢失必需 backup 的窗口。

Disposition: fixed

Status: resolved

Resolution: 已实现：`publish_validated_backup` 保留 `ensure_restrictive_directory` 的 `CreatedDirectory` 记录，在 staging 文件写入前调用 `sync_created_directory_entries(&created_directories, "XDG_DATA_HOME")`，使新建 `backups` 目录在其父 `data/skilload` 中的目录项在任何 staging 写入与 live schema write 之前 crash-durable。

Evidence: 修改位于 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `publish_validated_backup` 开头；fsync 行为无法在测试中断言，回归由既有 `doctor_fix_migrates_v1_after_a_validated_backup`、`migration_failpoints_leave_a_coherent_state` 与新增 prune/backup 测试覆盖的迁移路径完整性保证。全部 workspace tests 通过。修复以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head（`git diff --check` 无输出）。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3822536249；thread resolved: true。

### PRRT_kwDOT7YN2s6a1M6u - 已验证 backup 列表必须校验 SHA-256 并拒绝 symlink manifest


Source: PRRT_kwDOT7YN2s6a1M6u / PRRC_kwDOT7YN2s7j0Wo8（https://github.com/bootids/skilload/pull/5#discussion_r3822152252）

Problem: `known_validated_backups` 只检查 `complete` 与 `database_bytes`，从不比对 manifest 已携带的 SHA-256；backup 被等长篡改后仍会作为 validated backup 出现在每个 `database_corrupt` 响应中，且 `fs::read` 跟随 symlink，symlink manifest 也会被接受。

Disposition: fixed

Status: resolved

Resolution: 已实现：新增共享 predicate `backup_pair_is_valid(backups_root, stem)`——manifest 必须是 no-follow regular file（symlink 拒绝）、记录可解析、`complete`、database 为 regular file 且长度相等、`sha256` 与 `sha256_of_file` 流式哈希一致；`known_validated_backups` 与 `prune_old_backups` 共用同一 predicate。

Evidence: 新增回归测试 `tampered_or_symlinked_backups_are_never_validated`：(a) 等长翻转 backup 末字节（digest 漂移）后损坏 live database，`inspect()` 的 `DatabaseCorrupt.backups` 为空；(b) manifest 换成 symlink 后同样为空。既有 `corrupt_base_keeps_typed_details_with_known_backups` 仍验证未篡改 backup 列出 1 项。全部 workspace tests 通过。修复以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head（`git diff --check` 无输出）。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3822539977；thread resolved: true。

### PRRT_kwDOT7YN2s6a1M6y - prune 必须保护本次 migration 刚发布的 backup


Source: PRRT_kwDOT7YN2s6a1M6y / PRRC_kwDOT7YN2s7j0WpD（https://github.com/bootids/skilload/pull/5#discussion_r3822152259）

Problem: `prune_old_backups` 先对全部 manifest 形态文件名应用保留截断再逐个验证，排序只依赖文件名中的 wall-clock timestamp；时钟回拨或外部 future-dated manifest 会使本次 migration 刚创建的 backup 落入删除切片并被立即删除，可能删掉上一 schema generation 的唯一 backup。

Disposition: fixed

Status: resolved

Resolution: 已实现：`prune_old_backups(backups_root, protected_stem)` 先用 `backup_pair_is_valid` 验证每个 pair 再进入保留排序（无效/外部条目不再占用保留名额，symlink pair 不再被误删）；`publish_validated_backup` 返回最终 stem，migration 把它作为 `protected_stem` 传入 prune，该 stem 永不进入删除集合；仅第 `RETAINED_COMPLETE_BACKUPS` 名之后的已验证 pair 会被删除。

Evidence: 新增回归测试 `prune_keeps_the_backup_of_the_current_migration`：预置 3 个 future-dated（20 位 epoch ns stem）但完全有效的 foreign pair 后执行 `fix()`，断言迁移后 4 个 pair 全部保留（旧逻辑会把排序最旧的本次迁移 backup 删除）、schema 为 v2、`inspect()` 健康。全部 workspace tests 通过。修复以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head（`git diff --check` 无输出）。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3822542981；thread resolved: true。

### PRRT_kwDOT7YN2s6a1M60 - mutation 路径的 corruption 错误必须补全 backups


Source: PRRT_kwDOT7YN2s6a1M60 / PRRC_kwDOT7YN2s7j0WpE（https://github.com/bootids/skilload/pull/5#discussion_r3822152260）

Problem: `import`/`mutate_metadata` 的既有库分支与 `fix` 的 action 调用直接返回 `DatabaseCorrupt`，其 `backups` 为空；migration backup 恰好在写被拒绝时不被列出，与读取路径已统一执行的 `enrich_database_corruption` 不一致，违反 `DatabaseCorruptDetails` 必须列出已知 backups 的契约。

Disposition: fixed

Status: resolved

Resolution: 已实现：`import` 的三个 database 分支（dry-run `read_existing`、`import_existing`、`import_first`）与 `mutate_metadata` 的 `mutate_existing` 均经 `enrich_database_corruption` 包装；`fix()` 的 `migrate_v1`/`repair_fts` action 错误同样包装，所有公开入口的 `DatabaseCorrupt` 现在都带完整 `backups`。

Evidence: 新增回归测试 `mutation_paths_list_known_backups_on_base_corruption`：migration 产生 1 个 backup 后损坏 live database，`mutate_metadata` 与 `import` 的 `DatabaseCorrupt.backups` 均列出该 1 个 validated backup（修复前为空）。全部 workspace tests 通过。修复以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head（`git diff --check` 无输出）。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3822545191；thread resolved: true。

### PRRT_kwDOT7YN2s6a1M62 - 取得修复锁后必须重新诊断 FTS drift


Source: PRRT_kwDOT7YN2s6a1M62 / PRRC_kwDOT7YN2s7j0WpH（https://github.com/bootids/skilload/pull/5#discussion_r3822152263）

Problem: 两个 `doctor --fix` 进程都在拿锁前诊断出 FTS drift 时，第一个修复后第二个进入 locked 路径，其中只校验 schema 与 base rows，随后无谓地 drop/rebuild 健康索引并返回 `changed`（stale finding 标记为 fixed），而不是幂等的 `unchanged`。

Disposition: fixed

Status: resolved

Resolution: 已实现：`repair_fts_locked` 改为 `Result<Option<DoctorAction>>`——先用只读 transaction 完成 schema/base 校验，再于 durable lock 内重新执行 `derived_index_is_consistent`，已一致时重验 identity 后返回 `None`（数据库字节不变），只有仍不一致才进入 rebuild transaction；`fix()` 收到 `None` 时对该 FtsInvalid 情形重新执行 `diagnosis_classification` 并以 `unchanged` 返回最新 findings 与 `database_writable`。

Evidence: 新增回归测试 `fts_repair_rechecks_drift_under_the_lock`：健康 v2 database 上直接调用 `repair_fts` 返回 `None` 且文件字节逐字节不变；制造 drift 后同一调用返回 `repair` action 且 search 恢复。`fix()` 的幂等 `unchanged` 分支由既有 `fts_drift_is_doctor_fixable_without_touching_base_rows` 的 repeated-fix 断言覆盖。全部 workspace tests 通过。修复以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head（`git diff --check` 无输出）。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3822547563；thread resolved: true。

### PRRT_kwDOT7YN2s6a1M64 - doctor 返回诊断前必须重验 database identity


Source: PRRT_kwDOT7YN2s6a1M64 / PRRC_kwDOT7YN2s7j0WpO（https://github.com/bootids/skilload/pull/5#discussion_r3822152270）

Problem: `diagnosis_classification` 丢弃 `open_existing_database` 返回的 identity，不像 list/get/export 那样在快照前后重验；同账号其他进程原子替换 `skilload.db` 后，诊断继续描述旧 inode 却可能返回以替换后路径为 target 的健康结果或 finding，使恢复用的 doctor evidence 不安全。

Disposition: fixed

Status: resolved

Resolution: 已实现：`diagnosis_classification` 保留 `open_existing_database` 返回的 identity，并在返回诊断前按既有读取路径模式于 transaction commit 前后各执行一次 `revalidate_database_identity`；路径名被原子替换为其他 inode 时返回 `database_identity_drift` invalid_state 而不是基于旧 inode 的诊断。

Evidence: 新增回归测试 `doctor_never_reports_a_replaced_database`：以 `after_existing_database_open` hook 在打开后用 `rename` 原子替换 `skilload.db`，`inspect()` 与 `fix()` 均返回 `database_identity_drift`（open 尾部的既有重验与本条新增的快照前后重验共同构成该契约；若两级重验均被移除则测试失败）。全部 workspace tests 通过。修复以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head（`git diff --check` 无输出）。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3822550014；thread resolved: true。

### PRRT_kwDOT7YN2s6a3XAK - 将 generation gate 绑定到 SQLite 打开的 inode


Source: PRRT_kwDOT7YN2s6a3XAK / PRRC_kwDOT7YN2s7j3jwt（https://github.com/bootids/skilload/pull/5#discussion_r3822992429）

Problem: 只读 `list`、`get`、`export` 与 default `doctor` 在 `pre_open_generation_gate` 关闭已检查 descriptor 后才按 pathname 打开 SQLite；同账号进程可在该窗口将 `skilload.db` 替换为 WAL generation，使 SQLite 在较晚的 identity drift 报错前创建 `-shm`/`-wal` sidecar，违反只读命令不创建 state 的架构不变式。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs`：`pre_open_generation_gate` 返回并保留经 `O_NOFOLLOW` 检查的 regular-file descriptor，`open_existing_database` 的只读 SQLite connection 从该 descriptor 的 `/dev/fd/<fd>` 打开而非随后可替换的 database pathname，随后仍重验 pathname identity。新增 WAL replacement race regression，并将既有 ABA test 改为确认恢复原 pathname 时 export 仍使用已检查 generation。

Evidence: `read_only_open_never_creates_sidecars_for_a_replaced_wal_generation` 断言 hook 替换为 WAL generation 后返回 `database_identity_drift` 且 replacement 没有 `-wal`/`-shm`；`export_uses_checked_generation_when_a_read_only_aba_is_restored` 断言短暂 ABA 后 export 成功读取原 generation。focused core tests 通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（11 + 17 + 142）与 `cargo build --workspace --locked` 通过；`git diff --check` clean。修复提交并已推送：`7e5a7bda7ce2dc3804a687a4e7249944a7908980`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3823320981；thread resolved: true。

### PRRT_kwDOT7YN2s6a3XAV - 不得按 stale pathname 删除已验证 backup


Source: PRRT_kwDOT7YN2s6a3XAV / PRRC_kwDOT7YN2s7j3jw_（https://github.com/bootids/skilload/pull/5#discussion_r3822992447）

Problem: `prune_old_backups` 在 `backup_pair_is_valid` 返回后按重建的 pathname 调用 `remove_file`；同账号进程可在验证与删除之间替换 backup 或 manifest，使 prune 删除未被验证的 foreign replacement，违反只能删除已证明 owned path 的架构不变式。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs` 与 `docs/design-docs/application-and-persistence.md`：移除 `RETAINED_COMPLETE_BACKUPS`、`prune_old_backups` 和 migration 后按 pathname deletion；migration 保留所有 complete validated pairs。该安全技术取舍不改变 Product Baseline 的 migration/backup 可观察行为。

Evidence: `migration_retains_all_validated_backup_pairs` 预置三个按名称排序更旧的 valid pairs，migration 后断言四个 pairs 均保留（旧 prune 会删一个）。focused core tests 与全部 workspace format/clippy/test/build validation 通过；`git diff --check` clean。修复提交并已推送：`7e5a7bda7ce2dc3804a687a4e7249944a7908980`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3823322405；thread resolved: true。

### PRRT_kwDOT7YN2s6a3XAd - 将 MATCH corruption 归类为 derived-index drift


Source: PRRT_kwDOT7YN2s6a3XAd / PRRC_kwDOT7YN2s7j3jxH（https://github.com/bootids/skilload/pull/5#discussion_r3822992455）

Problem: base validation 与 FTS content-row comparison 成功后，损坏的 inverted-index shadow b-tree 仍可使 count 或 paged `MATCH` 返回 `SQLITE_CORRUPT`；当前 generic `database_error` 将其误报为 base `database_corrupt`，尽管同一状态会被 `doctor --fix` 诊断和修复为 `library_fts_invalid`。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs`：`fts_match_error` 仅将 base/derived content validation完成后的 count、paged MATCH preparation 与 row iteration中的 `SQLITE_CORRUPT`/`SQLITE_NOTADB` 转为 `library_fts_invalid`；其他 query 与 base validation继续使用原有 `database_corrupt` mapping。

Evidence: `fts_shadow_corruption_stays_doctor_fixable` 现在在 physical FTS shadow corruption后先断言 search 返回 `invalid_state/library_fts_invalid`，再验证 `doctor --fix` repair、search recovery 与 healthy diagnosis；`corrupt_base_keeps_typed_details_with_known_backups` 仍断言 base corruption为 `database_corrupt`。focused core tests 与全部 workspace format/clippy/test/build validation 通过；`git diff --check` clean。修复提交并已推送：`7e5a7bda7ce2dc3804a687a4e7249944a7908980`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3823323876；thread resolved: true。

### PRRT_kwDOT7YN2s6a3XAk - 以 no-follow descriptor 读取 backup manifest


Source: PRRT_kwDOT7YN2s6a3XAk / PRRC_kwDOT7YN2s7j3jxO（https://github.com/bootids/skilload/pull/5#discussion_r3822992462）

Problem: `backup_pair_is_valid` 先以 `symlink_metadata` 检查 manifest，再以 `fs::read` 按 pathname 打开；同账号进程可在两者之间替换为 symlink，使 predicate 读取未证明是 regular file 的 manifest 并错误认证 backup pair。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs`：`backup_manifest_stems` 从 held directory descriptor 枚举，`open_regular_file_at` 以 `openat(..., O_NOFOLLOW | O_NONBLOCK)` 打开 manifest/database，`backup_pair_is_valid` 在这些 opened descriptors上确认 regular type、读取 manifest、计算 SHA-256并重验 directory-entry identity；`known_validated_backups` 复用该路径。

Evidence: `tampered_or_symlinked_backups_are_never_validated` 继续断言等长 digest drift 与 symlink manifest均不会进入 `DatabaseCorrupt.backups`；`migration_retains_all_validated_backup_pairs` 覆盖 held-directory validation下的 migration path。focused core tests 与全部 workspace format/clippy/test/build validation 通过；`git diff --check` clean。修复提交并已推送：`7e5a7bda7ce2dc3804a687a4e7249944a7908980`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3823325270；thread resolved: true。

2026-08-21 最终完整会话读取：PR #5 当前有 4 个 top-level comments、17 个 submitted reviews 与 14 个 review threads；14 个 threads 均 resolved。新增 submitted review bodies `PRR_kwDOT7YN2s8AAAABKSBnIQ`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4984956705）、`PRR_kwDOT7YN2s8AAAABKSBt-Q`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4984958457）、`PRR_kwDOT7YN2s8AAAABKSB0Ww`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4984960091）和 `PRR_kwDOT7YN2s8AAAABKSB7IA`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4984961824）均为 `@bootids` 的空 `COMMENTED` body，未提出独立问题；它们对应本轮 inline reply/resolve 容器，不需额外 disposition。

### PRRT_kwDOT7YN2s6bAVQR - 仅列出兼容且可验证的 migration backup

Source: PRRT_kwDOT7YN2s6bAVQR / PRRC_kwDOT7YN2s7kE-qu（https://github.com/bootids/skilload/pull/5#discussion_r3826510510）

Problem: `known_validated_backups` 目前只验证 `complete`、大小、digest 和目录项 identity；任意 hash 匹配的 newer schema 或 foreign SQLite pair 仍会作为恢复 backup 出现在 `database_corrupt` diagnostics 中，违反 `database-recovery.md` 只考虑 recorded schema 不新于 binary 且可实际验证的 standalone candidate 的规则。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs`：`backup_pair_is_valid` 现在要求 current manifest format、source schema 1、target schema 2、complete/size/digest/entry identity，并通过 held descriptor 的 `standalone_backup_is_valid` 验证 DELETE-journal header、SQLite schema v1 与完整 base rows；`docs/design-docs/application-and-persistence.md` 同步该 inventory 规则。修复提交：`648fb40323f2d35ac1dba6331501d0e03f7ecc6a`。

Evidence: `incompatible_or_nonstandalone_backups_are_never_advertised`；focused SQLite adapter tests 70 passed。workspace fmt/clippy/test（11+17+146）与 locked build 通过；`git diff --check` clean。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826602054；thread resolved: true。

### PRRT_kwDOT7YN2s6bAVQT - 在 migration lock 内重诊断 schema

Source: PRRT_kwDOT7YN2s6bAVQT / PRRC_kwDOT7YN2s7kE-qw（https://github.com/bootids/skilload/pull/5#discussion_r3826510512）

Problem: 两个 `doctor --fix` 都在取得 `database.lock` 前观察到 schema v1 时，先取得锁的进程成功迁移为 v2；第二个进程取得锁后把这一正常串行化结果误报为 `migration_baseline_changed`，而非幂等的 `unchanged`。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs`：`migrate_v1_locked` 在 durable lock 内确认完整 v2 后返回 `None`，而 `fix()` 对由 migration 或 FTS repair 消除的 stale finding 重跑 diagnosis 并返回当前 `unchanged`；backup 发布后仍保留原有 baseline drift error。修复提交：`648fb40323f2d35ac1dba6331501d0e03f7ecc6a`。

Evidence: `migration_rechecks_state_after_acquiring_lock` 先记录 v1 diagnosis、完成一次 migration、再模拟等待者进入 locked path 并断言 `None`、healthy diagnosis 与 public `fix()` 的 unchanged。focused 70 tests 与 workspace fmt/clippy/test（11+17+146）/locked build 通过；`git diff --check` clean。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826602675；thread resolved: true。

### PRRT_kwDOT7YN2s6bAVQV - 限制 backup manifest 的读取大小

Source: PRRT_kwDOT7YN2s6bAVQV / PRRC_kwDOT7YN2s7kE-qy（https://github.com/bootids/skilload/pull/5#discussion_r3826510514）

Problem: 每个 `database_corrupt` path 都会对候选 manifest 无界 `read_to_end`；截断、稀疏或多 GiB 的 regular manifest 可在诊断恢复信息时消耗内存或阻塞 `doctor`。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs`：候选 manifest 在 allocation 前按 held descriptor 长度拒绝超过 4 KiB 的文件，随后以 `Read::take(4 KiB + 1)` bounded read 防止 metadata/read race 产生无界 allocation；超限 pair 不会进入 backup inventory。修复提交：`648fb40323f2d35ac1dba6331501d0e03f7ecc6a`。

Evidence: `oversized_backup_manifest_is_never_advertised` 在损坏 live database 后验证 4 KiB+1 regular manifest 不会出现在 `DatabaseCorrupt.backups`。focused 70 tests 与 workspace fmt/clippy/test（11+17+146）/locked build 通过；`git diff --check` clean。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826603317；thread resolved: true。

### PRRT_kwDOT7YN2s6bAVQX - 重建前分离 orphaned FTS shadow tables

Source: PRRT_kwDOT7YN2s6bAVQX / PRRC_kwDOT7YN2s7kE-q2（https://github.com/bootids/skilload/pull/5#discussion_r3826510518）

Problem: `library_fts` virtual-table row 缺失而 `library_fts_*` shadow table 仍在时，diagnosis 正确报告 `library_fts_invalid`，但 rebuild 把它误判为未损坏；`DROP TABLE IF EXISTS library_fts` 成为 no-op，随后 `CREATE VIRTUAL TABLE` 与残余 shadow name 冲突而失败。

Disposition: fixed

Status: resolved

Resolution: 已实现于 `crates/skilload-core/src/adapters/sqlite_library.rs`：`fts_schema_requires_detach` 将缺失 `library_fts` virtual-table row 而存在任一 shadow row 的状态加入 `writable_schema` removal；随后 schema cookie 更新、fixed virtual table 创建与 base projection refill 复用既有 repair transaction。`docs/design-docs/application-and-persistence.md` 同步该 derived repair rule。修复提交：`648fb40323f2d35ac1dba6331501d0e03f7ecc6a`。

Evidence: `orphaned_fts_shadow_tables_are_doctor_fixable` 删除 virtual-table schema row、保留 `library_fts_data`，断言 diagnosis `library_fts_invalid`、`doctor --fix` 返回 repair、search 恢复且 final diagnosis healthy。focused 70 tests 与 workspace fmt/clippy/test（11+17+146）/locked build 通过；`git diff --check` clean。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826603954；thread resolved: true。

2026-08-21 最终完整会话读取：PR #5 当前有 5 个 top-level comments、22 个 submitted reviews 与 18 个 review threads，所有 threads 均为 resolved。新出现的空 `@bootids` `COMMENTED` review containers `PRR_kwDOT7YN2s8AAAABKV1igQ`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4988953217）、`PRR_kwDOT7YN2s8AAAABKV1lHg`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4988953886）、`PRR_kwDOT7YN2s8AAAABKV1n1Q`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4988954581）和 `PRR_kwDOT7YN2s8AAAABKV1qmg`（https://github.com/bootids/skilload/pull/5#pullrequestreview-4988955290）仅为本轮 inline reply containers，body 为空且无独立问题；不需 GitHub reply 或 disposition。全部 18 个实际 inline source 均有本 Log entry、成功 reply 和 resolved state。

2026-08-21 第五轮最终完整会话读取：PR #5 当前有 6 个 top-level comments、27 个 submitted reviews 与 22 个 review threads；所有 22 个 threads 均为 resolved。新增 top-level trigger `IC_kwDOT7YN2s8AAAABP7uOKg`、自动化 review body `PRR_kwDOT7YN2s8AAAABKV_Hcg` 与四个 `@bootids` empty reply containers 均未提出独立问题；本轮 4 个 inline 问题均以 `fixed`、`resolved` 完成，以下条目记录代码提交、验证与 GitHub outcome。

### PRRT_kwDOT7YN2s6bA7j1 - descriptor-bound read 前拒绝 rollback journal

Source: PRRT_kwDOT7YN2s6bA7j1 / PRRC_kwDOT7YN2s7kF4g5（https://github.com/bootids/skilload/pull/5#discussion_r3826747449）

Problem: 已有 `skilload.db-journal` 的 DELETE-mode generation 未被 `pre_open_generation_gate` 检查；后续 `/dev/fd/<fd>` read-only open 无法关联 pathname sibling，可能读取未恢复的 main image 或返回不带 recovery details 的 generic error。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `pre_open_generation_gate` 使用完整 `DATABASE_SIDECAR_SUFFIXES`（`-journal`、`-wal`、`-shm`）拒绝 descriptor-bound read 前的 non-standalone generation；新增 `rollback_journal_generation_is_rejected_before_sqlite_opens` 覆盖 export/list/search/get/default doctor 的 typed error、bytes 不变，且将既有 concurrent-writer regression 改为 `reads_reject_live_rollback_journals_before_descriptor_opens`，明确 active journal 的保守拒绝。同步 `SKL-OPS-004` clarification、`ARCHITECTURE.md`、persistence design 与 SQLite reference。修复提交已推送：`ffeea3a7e850712db8b4b89c19dd6bfddf84136b`。

Evidence: 代码提交 `ffeea3a7e850712db8b4b89c19dd6bfddf84136b`；`rollback_journal_generation_is_rejected_before_sqlite_opens` 与 `reads_reject_live_rollback_journals_before_descriptor_opens` 通过；`mise exec -- cargo fmt --all -- --check`、`mise exec -- cargo clippy --quiet --workspace --all-targets --all-features -- -D warnings`、`mise exec -- cargo test --quiet --workspace --all-features --locked`（12 CLI binary + 17 contract + 149 core）、`mise exec -- cargo build --quiet --workspace --all-features --locked` 与 `git diff --check`均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826859410；thread resolved: true。

### PRRT_kwDOT7YN2s6bA7j5 - schema version 零是 base corruption

Source: PRRT_kwDOT7YN2s6bA7j5 / PRRC_kwDOT7YN2s7kF4g-（https://github.com/bootids/skilload/pull/5#discussion_r3826747454）

Problem: `read_schema_generation` 接受 version `0` 并错误分类为 `SchemaGeneration::Newer(0)`，使读取返回 `schema_newer`、doctor 产生不可修复 `library_schema_newer` finding，而该值违反 `schema_info.version >= 1` 的 base-schema invariant。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 将 `read_schema_generation` 的范围从 `0..=API_V1_UINT_MAX` 收紧为 `1..=API_V1_UINT_MAX`；新增 `schema_version_zero_is_database_corrupt`，以 SQLite `ignore_check_constraints` 夹具构造外部损坏值并验证 export/list/search/get/default doctor/fix 均走 typed `database_corrupt`。修复提交已推送：`ffeea3a7e850712db8b4b89c19dd6bfddf84136b`。

Evidence: 代码提交 `ffeea3a7e850712db8b4b89c19dd6bfddf84136b`；`schema_version_zero_is_database_corrupt` 覆盖 export/list/search/get/default doctor/fix；完整 focused/workspace validation 与 `git diff --check`均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826860071；thread resolved: true。

### PRRT_kwDOT7YN2s6bA7j7 - human read 输出完整 LibraryEntry

Source: PRRT_kwDOT7YN2s6bA7j7 / PRRC_kwDOT7YN2s7kF4hA（https://github.com/bootids/skilload/pull/5#discussion_r3826747456）

Problem: `library list`、`library search` 与 `library get` 的 human renderer 只输出部分 `LibraryEntry`，漏掉 description、expanded `SourceIdentity`、repository ID、commit、integrity、entry/byte counts，导致 human 与 JSON 对同一 application outcome 的信息不一致。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-cli/src/human.rs` 的共享 `append_library_entry` 投影 canonical 与全部 `SourceIdentity` 字段、repository ID、commit、integrity、name、description、entry/byte counts、metadata/tags/trust state；所有字符串继续复用 `quote_string`。新增 `library_read_renderers_project_complete_terminal_safe_entries`，同时断言 list/search/get 输出完整字段且 description control newline 被转义。修复提交已推送：`ffeea3a7e850712db8b4b89c19dd6bfddf84136b`。

Evidence: 代码提交 `ffeea3a7e850712db8b4b89c19dd6bfddf84136b`；`library_read_renderers_project_complete_terminal_safe_entries` 覆盖 list/search/get 的完整字段与 control newline escaping；完整 focused/workspace validation 与 `git diff --check`均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826860831；thread resolved: true。

### PRRT_kwDOT7YN2s6bA7j- - 派生 FTS validation 保留 operational error

Source: PRRT_kwDOT7YN2s6bA7j- / PRRC_kwDOT7YN2s7kF4hE（https://github.com/bootids/skilload/pull/5#discussion_r3826747460）

Problem: 搜索与 doctor 将 `validate_derived_database` 的全部错误无条件转换为 `library_fts_invalid`；SQLite busy、I/O、memory 等 operational failure 因而被误报为可通过 `doctor --fix` 修复的 derived drift。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 添加 `map_derived_validation_error`：仅 `AppError::DatabaseCorrupt` 转为 `library_fts_invalid`；search 与 `validate_database` 复用该映射，doctor 的 `derived_index_is_consistent` 对 operational error 返回原 error、仅 corruption/content mismatch 返回 false finding。新增 `derived_validation_preserves_busy_errors`，以即时 busy locked reader 证明 doctor 不再产生假修复 finding。修复提交已推送：`ffeea3a7e850712db8b4b89c19dd6bfddf84136b`。

Evidence: 代码提交 `ffeea3a7e850712db8b4b89c19dd6bfddf84136b`；`derived_validation_preserves_busy_errors` 证明 default doctor 传播 `AppError::Busy` 而非产生 drift finding；完整 focused/workspace validation 与 `git diff --check`均通过。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3826861427；thread resolved: true。

### PRRT_kwDOT7YN2s6bB2FR - 修复 malformed FTS schema 的派生索引恢复

Source: PRRT_kwDOT7YN2s6bB2FR / PRRC_kwDOT7YN2s7kHRhd（https://github.com/bootids/skilload/pull/5#discussion_r3827112029）

Problem: 仅 `library_fts` 的 `sqlite_master` SQL 文本损坏时，普通 schema load 在 base validation 前以 `malformed database schema` 失败，错误归为 `database_corrupt`；base rows 完整的 FTS-only failure 因而不能由 `doctor --fix` 走现有 derived-index rebuild。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 以 connection-local `writable_schema` tolerance 完成 base-only inspection，repair 在确认 base rows 后识别 malformed FTS SQL、分离 FTS/shadow schema rows并以 `RESET` reload。新增 `malformed_fts_schema_stays_derived_and_doctor_fixable`，覆盖 list/get/export、search invalid、default doctor finding 和 `doctor --fix` repair。修复 commit：`b581acb63df42a882e0f02d5167a931fdf6e47f0`。

Evidence: `SKL-OPS-004` Revision 1 要求 base records 可证明完整时 FTS-only failure 可由 doctor rebuild；2026-08-21 local SQLite probe 证明 read-only `PRAGMA writable_schema=ON` 可读取 intact base table，而普通 schema load 失败。focused regression、core 153 tests、workspace fmt/clippy/test/build 均通过；`git diff --check` clean。commit `b581acb63df42a882e0f02d5167a931fdf6e47f0` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3827330061；thread resolved: true。

### PRRT_kwDOT7YN2s6bB2FX - corruption diagnostics 列出仍可导出的 Library

Source: PRRT_kwDOT7YN2s6bB2FX / PRRC_kwDOT7YN2s7kHRhj（https://github.com/bootids/skilload/pull/5#discussion_r3827112035）

Problem: `database_corrupt_with_known_backups` 与 `enrich_database_corruption` 均把 `recoverable_exports` 固定为空；例如仅 `state_revision` 无效而 `library_entries`/`library_tags` 完整时，portable Library 仍可读，但 `doctor --json` 没有列出可保留的恢复导出。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 用 descriptor-bound read-only connection 独立验证 portable Library projection（entry/tag schema、integrity、foreign keys 与 domain document），成功时将 `library.export` 加入 `DatabaseCorruptDetails.recoverable_exports`；同一 projection 已成为 `library export` 的 read path，不能证明完整时仍为空。新增 state-revision corruption regression。修复 commit：`b581acb63df42a882e0f02d5167a931fdf6e47f0`。

Evidence: `SKL-OPS-004` Revision 1 明定 diagnostics MUST “name every still-readable export”；`database-recovery.md` 要求保留 details 指向的 export。focused regression、core 153 tests、workspace fmt/clippy/test/build 均通过；`git diff --check` clean。commit `b581acb63df42a882e0f02d5167a931fdf6e47f0` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3827330759；thread resolved: true。

### PRRT_kwDOT7YN2s6bB2Fb - mutation 前验证 FTS special integrity-check

Source: PRRT_kwDOT7YN2s6bB2Fb / PRRC_kwDOT7YN2s7kHRhp（https://github.com/bootids/skilload/pull/5#discussion_r3827112041）

Problem: write validation 只比对 visible FTS projection；`library_fts_docsize` 等 logical shadow drift 可使 projection 与 ordinary MATCH 成功，但 FTS5 special `integrity-check` 失败，unrelated import/metadata mutation 仍会提交并报告成功。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 将 FTS5 special `integrity-check` 纳入 writable `validate_database` 的 schema-v2 precondition；detected derived mismatch 映射为 `library_fts_invalid`，busy/I/O 等 operational error 原样保留。新增 docsize-shadow drift regression，验证 import 与 metadata mutation 均在任何 product-state write 前拒绝。修复 commit：`b581acb63df42a882e0f02d5167a931fdf6e47f0`。

Evidence: Plan Design Inputs 要求 write path 先证明 derived index consistent，doctor 已用同一 FTS5 special command。focused regression、core 153 tests、workspace fmt/clippy/test/build 均通过；`git diff --check` clean。commit `b581acb63df42a882e0f02d5167a931fdf6e47f0` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3827331433；thread resolved: true。

### PRRT_kwDOT7YN2s6bB2Fg - human doctor error 显示 validated backups

Source: PRRT_kwDOT7YN2s6bB2Fg / PRRC_kwDOT7YN2s7kHRhv（https://github.com/bootids/skilload/pull/5#discussion_r3827112047）

Problem: JSON `DatabaseCorruptDetails` 显示 validated backup paths，但 `human::render_error` 仅显示 database 与 procedure，终端 operator 看不到应保留的 recovery asset。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-cli/src/human.rs` 为 `AppError::DatabaseCorrupt` terminal-safe 逐项渲染 backup paths 与 recoverable export identifiers，并加入 `database_corruption_renderer_lists_terminal_safe_recovery_assets`；API-v2 JSON 不变。修复 commit：`b581acb63df42a882e0f02d5167a931fdf6e47f0`。

Evidence: `cli-json-and-release.md` 要求 human error 提供 relevant paths 且 terminal-safe；`SKL-OPS-004` 与 recovery procedure 要求 operators 保留 backups/exports。focused renderer regression、CLI 13+17 tests、workspace fmt/clippy/test/build 均通过；`git diff --check` clean。commit `b581acb63df42a882e0f02d5167a931fdf6e47f0` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3827332140；thread resolved: true。

### PRRT_kwDOT7YN2s6bB2Fm - export 不得覆盖 migration backup pair

Source: PRRT_kwDOT7YN2s6bB2Fm / PRRC_kwDOT7YN2s7kHRh1（https://github.com/bootids/skilload/pull/5#discussion_r3827112053）

Problem: migration 发布的 `data/backups/*.db` 与 `.manifest.json` 不在 export protected-target inventory；用户可把 `library export --output` 指向其中任一条目并以 JSON 原子替换 recovery pair，破坏可能唯一的 pre-migration backup。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/portable_library.rs` 将已发布 migration backup-pair entries 纳入 pathname、resolved-path 与 inode collision protection，并新增 pair DB、manifest与 hard-link alias regression；`SKL-LIB-009` 已同步至 Revision 5、Product Baseline 与 Design Inputs。修复 commit：`b581acb63df42a882e0f02d5167a931fdf6e47f0`。

Evidence: `SKL-OPS-003` Revision 1 要求 backup 保持 recoverable，`ARCHITECTURE.md` 禁止替换不应由当前 operation 覆盖的 skilload-owned path。focused transfer regression、core 153 tests、workspace fmt/clippy/test/build 均通过；`git diff --check` clean。commit `b581acb63df42a882e0f02d5167a931fdf6e47f0` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3827332858；thread resolved: true。
### PRRT_kwDOT7YN2s6bD4zc - backup validation 拒绝 companion sidecars

Source: PRRT_kwDOT7YN2s6bD4zc / PRRC_kwDOT7YN2s7kKYU6（https://github.com/bootids/skilload/pull/5#discussion_r3827926330）

Problem: `backup_pair_is_valid` 只验证 held `.db` 与 manifest；published standalone backup 若有 sibling `-journal`、`-wal` 或 `-shm`，descriptor-bound `/dev/fd` SQLite open 不会关联 companion，可能把需要 recovery 的 generation 误报为可单独 restore 的 validated backup。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `backup_pair_is_valid`、`standalone_backup_is_valid` 与 `has_database_sidecar` 通过 held backup-directory descriptor 在打开、SHARED snapshot validation 与返回 inventory 前拒绝全部 SQLite companion；新增 `backup_inventory_rejects_pairs_with_sqlite_sidecars` 覆盖每个 suffix。修复提交已推送：`8a0d84dc1e6de9959c0423f99273aa214c4f38b8`。

Evidence: `SKL-OPS-004` Revision 1 要求 diagnostics 只列出 known backups；focused `backup_inventory_rejects_pairs_with_sqlite_sidecars` 通过。workspace `fmt`、`clippy`、`test`（13 + 17 + 158）与 locked build 均通过；修复提交 `8a0d84dc1e6de9959c0423f99273aa214c4f38b8` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3828481782；thread resolved: true。

### PRRT_kwDOT7YN2s6bD4zh - descriptor read 在 snapshot 内重验 sidecars

Source: PRRT_kwDOT7YN2s6bD4zh / PRRC_kwDOT7YN2s7kKYVC（https://github.com/bootids/skilload/pull/5#discussion_r3827926338）

Problem: live generation gate 仅在 `/dev/fd/<fd>` SQLite connection 前盘点 sidecars。`skilload.db-journal` 在 gate 与实际 read lock 之间出现时，descriptor-bound connection 无法触发 SQLite 的 pathname hot-journal recovery，list/get/search/export/default doctor 可能读取未提交 main image。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `begin_existing_read_snapshot`/`run_read_snapshot` 让每个 read-only path 在同一 transaction 用 `PRAGMA schema_version` 取得 SHARED snapshot，再经 held data-directory descriptor 重验 `-journal`、`-wal`、`-shm`，随后才读 schema/data；新增 `read_snapshot_rejects_a_journal_created_after_generation_gate` 覆盖 gate 后 sidecar 与 list/search/get/export/default doctor。修复提交已推送：`8a0d84dc1e6de9959c0423f99273aa214c4f38b8`。

Evidence: `SKL-OPS-004` Revision 1 与 SQLite locking 官方文档规定的 SHARED/RESERVED/EXCLUSIVE 边界；focused `read_snapshot_rejects_a_journal_created_after_generation_gate` 通过。workspace `fmt`、`clippy`、`test`（13 + 17 + 158）与 locked build 均通过；修复提交 `8a0d84dc1e6de9959c0423f99273aa214c4f38b8` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3828483212；thread resolved: true。

### PRRT_kwDOT7YN2s6bD4zj - corruption inventory 绑定诊断 root generation

Source: PRRT_kwDOT7YN2s6bD4zj / PRRC_kwDOT7YN2s7kKYVE（https://github.com/bootids/skilload/pull/5#discussion_r3827926340）

Problem: `database_corrupt_with_known_backups` 和 generic enrichment 在 error path 重新解析 XDG roots；若 held `data/skilload` 已被替换，diagnostic 仍可能声称描述旧 generation，却从 replacement root 列出 backup/export recovery evidence。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 以 `ValidatedDataDirectory::open_optional_child` 从 held/revalidated `data/skilload` descriptor 相对打开 `backups`；`database_corrupt_for_generation` 与 generation-bound enrichment 前后重验 directory/main identity，replacement 返回 `database_identity_drift`。新增 `corruption_details_reject_a_replaced_data_directory`。修复提交已推送：`8a0d84dc1e6de9959c0423f99273aa214c4f38b8`。

Evidence: `SKL-OPS-004` observed generation 约束与 `ARCHITECTURE.md` database identity boundary；focused `corruption_details_reject_a_replaced_data_directory` 通过。workspace `fmt`、`clippy`、`test`（13 + 17 + 158）与 locked build 均通过；修复提交 `8a0d84dc1e6de9959c0423f99273aa214c4f38b8` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3828484555；thread resolved: true。

### PRRT_kwDOT7YN2s6bF0oA - 新代际必须先于当前基表校验拒绝

Source: PRRT_kwDOT7YN2s6bF0oA / PRRC_kwDOT7YN2s7kNSt0（https://github.com/bootids/skilload/pull/5#discussion_r3828689780）

Problem: 现有 write-path 在读取到 `SchemaGeneration::Newer` 后仍先按 v1/v2 基表不变量验证；未来 schema 合法替换当前 base table 时，旧 binary 会错误报 `database_corrupt`，而不是 `schema_newer` 的降级拒绝。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 增加 `validate_base_for_generation`，在当前 v1/v2 base validation 前返回 `Newer`；read、write、migration 与 FTS repair race 均复用该分类，新增 `newer_schema_precedes_current_base_validation` 以 version 9 + renamed `library_entries` 覆盖 list/search/get/mutation/import/doctor。修复 commit `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 已推送。

Evidence: `SKL-OPS-003` Revision 1 要求 unknown newer schema 拒绝写入；新增 regression pre-fix 失败、修复后通过。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`（13 + 17 + 160）与 `cargo build --workspace --locked` 全部通过；修复 commit `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3828888972；thread resolved: true。

### PRRT_kwDOT7YN2s6bF0oG - 接受 absent 前必须重验数据根

Source: PRRT_kwDOT7YN2s6bF0oG / PRRC_kwDOT7YN2s7kNSt8（https://github.com/bootids/skilload/pull/5#discussion_r3828689788）

Problem: list/search/get/export/default doctor 在 `resolve_roots` 后以 pathname 判定缺失并立即返回；若 `data/skilload` 在此窗口被替换为空目录，操作会把替换 generation 误报为 empty/not_found，而非拒绝 root identity drift。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `database_exists_with_details` 让缺失 probe 前后 revalidate roots，所有共享该 gate 的 early-return reader/mutation 都拒绝 root replacement；新增 `absent_read_rejects_data_root_replaced_after_resolution` 以 resolver 后目录替换覆盖 public list path。修复 commit `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 已推送。

Evidence: `SKL-OPS-004` Revision 1 与 persistence design 的 root-generation boundary 拒绝采纳 replacement；新增 regression pre-fix 返回 empty list、修复后返回 `XDG_DATA_HOME` identity error。workspace fmt/clippy/test/build 的同一上述命令全部通过；修复 commit `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3828891536；thread resolved: true。

### PRRT_kwDOT7YN2s6bF0oR - FTS detach 后必须回收孤立页面

Source: PRRT_kwDOT7YN2s6bF0oR / PRRC_kwDOT7YN2s7kNSuI（https://github.com/bootids/skilload/pull/5#discussion_r3828689800）

Problem: damaged FTS repair 只删除六条 `sqlite_master` schema row 后立即重建；旧 shadow b-tree pages 未加入 freelist，整库 `PRAGMA integrity_check` 保留 `never used` pages，doctor 却可能报 healthy，重复 repair 还会累积泄漏。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 让 `fts_schema_requires_detach` 覆盖 missing/partial/damaged FTS schema；`repair_fts_locked` 先提交 detach、在同一 lock 的无 transaction connection 上 `VACUUM` 并重验 identity，再以第二笔 transaction rebuild；`rebuild_derived_index` 仅处理已准备 schema。`fts_shadow_corruption_stays_doctor_fixable` 现断言修复后整库 integrity 为 `ok`。修复 commit `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 已推送。

Evidence: `SKL-OPS-004` Revision 1 只允许 base 完整时 repair derived FTS；pre-fix regression 显示 five `never used` pages，修复后 bundled SQLite test 与 local SQLite 3.51.0 probe 都返回 `ok`。workspace fmt/clippy/test/build 的同一上述命令全部通过；修复 commit `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3828894509；thread resolved: true。

### PRRT_kwDOT7YN2s6bKUKq - free-text FTS 需要 NFC projection

Source: PRRT_kwDOT7YN2s6bKUKq / PRRC_kwDOT7YN2s7kT4Hx（https://github.com/bootids/skilload/pull/5#discussion_r3830415857）

Problem: `LibrarySearchQuery` 已把每个词项规范为 NFC，但 `fts_row_values` 只投影原始 name、description、alias、category、note 与 repository。decomposed free-text（例如 `cafe\u{301}`）在 `unicode61 remove_diacritics 0` 下不会匹配 NFC query `café`，与 `SKL-LIB-004` Revision 2 的 NFC query-term 语义不一致。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 增加共享 `fts_free_text_projection`：非 NFC value保留原始 indexed text并以 newline-separated NFC projection补充搜索；helper由 import、metadata mutation、migration、doctor rebuild 与 derived validation复用。它应用到 name、description、alias、category、note 与 repository；当前 source grammar使 name/repository display只能是 ASCII，regression因此覆盖实际可接受的 description/alias/category/note。已同步澄清 `docs/product-specs/library.md`、`docs/design-docs/application-and-persistence.md` 和本 Plan。

Evidence: `search_matches_nfc_forms_of_normalizable_free_text_fields` pre-fix 对 composed `café` 返回空集，修复后对 composed/decomposed query均返回四个 preserved-raw entries；focused tests、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（13 + 17 + 162）与 `cargo build --workspace --locked` 均通过。preliminary remediation commit `9dc0fd058d54cf67f4d9e3edea5e9d7cdabc34f0` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3830978897；thread resolved: true。

### PRRT_kwDOT7YN2s6bKUK8 - read error 返回前必须重验 generation

Source: PRRT_kwDOT7YN2s6bKUK8 / PRRC_kwDOT7YN2s7kT4IN（https://github.com/bootids/skilload/pull/5#discussion_r3830415885）

Problem: `run_read_snapshot` 在 callback 返回 `not_found`、`migration_required` 或 `schema_newer` 等应用错误时通过 `?` 提前退出，未执行已存在的 generation revalidation。snapshot 建立后若 pathname 被替换，命令会把旧 inode 的错误归因给 replacement path，而不是返回 `database_identity_drift`。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `run_read_snapshot` 保留 snapshot callback 的 `Result`，先执行 generation revalidation，再传播原结果；successful read 的 commit 前后 revalidation 保持不变。新增 callback-error 与 same-account replacement race regression，断言 identity drift 优先于原 `not_found`。

Evidence: `failed_read_revalidates_database_generation_before_returning_error` pre-fix 返回 callback 的 `not_found`，修复后返回 `library_database/database_identity_drift`；focused tests、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（13 + 17 + 162）与 `cargo build --workspace --locked` 均通过。preliminary remediation commit `9dc0fd058d54cf67f4d9e3edea5e9d7cdabc34f0` 已推送。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3830980524；thread resolved: true。

2026-08-21 第十轮 final reconciliation：PR #5 有 11 个 top-level comments、47 个 submitted reviews 与 37 个 review threads。37 个 thread ID 全部有本 Log heading，current `isResolved` 均为 true；PRRT_kwDOT7YN2s6bKUKq 的回复为 https://github.com/bootids/skilload/pull/5#discussion_r3830978897，PRRT_kwDOT7YN2s6bKUK8 的回复为 https://github.com/bootids/skilload/pull/5#discussion_r3830980524。十个 nonempty review body 是同一 automated wrapper template，top-level comments均为 `@codex` trigger/notification，没有 pending、blocked 或未记录 source。

### PRRT_kwDOT7YN2s6bDCYF - 将既有数据库读取绑定到已解析的数据目录


Source: PRRT_kwDOT7YN2s6bDCYF / PRRC_kwDOT7YN2s7kJD6S（https://github.com/bootids/skilload/pull/5#discussion_r3827580562）

Problem: `resolve_roots` 与 `database_exists` 后、pathname-based generation gate 之前，同账号进程可替换 `data/skilload` 整个目录。当前 main-file identity 检查会把 replacement 当成新的有效 generation，导致 list/get/search/export 或 doctor 从错误 root 返回数据或恢复证据，违反 read-only 不采纳外部 replacement 的不变量。

Disposition: fixed

Status: resolved

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 增加 `open_bound_data_directory`，在 root anchors 前后绑定 `data/skilload` directory identity；`pre_open_generation_gate` 从 held directory 相对打开并验证 `skilload.db`，所有 read-only list/search/get/export/default doctor 与 dry-run read 都传递该 held directory，transaction 前后继续重验。`0a1cad3897588623b77c69b0fe90279a9d770257` 已推送。

Evidence: `read_only_open_rejects_a_replaced_data_directory`、既有 `export_uses_checked_generation_when_a_read_only_aba_is_restored` 与 `read_only_open_never_creates_sidecars_for_a_replaced_wal_generation` 通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo build --workspace --locked` 通过，提交前 `git diff --check` clean。修复提交：`0a1cad3897588623b77c69b0fe90279a9d770257`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3827811416；thread resolved: true。

### PRRT_kwDOT7YN2s6bDCYM - no-follow generation open 必须 nonblocking


Source: PRRT_kwDOT7YN2s6bDCYM / PRRC_kwDOT7YN2s7kJD6b（https://github.com/bootids/skilload/pull/5#discussion_r3827580571）

Problem: generation gate 只使用 `O_NOFOLLOW`；regular-file precheck 与实际 open 之间若同账号进程将 `skilload.db` 替换为 FIFO，read-only open 会等待 writer，而不是在 metadata type check 前返回 typed identity error，可能无限挂起数据库相关 CLI。

Disposition: fixed

Status: resolved

Resolution: 已在同一 `pre_open_generation_gate` 的 directory-relative `openat` 加入 `O_NONBLOCK` 与已有 `O_NOFOLLOW`，保留 opened-descriptor regular-file validation；`0a1cad3897588623b77c69b0fe90279a9d770257` 已推送。

Evidence: `generation_gate_rejects_fifo_without_waiting` 通过，证明 regular-file precheck 后的 FIFO replacement 立即返回 typed identity error；同轮 existing ABA/WAL regressions与 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo build --workspace --locked` 通过，提交前 `git diff --check` clean。修复提交：`0a1cad3897588623b77c69b0fe90279a9d770257`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/5#discussion_r3827813420；thread resolved: true。

2026-08-21 16:08Z 第十一轮 final-review initial classification：top-level `IC_kwDOT7YN2s8AAAABQDWwtw` 仅为 `@codex` trigger，review body `PRR_kwDOT7YN2s8AAAABKbzRzA` 仅为自动化 wrapper，均无独立问题。以下三个未 resolved inline thread 均在现有 Product Baseline 内，按 `fixed` 处理；尚未回复或关闭。

### PRRT_kwDOT7YN2s6bN3dy - 无法完整枚举 backup inventory 时必须 fail closed

Source: PRRT_kwDOT7YN2s6bN3dy / PRRC_kwDOT7YN2s7kZJeW（https://github.com/bootids/skilload/pull/5#discussion_r3831797654）

Problem: `crates/skilload-core/src/adapters/portable_library.rs` 的 `protected_paths` 对 `read_dir(data/backups)` 使用 `if let Ok`，并以 `entries.flatten()` 丢弃逐项错误。若已发布 migration backup 的目录无法打开或无法完整枚举，export 可能将它视为无受保护项并覆盖 recovery asset。

Disposition: fixed

Status: open

Resolution: 已在 `crates/skilload-core/src/adapters/portable_library.rs` 将 `protected_paths` 改为 fallible inventory：仅 absent `data/backups` 返回空集合，目录打开或 iterator entry error 返回 `library_export_protected_inventory_unavailable` validation error；初始与最终 export protection、publication guard 共享该结果。新增 `output_rejects_an_unreadable_migration_backup_inventory_before_staging`，证明拒绝发生在 staging 前且 recovery file 未变。预备 remediation commit `f032f9c1f087fa72b7ca55666e8b5d92e3149f27` 已推送。

Evidence: focused regression 通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（13+17+164）和 `cargo build --workspace --locked` 均通过。修复 SHA：`f032f9c1f087fa72b7ca55666e8b5d92e3149f27`。

GitHub outcome: 未回复；thread unresolved。

### PRRT_kwDOT7YN2s6bN3d2 - writable SQLite handle 必须在最终 generation revalidation 后重验

Source: PRRT_kwDOT7YN2s6bN3d2 / PRRC_kwDOT7YN2s7kZJeg（https://github.com/bootids/skilload/pull/5#discussion_r3831797664）

Problem: writable `open_existing_database` 在 `SQLITE_FCNTL_HAS_MOVED` 检查后、directory/main-entry identity revalidation 前可经历同账号 ABA pathname swap；攻击者恢复原 pathname 后，现有路径检查通过，但 connection 仍可能指向 replacement database，随后 migration、repair 或 mutation 会写错 generation。

Disposition: fixed

Status: open

Resolution: 已在 `crates/skilload-core/src/adapters/sqlite_library.rs` 保留 writable open 后的早期 `SQLITE_FCNTL_HAS_MOVED`，并在 held data-directory entry/path identity 都重验后再次执行，以拒绝 restoration ABA。`open_existing_database` 是 import、metadata mutation、v1 migration 与 FTS repair 的共享 writable gate；新增受控 `writable_open_rejects_an_aba_generation_restored_after_initial_handle_check`，在初次 handle check 后恢复原 pathname 时要求 `database_identity_drift`。预备 remediation commit `f032f9c1f087fa72b7ca55666e8b5d92e3149f27` 已推送。

Evidence: focused regression 通过；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（13+17+164）和 `cargo build --workspace --locked` 均通过。修复 SHA：`f032f9c1f087fa72b7ca55666e8b5d92e3149f27`。

GitHub outcome: 未回复；thread unresolved。

### PRRT_kwDOT7YN2s6bN3d7 - FTS detach 后必须在 rebuild 前 VACUUM

Source: PRRT_kwDOT7YN2s6bN3d7 / PRRC_kwDOT7YN2s7kZJem（https://github.com/bootids/skilload/pull/5#discussion_r3831797670）

Problem: `docs/references/sqlite-fts5-library-search.md` 描述 physical FTS shadow corruption 时列出 schema-row detach 后立即 recreate，遗漏实现实际要求的 detach commit、non-transactional `VACUUM` 和随后独立 transaction rebuild；该遗漏会让维护者重引 orphan pages 与 full-database integrity failure。

Disposition: fixed

Status: open

Resolution: 已更新 `docs/references/sqlite-fts5-library-search.md` 的 verified recovery sequence 与 cautions：physical shadow corruption 必须先 commit writable-schema detach，在无 transaction 的 held writable connection 执行 `VACUUM`、重验 generation，随后以 fresh transaction recreate/fill/validate FTS；这与 `repair_fts_locked` 和 whole-database integrity regression 一致。预备 remediation commit `f032f9c1f087fa72b7ca55666e8b5d92e3149f27` 已推送。

Evidence: `fts_shadow_corruption_stays_doctor_fixable` 的既有 whole-database integrity contract仍适用；`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（13+17+164）和 `cargo build --workspace --locked` 均通过。修复 SHA：`f032f9c1f087fa72b7ca55666e8b5d92e3149f27`。

GitHub outcome: 未回复；thread unresolved。

## Context and Orientation


仓库是两个 crate的 Rust workspace。`crates/skilload-core`保存 domain/application/ports/adapters；`crates/skilload-cli`保存 clap schema、dispatch与 JSON/human renderers。`skilload-core/src/domain/library.rs`已有 portable document、metadata mutations与 `LibraryEntry`；`domain/unicode_15_1.rs`暴露 pinned `normalize_tag`、`full_case_fold`和 `is_white_space`。`ports/library.rs`目前只有 transfer/import/export/mutate contracts，`application/library.rs`只有相应 operations。`adapters/sqlite_library.rs`是唯一 durable Library repository，负责 XDG resolution、database identity、schema validation、import/mutation transactions、locks与sync。`skilload-cli/src/args.rs`当前未注册 list/search/get/doctor，`main.rs`只 dispatch现有 config和 Library transfer/metadata leaves。

持久 Library位于有效 `XDG_DATA_HOME/skilload/skilload.db`；durable database process lock位于有效 `XDG_STATE_HOME/skilload/locks/database.lock`。Effective config/data/state/cache application roots必须两两不重叠。Absent read只能建立内存empty view，不能创建任何 root。SQLite main file必须是no-follow regular file；read-only open从 held data-directory descriptor相对读取 generation header并作第一次 companion census，再以同一 descriptor 的 `/dev/fd/<fd>` 路径打开 SQLite。connection在同一 transaction用 `PRAGMA schema_version` 建立 SHARED snapshot并作第二次 census，只有两次均无 `-journal`、`-wal`、`-shm` 后才读取 Library schema/data。无论 snapshot callback返回 success 还是 application error，都必须先重验 directory/main-file generation；directory/main-file replacement 返回 `database_identity_drift`，不让 SQLite 修改 replacement generation或把旧 inode结果归属到 replacement。
若任一 census 观察到同名 `-journal`、`-wal` 或 `-shm`，adapter以 `database_corrupt`（含可验证 backups 与 `database-corruption-v1`）返回并保留每个文件；snapshot 后启动的 writer不能将未提交 main image混入当前 read result。backup inventory从同一 held `data/skilload` generation相对打开 `backups`，并拒绝每个有 SQLite companion 的 pair。
所有 absent return 在接受 empty/not_found 前后也重验已解析 XDG roots；unknown newer generation 在解释已知 base schema 前返回 typed `schema_newer`，但 portable export 仅在其独立 entries/tags projection仍可证明时保留恢复出口。FTS schema row 直删后的 page 回收必须先 commit、无 transaction 执行 `VACUUM`，再以新 transaction重建，以免 unreachable pages躲过 derived-only validation。

Schema v1固定 `schema_info(version=1)`、`state_revision`、以 `canonical_source`为 primary key的 `library_entries`，以及 foreign-key cascade的 `library_tags(canonical_source, comparison_key, display)`。`state_revision`只记录 product-state mutation；添加/rebuild派生 FTS或升级schema不代表用户metadata变化。Portable import最多10,000 entries且完整 deterministic export最多67,108,864 bytes，因此 `total`在当前有效状态中可安全转换为 `u64`，但 API仍按 `DecimalU64`编码。

“FTS row”指只为搜索保存的一份可重建文字投影。Schema v2创建：

    CREATE VIRTUAL TABLE library_fts USING fts5(
        canonical_source UNINDEXED,
        name,
        description,
        alias,
        tags_display,
        tags_comparison,
        category,
        note,
        repository,
        tokenize = 'unicode61 remove_diacritics 0'
    );

Null optional metadata投影为空字符串。任何非 NFC 的 free-text FTS value保留原文并以 `\n`追加 NFC representation；base metadata仍保持原始 UTF-8。Tags按 comparison key稳定排序，display和comparison分别用 `\n`连接。每个 canonical source必须在 FTS table恰有一行；doctor比较这一完整集合与 base rows，不能只比较 row count。

“纯文本词项 AND”指：保留原始 query供 API回显；按 pinned `is_white_space`切分；对每个词项产生 NFC raw与 `full_case_fold(raw).nfc()`，去除同一词项的重复alternative；adapter将 literal `a"b`编码为 FTS string `"a""b"`；同词项 alternatives用括号内 OR组成一个 group，不同词项的 group之间以显式 `AND` 连接（FTS5 的 implicit AND只是裸 quoted phrases间的特例；`("Review" OR "review") "code"` 是 syntax error，`("Review" OR "review") AND "code"` 才有效）。没有词项时不调用 SQLite。Search结果不按 relevance，而在 count/page transaction中按 `canonical_source COLLATE BINARY`排序。

“Standalone migration backup”指通过 SQLite online backup API从已打开并绑定identity的 source connection复制到独立 destination database，而不是复制 live main/WAL文件。Sibling manifest至少包含 format version、source/target schema、UTC epoch nanoseconds、database bytes、`sha256:` digest、source device/inode和 `complete: true`。只有 database和manifest都写完、sync、验证并在 held backup-directory descriptor中发布后才能开始 live migration。

## Plan of Work


### Milestone 1：建立读取 domain、ports 与 application contract


在 `crates/skilload-core/src/domain/library.rs` 增加 validated page与search values。`LibraryPage`保存 `limit: u16`、`offset: u64`并保证limit 1..=1,000；`LibrarySearchQuery`保存原始query和private normalized terms，构造时使用 pinned Unicode helper并拒绝empty；`LibraryEntriesPage`与`LibrarySearchPage`保存 presentation-neutral entries/page/total。不要让 domain生成SQL或导入rusqlite。复用现有 `LibraryEntry`，当前 trust projection仍为 `Missing`。

在新文件 `crates/skilload-core/src/domain/doctor.rs` 定义 `DoctorFinding`、`DoctorAction`、`DoctorData`、`DoctorOutcome`与 `DoctorOperation`。Finding包含severity/code/message、optional source/target、fixable/fixed；action使用architecture已有 create/repair/migrate等稳定词汇。`database_writable`明确表示当前二进制是否允许 durable database mutation，而不是OS permission probe。

扩展 `ports/library.rs` 的 `LibraryRepository`：

    fn list(&self, page: &LibraryPage) -> Result<LibraryEntriesPage, AppError>;
    fn search(
        &self,
        query: &LibrarySearchQuery,
        page: &LibraryPage,
    ) -> Result<LibrarySearchPage, AppError>;
    fn get(&self, selector: &str) -> Result<LibraryEntry, AppError>;

在新文件 `ports/doctor.rs` 定义 focused `DatabaseMaintenance`：

    fn inspect(&self) -> Result<DoctorData, AppError>;
    fn fix(&self) -> Result<DoctorOperation, AppError>;

在 `application/library.rs` 增加 `library_list`、`library_search`、`library_get`，在新 `application/doctor.rs` 增加 `doctor(fix: bool)`。`Application`只调用ports，不检查files/SQL。扩展 `Application::new`接收 `Arc<dyn DatabaseMaintenance>`；CLI composition向 Library与doctor port传同一个 `Arc<SqliteLibraryRepository>`，避免第二个database owner。更新 `lib.rs`和module exports；对 `Application::new`执行 symbol references并迁移每个caller，不保留旧constructor shim。

该里程碑完成时，domain tests证明limit/offset边界、Unicode whitespace、NFC/case-fold alternatives、operator literals、quote-containing terms与empty query；fake repositories证明application每个leaf只调用一个port method且不产生network/filesystem行为。

### Milestone 2：实现 schema v2、FTS maintenance 与一致 snapshot reads


重构 `adapters/sqlite_library.rs` 的schema validation为base与derived两层。Base validation读取schema version、验证v1 tables/foreign keys/domain rows并区分v1、v2、newer；v2 derived validation确认virtual table和columns/tokenizer符合固定SQL、FTS rows与base deterministic projection一一相等。`PRAGMA integrity_check`、`foreign_key_check`和domain deserialization继续是base corruption gate，不能把missing/corrupt FTS误报成可丢弃的base rows。

把first import初始化切换为schema v2。新增一个接受transaction和完整 `PortableLibraryEntry`/stored entry的共享helper，按tag comparison order生成一行FTS projection；non-NFC free-text value保留原文并以 newline-separated NFC representation补充搜索，而不改变base row。`apply_additions`在同一transaction插入base/tags/FTS；`apply_metadata_change`在同一transaction更新base/tags后替换目标FTS row。Changed mutation仍只推进一次 `state_revision`；unchanged不触发FTS DELETE/INSERT、transaction commit或sync。Existing v1 import/mutation在取得现有identity并验证base后返回 `MigrationRequired { found:1, supported:2 }`，不自动upgrade。

实现list/search/get read transactions。所有对已存在 database的 read-only opens先经 Design Inputs固化的 pre-open generation gate：WAL-mode header或 `-wal`/`-shm` sibling在 SQLite 前即返回 `database_corrupt`，不创建任何 sidecar；通过 gate 的 held descriptor 必须存活至 SQLite 以 `/dev/fd/<fd>` 打开同一 inode，原 pathname 若在窗口中被替换则 identity revalidation返回 `database_identity_drift`且 replacement 不会产生 sidecar。Absent list/search返回empty page且不创建roots；absent get返回`not_found`。V1 list/get从base rows工作；v1 search返回migration_required。V2 list/search先在同一transaction计算total，再在`offset >= total`时不做SQLite signed offset conversion并返回empty；否则用CTE选择canonical page并一次LEFT JOIN tags，按source/tag comparison order流式组装entries，避免每entry query。Search CTE只接收adapter从domain terms编码的bound MATCH string；用户raw query不得拼入SQL/FTS grammar。完成 base 与 FTS content validation 后，count、paged MATCH preparation或 row iteration的 `SQLITE_CORRUPT`/`SQLITE_NOTADB` 返回`library_fts_invalid`，让用户走doctor repair而不误报base corruption。Get用exact selector和同样single-query tag assembly，missing返回`not_found`。

该里程碑的repository tests必须证明每个字段可搜索、raw/folded tag等价、description/alias/category/note 的 composed/decomposed NFC query 等价且 read/export保留原始 UTF-8、operators保持literal、same-name sources共存、source-order先于pagination、adjacent pages不重叠、offset==total/`u64::MAX`为空、limit边界在SQL前失败、read transaction在concurrent writer下提供同一snapshot。验证v1 list/get/export继续工作而search/writes要求migration；v2 import/mutations每次保持FTS一致；corrupt/missing/extra FTS rows不会反向改base。

### Milestone 3：实现 backup、v1→v2 migration 与 doctor repair


在workspace dependency中给locked `rusqlite 0.40.2`增加`backup` feature，并加入 `sha2 = { version = "=0.11.0", default-features = false }`。不要升级其他locked crates。为 `SqliteLibraryRepository` 实现 `DatabaseMaintenance`，复用现有 XDG resolver、no-follow database identity、data-directory descriptor、global lock与durability helpers。

Default `inspect`在absent state返回healthy/empty `DoctorData`且不创建root。对existing database，先执行 pre-open generation gate：main file header的 journal-mode bytes非 (1,1) 或存在`-wal`/`-shm` sibling时，不打开 SQLite，直接返回typed `database_corrupt`错误，其 `DatabaseCorruptDetails` 含database `PathValue`、已验证backup manifests、仍可读portable exports与 `recovery_procedure: "database-corruption-v1"`，live filesystem零改动。通过 gate 的 read-only source connection必须从仍持有的 no-follow descriptor `/dev/fd/<fd>` 打开并在回传诊断前重验 pathname identity；这使 replacement 返回`database_identity_drift`而不修改 replacement generation。该连接使用 `rusqlite::backup::Backup`复制到`:memory:` destination，在副本上执行完整v2 FTS content comparison与 `INSERT INTO library_fts(library_fts) VALUES('integrity-check')`。V1产生fixable `library_database_migration_required` finding并令 `database_writable=false`；healthy v2无finding且true；FTS-only drift产生fixable `library_fts_invalid`且false；newer产生unfixable `library_schema_newer` finding且false。Base corruption不进入 `DoctorData`：schema/base validation（schema version decode、integrity/foreign keys、domain rows）任何失败都按既有error mapping返回typed `database_corrupt`，满足 `SKL-OPS-004` 与 `docs/product-specs/database-recovery.md` 第1步对 doctor 返回 `database_corrupt` details的要求。可诊断generation的default outcome始终`observed`、`fix_requested=false`、每个finding `fixed=false`、actions empty。

`fix`先运行同一diagnosis，因此base corruption与WAL/sidecar generation同样在diagnosis阶段即返回`database_corrupt`，绝不进入action阶段或返回`unchanged`。Absent/healthy返回`unchanged`；newer保留finding并返回`unchanged`。V1 migration取得durable lock、重新绑定database/directory/schema/state revision baseline，然后在 `data/backups/` 的held no-follow directory中创建restrictive random staging database与manifest。Online backup完成后关闭destination、sync file、流式计算size/SHA-256、重开read-only验证schema/base/integrity/foreign keys、写completed manifest、sync并以no-clobber relative rename发布pair、sync parent。任何不匹配或collision保留foreign entry并返回typed error；只清理仍与held descriptors匹配的owned staging。

Backup durable后，在live connection的一个transaction创建 `library_fts`、从base rows填充、运行content comparison与FTS integrity check、最后把`schema_info.version`更新为2并commit。Migration前后 `state_revision`必须相等。Commit后执行现有 descriptor-bound database/file/parent sync和最终identity revalidation；sync后错误不得声称v1仍在。当前实现保留每个完整 validated backup pair，绝不按一个已验证后可能 stale 的 pathname 自动删除；跨 macOS/Linux 的可移植 unlink 没有条件化 inode 删除，未来任何 cleanup都必须先获得 ownership-bound deletion。无效、unpaired、symlinked或digest-drifted条目既不作为 validated backup，也不被本交付删除。

FTS-only fix在lock内重新诊断base，开启一个transaction，drop/recreate `library_fts`并从base rows重建，再运行content/integrity checks后commit/sync；不创建migration backup，不推进state revision。Failure保持base rows和先前committed FTS state或返回明确post-commit uncertainty，不得删除database。返回action的target是scope `database`与live `PathValue`；migration action before/after为`schema_1`/`schema_2`，repair action before/after为`fts_invalid`/`fts_valid`。

为 backup/migration/rebuild加入test-only hooks，覆盖backup open/copy/sync/hash/manifest/publish后、live transaction before/after commit、database/parent sync和final verify。每个failure assertion区分“v1+complete backup”“durable v2但command error”与“unchanged base+invalid FTS”；不得用pathname observation删除无法证明ownership的entry。Unknown newer fixture允许doctor诊断；只有当前base tables经只读验证仍可安全投影时才允许portable export，list/get/search返回`schema_newer`，fix和所有writes一律拒绝。

### Milestone 4：接通 CLI、投影、端到端与文档证据


在 `crates/skilload-cli/src/args.rs` 只注册：

    skilload library list [--limit <COUNT>] [--offset <COUNT>]
    skilload library search <QUERY> [--limit <COUNT>] [--offset <COUNT>]
    skilload library get <CANONICAL-SOURCE>
    skilload doctor [--fix]

Clap value parsers必须在dispatch前拒绝limit 0/1001/negative、offset negative/overflow；只有list/search出现pagination help。`json_operation`识别三个 dotted Library operations和`doctor`；不添加help subcommand、alias、placeholder或global pagination。

扩展 `main.rs` Projection/dispatch/render，确保每个leaf调用一个application method。`json.rs`增加closed API-v2 `LibraryEntriesData`、`LibrarySearchData`与`DoctorData` projections，复用已有 serialized `LibraryEntry` shape、DecimalU64、PathValue和error details。`human.rs`为list/search/get/doctor输出相同entry/page/finding/action内容；所有query、source、metadata、finding message和path通过terminal-safe quoted encoder，不能输出raw controls。JSON stdout恰好一个value，diagnostics留在stderr。

扩展 `crates/skilload-cli/tests/cli_contract.rs`：真实binary在isolated XDG roots中导入至少两个entries，修改alias/category/tag/note后逐字段search；验证literal operators、empty query、pagination/defaults/huge offset、get success/not_found、human/JSON等价、no network/no extra roots。Core adapter tests承担schema/fault/doctor details。建立10,000-entry fixture一次，随后在release binary记录exact get、list first page与representative full-text search各自elapsed并要求每项低于10秒；永久test只断言complete counts/order/semantics。

实际binary smoke执行import → list pages → metadata mutation → search → get → export，并在独立v1 fixture上执行doctor observe/fix/search。Smoke必须调用仓库明确路径 `./target/debug/skilload` 或 `./target/release/skilload`，不能依赖PATH中的同名程序。完成后更新`ARCHITECTURE.md`当前实现状态与external boundaries；更新product/design status只声明本Product Baseline完成，不把doctor cross-domain或CLI完整surface误报为完成；补齐本Plan living sections和evidence。

## Concrete Steps


所有命令均从 repository root运行。进入execution后先确认toolchain与状态：

    mise install
    git status --short --branch
    gh pr view <pull_request> --json url,isDraft,headRefName,baseRefName,headRefOid

预期当前branch为`codex/p4-library-indexed-reads`、worktree clean、PR Draft且base为`main`。然后按milestones实现；每个 exported symbol变更前使用可用language server查询references，clean cutover所有callers。

Focused开发循环使用：

    mise exec -- cargo test -p skilload-core --locked library_search_query
    mise exec -- cargo test -p skilload-core --locked sqlite_library
    mise exec -- cargo test -p skilload-core --locked doctor
    mise exec -- cargo test -p skilload-cli --test cli_contract --locked library_reads
    mise exec -- cargo test -p skilload-cli --test cli_contract --locked doctor

Test filter名称应按实际Rust test命名调整，但必须分别覆盖domain、repository/migration、CLI；不能把一次full suite当作缺失focused evidence的替代。

构建debug binary后，创建绝对temporary roots并写两个合法portable entries。可沿用`crates/skilload-cli/tests/cli_contract.rs::portable_document`的exact schema；source canonical必须与structured source字段重新render一致，name必须与path basename一致，commit/integrity长度有效，entry/byte counts为positive decimal。运行：

    mise exec -- cargo build --workspace --all-features --locked
    SMOKE_ROOT="$(mktemp -d)"
    export HOME="$SMOKE_ROOT/home"
    export XDG_CONFIG_HOME="$SMOKE_ROOT/config"
    export XDG_DATA_HOME="$SMOKE_ROOT/data"
    export XDG_STATE_HOME="$SMOKE_ROOT/state"
    export XDG_CACHE_HOME="$SMOKE_ROOT/cache"
    ./target/debug/skilload library import --input "$SMOKE_ROOT/library.json" --json
    ./target/debug/skilload library list --limit 1 --offset 0 --json
    ./target/debug/skilload library list --limit 1 --offset 1 --json
    ./target/debug/skilload library note set 'github:owner/repository#skills/review@refs/heads/main' 'code quality review'
    ./target/debug/skilload library search 'code review' --json
    ./target/debug/skilload library search 'OR NOT * name:review' --json
    ./target/debug/skilload library get 'github:owner/repository#skills/review@refs/heads/main' --json
    ./target/debug/skilload library export --output "$SMOKE_ROOT/export.json" --json
    ./target/debug/skilload doctor --json

预期import changed；两页各返回不同canonical source且total相同；`code review`命中note中不相邻词项；operator-literal query只按普通tokens匹配或返回empty而不产生FTS syntax error；get返回exact entry；export仍是portable data file；doctor healthy且不创建额外domain roots。记录smoke前后tracked temp-tree inventory或file metadata，证明read commands不改database bytes/mtime。清理仅删除`SMOKE_ROOT`，绝不对真实HOME运行。

Migration smoke使用core test helper生成真实schema v1 fixture及记录的state revision。对该isolated root先运行list/get/export并观察成功，search与metadata write观察`migration_required`；默认doctor观察fixable finding且database/dirs bytes、mtime不变；再运行：

    ./target/debug/skilload doctor --fix --json
    ./target/debug/skilload doctor --json
    ./target/debug/skilload library search review --json

预期fix返回changed和一个migrate action；`data/skilload/backups/`出现一个standalone database与completed sibling manifest，digest/size/source identity验证；live schema为2、base export和state revision与迁移前相同；第二次doctor healthy observed；search成功。FTS drift smoke只在isolated fixture中通过test helper破坏derived table，随后doctor observe/fix；不得用外部SQLite修改真实state。

最终validation只在所有实现和docs同步后运行一次：

    mise exec -- cargo fmt --all --check
    mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings
    mise exec -- cargo test --workspace --all-features --locked
    mise exec -- cargo build --workspace --all-features --locked
    git diff --check

预期全部exit 0，`git diff --check`无输出。另以release binary执行10,000-entry measurement并在`Artifacts and Notes`记录机器、fixture、exact commands、每项elapsed与deterministic result；不把M4本机数值伪装成所有CI机器承诺。

在active状态提交并push全部code/tests/docs/Plan updates后，确认remote HEAD，再执行：

    gh pr ready <pull_request>
    gh pr view <pull_request> --json isDraft,headRefOid

必须观察`isDraft: false`且`headRefOid`等于刚push的implementation SHA；之后才移动Plan到`docs/exec-plans/review/`、改`status: review`、记录evidence并提交push。不要在本计划创建流程中执行这些implementation/review步骤。

## Validation and Acceptance


Domain acceptance要求query normalization不依赖运行时Unicode版本：Unicode 15.1 whitespace exact boundaries、NFC composed/decomposed、C/F full fold、Turkish locale independence、quotes/operators均有golden expressions或logical terms。Empty/all-whitespace在repository invocation计数仍为零时返回`validation_failed/library_search_query_empty`。

Repository acceptance使用真实bundled SQLite。每个name/description/alias/tag display/tag key/category/note/repository字段都有独立match fixture；non-NFC description/alias/category/note 的 composed/decomposed query必须命中同一 entries且 list/get/export保留原始 UTF-8；`code review`可跨字段且不相邻；canonical source order在page之前；total/returned/offset/limit准确；offset `u64::MAX`不向SQLite做overflow conversion。Reads不持有mutation lock、不写state revision、不修改FTS或timestamps。V2 import/mutations在同一transaction保持base/index一致，failure rollback两者；unchanged不做derived rewrite。

Migration acceptance从P2/P3产生的真实v1 schema开始。Default doctor完全filesystem-inert，且该不变性覆盖 corrupt/WAL fixtures：base corruption与 WAL-mode/sidecar generation都返回typed `database_corrupt` error（details完整）而不创建`-shm`/`-wal`、不改动任何 byte/timestamp；fix只在durable validated backup pair发布后开始live transaction；backup digest、manifest、SQLite integrity/foreign keys和source identity均可重验。每个failpoint留下old readable v1加完整/无backup，或完整v2且命令明确报告post-commit uncertainty；从不报告partial success。Schema v2的base rows、portable export和state revision与v1相同。Newer schema无write。FTS-only repair保持base database semantic records byte-equivalent或query-equivalent、state revision不变，并使content comparison与special integrity-check通过。

CLI acceptance逐leaf验证help/parser/dispatch/API-v2/human。Pagination只出现在list/search；`library get --limit`、future leaves和aliases均usage error。JSON success恰好一个value；list/search/get outcome为observed，doctor outcomes遵循catalog；not_found、validation、migration、schema-newer、database-corrupt与busy保持typed details/exit。Human输出表达同一query/page/entry/finding/action且所有untrusted字段terminal-safe。

End-to-end acceptance是actual binary的import → list/page → metadata mutations → field searches → get → export，以及v1 doctor observe → fix → search。所有读取在network denied环境成功；absent reads/doctor保留empty filesystem。10,000-entry release evidence中exact get、first list page和representative full-text search各低于10秒、total为10,000、结果顺序稳定。已有import/export/mutation regressions全部通过，证明schema v2没有破坏PLAN-0003/0004完成行为。

完成判定还要求product/spec index、`ARCHITECTURE.md`、两个design docs、新reference和本Plan与runtime一致；没有generated artifact手工修改、placeholder、compat shim、hidden migration或第二套query normalization。

## Idempotence and Recovery


List/search/get/default doctor纯读且可安全重复；对同一snapshot返回相同order/page/data。Healthy `doctor --fix`可重复并返回unchanged，不创建backup。FTS repair重复后第二次unchanged。成功v1→v2后再次fix不生成第二份migration backup。Import/mutations仍使用现有idempotency和lock语义。

Backup/migration必须按身份保守恢复。Backup publish前失败只删除仍绑定held staging descriptors的本调用文件；已发布complete backup是安全artifact，即使live migration随后失败也保留。Foreign collision、symlink、unpaired manifest、digest drift或无法证明ownership的backup entry永不删除。Live transaction failure rollback到v1；commit后sync/final-identity failure返回error而不声称v1仍在，下一次doctor重新观察actual schema。FTS repair transaction失败保留旧derived state；post-commit sync failure同样由下一次doctor重新诊断。任何base corruption都停止writes并保留evidence，转到`docs/product-specs/database-recovery.md`，不得自动reset。

如果implementation发现必须改变query semantics、base schema ownership、portable format、Trust或引入network，停止扩大本Plan，记录discovery并按`docs/PLANS.md`重评scope与behavior revision；不得用temporary table fallback、hidden schema variant或兼容alias绕开migration。

Ready/review transaction严格遵守`docs/PLANS.md`。若`gh pr ready`失败，Plan保持active。若PR已ready但review-state commit/push失败，立即`gh pr ready <URL> --undo`并确认Draft，再保持/恢复active。Review发现material scope缺失时先undo ready并验证Draft，再把Plan移回active、记录原因、commit/push；发布active transition失败则恢复review并将PR恢复ready。

只有明确merge prompt才进入completed。Completion commit后若required check、fresh conversation gate、queue或merge在GitHub报告MERGED前失败，查询ambiguous result；确认未merge后把Plan恢复review并push，不能让unmerged PR保持completed。Merge成功后才更新main并删除local branch。

## Artifacts and Notes


规划期证据：

    main...origin/main，worktree clean
    GitHub认证账户：bootids
    已完成计划：PLAN-0001、PLAN-0002、PLAN-0003、PLAN-0004
    open/draft delivery：无
    selected plan：PLAN-0005
    branch：codex/p4-library-indexed-reads
    product choice：纯文本词项 AND

当前代码证据：`crates/skilload-core/src/adapters/sqlite_library.rs` 的 `SCHEMA_VERSION`为1，initializer只创建base tables；`LibraryRepository`没有list/search/get；CLI args没有这些leaves；bundled compile-options test包含`ENABLE_FTS5`。规划期SQLite实验得到：

    'code review' -> '"code" "review"' -> 同时命中 code review 与 code quality review
    'OR' -> '"OR"' -> OR按literal命中
    '*' -> '"*"' -> empty result，无grammar扩张
    empty -> empty MATCH expression -> FTS syntax error
    'Review code' -> '("Review" OR "review") "code"' -> fts5: syntax error（括号 group 后无 implicit AND）
    'Review code' -> '("Review" OR "review") AND "code"' -> 正常命中
    WAL-mode fixture + mode=ro -> 创建并保留 skilload.db-shm（header 为 WAL 且无 sidecar 时连 -wal 一起创建）
    WAL-mode fixture + immutable=1 -> 无 sidecar 但忽略 WAL 内容（no such table）

因此product在SQLite前显式拒绝empty query，而不是泄露engine error。External facts与来源保存在`docs/references/sqlite-fts5-library-search.md`。

执行时在此追加：planning commits、Draft PR URL/head、migration backup manifest示例、failpoint summary、focused/full validation、actual smoke输出、10,000-entry release timings、implementation SHA、ready verification与review-state commit。只保留证明行为所需的短摘录，不粘贴完整test logs。

执行证据（2026-08-20 13:36Z，机器 Apple M4 Pro / darwin 25.6.0 / arm64）：

Focused/full validation（`mise exec -- cargo … --locked`）：

    cargo test -p skilload-core                      -> 135 passed, 0 failed（含 13 个新增 sqlite_library tests）
    cargo test -p skilload-cli --bin skilload        -> 11 passed（含 json_operations_cover_indexed_reads_and_doctor）
    cargo test -p skilload-cli --test cli_contract   -> 17 passed（含 library_reads / read_commands_never_mutate /
                                                         doctor_observes_and_fixes_a_v1_database_end_to_end /
                                                         absent_reads_and_doctor_stay_offline）
    cargo build --workspace --all-features --locked  -> exit 0

Debug binary smoke（isolated XDG roots，`./target/debug/skilload`）：

    library import --input library.json --json          -> changed
    library list --limit 1 --offset 0 --json            -> skills/other（total 2）
    library list --limit 1 --offset 1 --json            -> skills/review（total 2）
    library note set 'github:…#skills/review@…' 'code quality review' --json -> changed
    library search 'code review' --json                 -> total 1，命中 note 中不相邻词项
    library search 'OR NOT * name:review' --json        -> total 0，无 FTS grammar error
    library get 'github:…#skills/review@…' --json       -> entry（name "review"）
    library export --output export.json --json          -> format_version 1
    doctor --json                                       -> findings 0，database_writable true

v1 migration smoke（真实 binary + rusqlite 生成的 v1 fixture，见 cli_contract
`doctor_observes_and_fixes_a_v1_database_end_to_end`）：list/get/export 成功；search 返回
`migration_required`（details found_version 1 / supported_version 2）；默认 doctor 报
`library_database_migration_required`（fixable，database_writable false）且 database
bytes/mtime 不变；`doctor --fix` 返回 changed + migrate action（before schema_1 / after
schema_2，target scope database）；`data/backups/` 出现 1 个 standalone db + completed
manifest（source_schema 1 / target_schema 2 / database_bytes 与实际相等）；第二次 doctor
healthy（writable true）；search 成功；重复 fix unchanged 且 backups 数量仍为 1。

Migration backup manifest 示例（core test `doctor_fix_migrates_v1_after_a_validated_backup`
校验全部字段并重算 SHA-256）：

    {"format_version":1,"source_schema":1,"target_schema":2,
     "created_at_epoch_ns":…,"database_bytes":…,"sha256":"sha256:<64 hex>",
     "source_device":…,"source_inode":…,"complete":true}

Failpoint summary（`migration_failpoints_leave_a_coherent_state`）：backup copy 失败 ->
live 保持 v1、无 complete pair、state revision 不变；pre-commit migration 失败 -> v1 +
1 个 complete backup 保留；post-commit sync 前失败 -> v2 durable、命令报
internal_invariant、下一次 doctor healthy（不声称 v1 仍在）；FTS rebuild post-commit
失败 -> base 与 state revision 不变、索引已提交、下一次 doctor healthy。

10,000-entry release measurement（fixture 6,296,601 bytes，一次 import 后逐项计时）：

    exact get  github:owner/repo-042#skills/skill-00042@refs/heads/main  real 0.06s
    first list page  --limit 100 --offset 0    total 10000 returned 100  real 0.06s
    full-text search 'code review'             total 910  returned 100   real 0.12s
    deep-offset search 'Review' --offset 3200  total 10000 returned 100  real 0.12s
    last list page    --limit 1000 --offset 9900  returned 100           real 0.06s

全部远低于 Product Baseline 的 10 秒预算；语义（counts/order/分页）由永久 tests 断言，
不依赖 wall clock。

## Interfaces and Dependencies


继续使用workspace Rust 1.97.1、edition 2024、`clap 4.6.6`、`serde`/`serde_json`、`tempfile 3.27.0`、`rustix 1.1.4`、`unicode-normalization 0.1.23`和bundled `rusqlite 0.40.2`。只允许两项dependency变化：给rusqlite增加`backup` feature；加入exact `sha2 0.11.0`且default-features false。更新`Cargo.lock`并以locked full suite验证。禁止system SQLite、dynamic FTS extension、ORM、HTTP client、async runtime、search service、time crate或general migration crate。

最终public domain/API至少存在：

    pub struct LibraryPage { /* validated limit: u16, offset: u64 */ }
    pub struct LibrarySearchQuery { /* original + private normalized terms */ }
    pub struct LibraryEntriesPage { /* entries, page, total */ }
    pub struct LibrarySearchPage { /* original query, entries, page, total */ }

    impl Application {
        pub fn library_list(&self, page: LibraryPage) -> Result<LibraryEntriesPage, AppError>;
        pub fn library_search(
            &self,
            query: String,
            page: LibraryPage,
        ) -> Result<LibrarySearchPage, AppError>;
        pub fn library_get(&self, selector: String) -> Result<LibraryEntry, AppError>;
        pub fn doctor(&self, fix: bool) -> Result<DoctorOperation, AppError>;
    }

`LibrarySearchQuery`不公开可由caller伪造的raw FTS expression；adapter通过只读term iterator生成quoted expression。Page structs不得为API serialization预先把u64转String；JSON renderer统一负责DecimalU64。

最终ports至少存在本文Milestone 1给出的`LibraryRepository::{list,search,get}`与`DatabaseMaintenance::{inspect,fix}`。`SqliteLibraryRepository`实现二者并保持唯一XDG/database composition。Portable transfer store不接触FTS；configuration store不接触doctor database。

SQLite私有helper至少形成一条共享projection路径：base row与sorted tags → `LibraryEntry`/portable evidence → FTS columns。Import、metadata mutation、migration、doctor compare/rebuild不得各自重写tag concatenation、null handling或repository/name选择。Query SQL必须使用bound parameters和fixed statement text；唯一动态内容是adapter从validated logical terms安全编码的MATCH value。

Backup manifest是private versioned serde record，不进入API-v2或portable export。Digest用`sha2::Sha256`分块读取同一个 no-follow opened backup descriptor；大小、regular-file type和identity也来自该 descriptor，并在 held backup-directory descriptor的相对 directory entry上重验。Manifest枚举、validation与live migration只使用validated directory handles、`openat(..., O_NOFOLLOW)`和single-component relative names，复用现有native identity规则；本交付不执行无法以 inode 绑定的 backup pruning，也不创建general filesystem abstraction。

`known_validated_backups` 是 recovery diagnostic 的保守 inventory，而非 filename scan：每个 pair 的 held manifest 限制为 4 KiB，必须同时符合 current manifest format、source schema 1、target schema 2、complete marker、size/digest 和 directory-entry identity；held backup descriptor 还必须证明 DELETE-journal SQLite header、schema v1 与完整 base rows。任何不兼容或无界 candidate 都不进入 `DatabaseCorrupt.backups`。`doctor --fix` 的 v1 diagnosis 在取得 lock 后若已被另一 contender 完成，则重新报告 healthy `unchanged`；derived repair 也必须先分离 orphaned FTS shadow schema 才可重建。

## Plan Revision Note


2026-08-20：创建PLAN-0005 planning baseline。基于completed PLAN-0004与当前实现选择indexed offline Library reads；经用户决定把`SKL-LIB-004`提升到Revision 2并固定纯文本词项AND；固定schema v2 content-bearing FTS、explicit doctor migration/repair、v1 read compatibility、10,000-entry预算和dependency边界。11:36Z 将 initial commit `88eec453bbb7a08dea160601fa66093398be9c72` 推送到 delivery branch，创建 Draft PR https://github.com/bootids/skilload/pull/5，并写回 canonical URL、Progress 与 publication evidence。该修订只更新delivery metadata；在获得后续明确execution prompt前不实现runtime行为。

2026-08-20 12:03Z：处理 PR #5 首轮规划评审。三个 inline 问题（FTS group 间显式 `AND`、base corruption 走 `database_corrupt` error、pre-open generation gate 防 WAL sidecar）均按 planning 边界以文档修订处置：更新本 Plan 的 Design Inputs、Milestone 2/3、Validation、Product Baseline 可观察路径、Surprises & Discoveries、Decision Log、Progress 与 Review Conversation Log，同步 `docs/design-docs/application-and-persistence.md`、`docs/references/sqlite-fts5-library-search.md` 与 `docs/references/sqlite-backup-and-corruption-recovery.md`。产品语义（`SKL-LIB-004` Revision 2）不变；未改动任何运行时代码，Plan 保持 `plan`、PR 保持 Draft。

2026-08-20 12:20Z：进入执行。前置验证全部通过（依赖 completed、PR Draft、branch/HEAD 一致）；本文件移入 `docs/exec-plans/active/`，`status` 改为 `active`。未改动其他内容。

2026-08-20 13:36Z：完成全部四个 milestones 的实现与验收。运行时代码变更：`crates/skilload-core`（domain library/doctor、ports library/doctor、application library/doctor、adapters/sqlite_library、application/configuration 的 `Application::new` 签名）与 `crates/skilload-cli`（args/main/json/human/tests）；依赖仅按既定 Decision 增加 `rusqlite` `backup` feature 与 `sha2 =0.11.0`。同步 `docs/product-specs/README.md`、`docs/product-specs/library.md`、`docs/product-specs/cache-and-operations.md`、`ARCHITECTURE.md`、两个 design docs 的实现状态；Progress、Surprises & Discoveries、Decision Log、Outcomes & Retrospective 与 Artifacts 已记录实现证据。实现中的低风险决策（FTS drift 的 `invalid_state` 分类、`repository_display` 列、linkat backup 发布、corruption details enrichment、v1 测试 fixture 生成方式）均已记录在 Decision Log。


2026-08-20 13:44Z：进入 review。Ready 事务证据：`gh pr ready` 成功（"Pull request bootids/skilload#5 is marked as \"ready for review\""），随后 `gh pr view --json isDraft,headRefOid,state` 观察到 `isDraft: false`、`state: OPEN`、`headRefOid: 7f9fd769b12eb75f051c1f29aaece9dd4a292c6b`（等于已推送的 implementation HEAD）。最终 validation（fmt/clippy -D warnings/全 workspace tests 11+17+135/build --locked/`git diff --check`）全部通过，证据见 Artifacts。

2026-08-20 14:39Z：处理 PR #5 第二轮实现评审（`address-pr-threads`，final review 模式）。Codex review `PRR_kwDOT7YN2s8AAAABKQk3WA` 的 7 个 inline 问题（FTS shadow 分类、backups 目录项同步、backup digest/symlink 校验、prune 保护当前 backup、mutation 路径 corruption 补全、锁内 FTS 重诊断、doctor identity 重验）全部按 `fixed` 处置：实现修复与 6 个新回归测试以 `4469112367ecb145cc5755100c1959a5de5934e6` 推送到 PR head，全部 thread 已回复并 resolved；逐表 integrity_check、shadow 损坏的 SQL 不可清除性与 `writable_schema` 手术事实同步到 `docs/references/sqlite-fts5-library-search.md`。最终 validation：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（11+17+141，debug 与 release）、`cargo build --workspace --locked`、`git diff --check` 全部通过。Plan 保持 `review`、PR 保持 ready。

2026-08-21：第三轮 final-review remediation。四个新 inline 问题均为 Product Baseline 内的普通修复：以 held `/dev/fd` descriptor 消除 read gate/open race、取消无法 identity-bound 的 backup prune、把 post-validation MATCH corruption归为 `library_fts_invalid`、以 held directory-relative `openat(..., O_NOFOLLOW)`验证 backup pair。实现、回归测试、`docs/design-docs/application-and-persistence.md` 与 preliminary Review Conversation Log 以 `7e5a7bda7ce2dc3804a687a4e7249944a7908980` 提交并推送；focused tests、fmt、clippy、workspace test（11+17+142）、locked build与`git diff --check`均通过。Plan 保持 `review`、PR 保持 ready；等待本 workflow 写入 GitHub replies/closures 与最终 Review Conversation Log reconciliation。

2026-08-21：第三轮 review conversation 已完成 reconciliation。PRRT_kwDOT7YN2s6a3XAK、PRRT_kwDOT7YN2s6a3XAV、PRRT_kwDOT7YN2s6a3XAd 与 PRRT_kwDOT7YN2s6a3XAk 均回复了 `7e5a7bda7ce2dc3804a687a4e7249944a7908980` 的实现与验证证据，reply URLs 和 `thread resolved: true` 已逐项写入 Review Conversation Log。final log commit `590b88b0a6aa71d7848d423abd68f10cce880e46` 已推送；其后完整 conversation read 未发现 pending/blocked source。

2026-08-21：final reconciliation 后的完整会话读取发现四个新增、空 body 的 `@bootids` submitted review containers；已在 Review Conversation Log 记录其 stable IDs/URLs 和 no-problem assessment。四个修复 thread 已回复并 resolved，未发现未记录、未回答或 blocked 的实际问题；该 source-complete note 已以 `bea597b1b3f5fc6307326520b4dcaf0d1eac4404` 推送并再次核验。

2026-08-21：第四轮 final-review remediation。PRRT_kwDOT7YN2s6bAVQR、PRRT_kwDOT7YN2s6bAVQT、PRRT_kwDOT7YN2s6bAVQV 与 PRRT_kwDOT7YN2s6bAVQX 的 recovery/repair 缺陷均在 Product Baseline 内修复：兼容 standalone backup inventory、锁后 migration re-diagnosis、4 KiB bounded manifest read 与 orphaned FTS shadow separation。实现与回归测试、`docs/design-docs/application-and-persistence.md` 和 preliminary Plan evidence 由 `648fb40323f2d35ac1dba6331501d0e03f7ecc6a` 推送；四个 replies 的 URLs 和 resolved state 已逐项记录到 Review Conversation Log。focused 70 tests、workspace fmt/clippy/test（11+17+146）、locked build与`git diff --check`均通过；Plan 保持 `review`、PR 保持 ready。

2026-08-21：第四轮最终 reconciliation 读取并记录五个 top-level sources、22 个 review bodies 与 18 个 threads；新增四个 empty review containers 不含实际问题。所有 18 个 inline problem source 都有 Plan entry、reply URL 和 `thread resolved: true`，PR head 与本地/remote 均为 ready `review` 状态；本次 Plan revision 专门补齐该 source-complete evidence。

2026-08-21：第五轮 final-review remediation 与 reconciliation。PRRT_kwDOT7YN2s6bA7j1、PRRT_kwDOT7YN2s6bA7j5、PRRT_kwDOT7YN2s6bA7j7 与 PRRT_kwDOT7YN2s6bA7j- 均在 Product Baseline 内完成：rollback-journal descriptor gate、zero-version corruption、complete human `LibraryEntry` 及 operational FTS error propagation。实现/测试/架构、设计、product clarification 与 SQLite reference 由 `ffeea3a7e850712db8b4b89c19dd6bfddf84136b` 推送；预备 Plan evidence 为 `53aac2f625ca246fcaf00fc2865f636a60bab5e7`。四个 GitHub reply URLs 和 resolved states 已逐项记录，final conversation read 为 6 top-level comments、27 reviews、22 threads（均 resolved），无未记录、未回答或 blocked 的实际问题；Plan 继续保持 `review`、PR 保持 ready。

2026-08-21：第六轮 final-review reconciliation。PRRT_kwDOT7YN2s6bB2FR、PRRT_kwDOT7YN2s6bB2FX、PRRT_kwDOT7YN2s6bB2Fb、PRRT_kwDOT7YN2s6bB2Fg 与 PRRT_kwDOT7YN2s6bB2Fm 分别修复 malformed derived FTS schema recovery、recoverable export diagnostics、write-time FTS special integrity、human recovery assets 与 migration backup export collision。实现、tests、产品 Revision 5、design/reference 与 preliminary Review Conversation Log 由 `b581acb63df42a882e0f02d5167a931fdf6e47f0` 推送；五个 reply URLs 和 resolved states 已逐项写入本 Log。最终完整读取为 7 top-level comments、33 reviews、27 threads，所有 thread resolved、无未记录、未回答或 blocked actual problem；新增 trigger、自动化 wrapper 与五个空 reply containers 均无独立问题。focused tests、core 153、CLI 13+17、workspace fmt/clippy/test/build 与 `git diff --check` 均通过。

2026-08-21：第七轮 final-review remediation。PRRT_kwDOT7YN2s6bDCYF 与 PRRT_kwDOT7YN2s6bDCYM 都在 Product Baseline 内修复：existing-database read gate 现在将 root-validated data-directory descriptor、relative main-file entry、header/sidecar inspection 与 `/dev/fd` SQLite source 连续绑定，并以 nonblocking open 拒绝 FIFO race。实现、两项新 regression、架构/持久化设计、preliminary Review Conversation Log 由 `0a1cad3897588623b77c69b0fe90279a9d770257` 推送；focused 与 workspace validation 均通过。Plan 保持 `review`、PR 保持 ready；待本 workflow 回复并 resolve 两个 thread 后完成 reconciliation。

2026-08-21：第七轮 final-review reconciliation。PRRT_kwDOT7YN2s6bDCYF 与 PRRT_kwDOT7YN2s6bDCYM 的修复 commit、workspace validation、reply URLs 与 resolved states 已逐项写入 Review Conversation Log。最终完整会话读取为 8 个 top-level comments、36 个 reviews 与 29 个 threads；新增 trigger `IC_kwDOT7YN2s8AAAABP9F5SA` 以及自动化 wrappers `PRR_kwDOT7YN2s8AAAABKRoboQ`、`PRR_kwDOT7YN2s8AAAABKVvOlQ`、`PRR_kwDOT7YN2s8AAAABKW3ROQ` 均未提出独立问题，所有 29 个 inline source 已记录、回复并 resolved。

2026-08-21 最终完整会话 reconciliation：PR #5 有 8 个 top-level comments、36 个 submitted reviews 与 29 个 review threads。所有 inline thread 的 current `isResolved` 均为 true；所有 thread ID 均已在本 Log 记录。`IC_kwDOT7YN2s8AAAABP9F5SA` 是 `@codex` trigger，`PRR_kwDOT7YN2s8AAAABKRoboQ`、`PRR_kwDOT7YN2s8AAAABKVvOlQ` 与 `PRR_kwDOT7YN2s8AAAABKW3ROQ` 只是自动化 review wrapper，均无独立 requested change、defect、question 或 constraint；没有 pending 或 blocked source。

2026-08-21：第八轮 final-review remediation 与 reconciliation。PRRT_kwDOT7YN2s6bD4zc、PRRT_kwDOT7YN2s6bD4zh 与 PRRT_kwDOT7YN2s6bD4zj 分别修复 standalone backup companion inventory、descriptor-bound live read 的 pre-open/snapshot 双重 census，以及 corruption diagnostics 的 held-root binding。运行时代码、产品 clarification、持久化设计、SQLite reference 与 preliminary Review Conversation Log 由 `8a0d84dc1e6de9959c0423f99273aa214c4f38b8` 推送；三个 reply URL 和 resolved state 已逐项记录。最终完整会话读取为 9 个 top-level comments、40 个 reviews 与 32 个 threads，所有 actual inline sources 都有 Log entry/reply/resolution，无 pending 或 blocked source。本次 revision 完成最终 Plan log evidence，Plan 保持 `review`、PR 保持 ready。

2026-08-21：第九轮 final-review reconciliation。PRRT_kwDOT7YN2s6bF0oA、PRRT_kwDOT7YN2s6bF0oG 与 PRRT_kwDOT7YN2s6bF0oR 的 `a140aad0f9fa85c0a9cb74f433793e4644bd2ce4` 修复证据、workspace validation、reply URLs 与 resolved states 已逐项写入 Review Conversation Log。最终完整会话读取为 10 个 top-level comments、44 个 reviews 与 35 个 threads；所有 thread 均为 resolved，三个新增 empty review containers 无独立问题，未发现 pending、blocked 或未记录 source。Plan 保持 `review`、PR 保持 ready。

2026-08-21：第十轮 final-review reconciliation。将 PRRT_kwDOT7YN2s6bKUKq 与 PRRT_kwDOT7YN2s6bKUK8 的 `9dc0fd058d54cf67f4d9e3edea5e9d7cdabc34f0` 修复、workspace validation、两个 GitHub reply URL 与 resolved states 写入 Review Conversation Log；同时澄清 `SKL-LIB-004` Revision 2 的既有 NFC query-term 行为而不改变 revision。最终完整会话读取为 11 个 top-level comments、47 个 reviews 与 37 个 threads，所有 source 均已记录或无独立问题，Plan 保持 `review`、PR 保持 ready。

2026-08-22：第十一轮 final-review preliminary remediation。完整会话读取确认新 top-level trigger `IC_kwDOT7YN2s8AAAABQDWwtw` 与 automated wrapper `PRR_kwDOT7YN2s8AAAABKbzRzA` 无独立问题；三个 inline defects PRRT_kwDOT7YN2s6bN3dy、PRRT_kwDOT7YN2s6bN3d2、PRRT_kwDOT7YN2s6bN3d7 均在 Product Baseline 内。`f032f9c1f087fa72b7ca55666e8b5d92e3149f27` 已推送 fail-closed backup inventory、writable connection post-revalidation identity check、FTS detach/VACUUM reference 与本初步 Review Conversation Log；focused regressions与 workspace fmt/clippy/test/locked build 全部通过。下一步回复/resolve threads 并写入最终 SHA/URLs。
