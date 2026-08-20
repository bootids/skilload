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

* `docs/product-specs/library.md` 中 `SKL-LIB-004` Revision 2：使用 embedded FTS5 索引 verified name、description、alias、tag display spelling、tag comparison key、category、note 和 repository。用户查询是纯文本词项 AND，不是 raw FTS5 expression；Unicode 15.1.0 `White_Space` 分词、NFC 原文/完整默认大小写折叠 alternatives、完整 FTS string quoting 与空查询错误都属于本 revision。该 Revision 2 由 2026-08-20 的产品选择确定，替代尚未实现且未规定 query language 的 Revision 1。
* 同一文件中 `SKL-LIB-005` Revision 1：`library list`、`library search` 和 `library get` 只读本地 durable metadata且不联网、不刷新、不写 derived state。List/search 在分页前按 canonical source binary order 排序；仅二者接受 limit 1..=1,000（默认 100）与完整 `u64` offset（默认 0），并返回 requested page、returned count 和 pre-page total。
* 同一文件中 `SKL-LIB-011` Revision 1：完成已有 import/export/metadata mutation 与本交付 list/indexed-search/get 的 10,000-entry 组合证据。代表性 release-build exact get、第一页 list 和 full-text search 各使用 10 秒本地验收预算；fixture 构造和 schema migration 不计入单次 query 计时，永久 CI tests 验证语义而不使用容易抖动的 wall-clock assertion。
* `docs/product-specs/cache-and-operations.md` 中 `SKL-OPS-003` Revision 1：当前唯一 forward migration 在任何 live schema write 前生成 standalone、durable、带 manifest 的 online backup，再以一个 SQLite transaction 从 v1 升到 v2；失败留下 prior readable v1 加完整 backup，或完整 durable v2，绝不留下被报告为成功的部分 schema。Unknown newer schema 和 downgrade 保持 write refusal。

以下行为约束本切片，但本计划不把跨产品尚未满足的 acceptance 误报为完成项。

* `SKL-CLI-001` 仍为 planned，因为 50 个 canonical leaves 尚未全部实现；本切片只把 `library list|search|get` 与真实 `doctor [--fix]` 从 usage error 切换到 application operations，不能增加 alias 或 scaffold。
* `SKL-CLI-004`、`SKL-CLI-005` 与 `SKL-CLI-012` Revision 2 已由 `PLAN-0003` 完成。本交付必须扩充现有 API-v2 producer：`library.list` 返回 `LibraryEntriesData`，`library.search` 返回 `LibrarySearchData`，`library.get` 返回 `LibraryEntry`，`doctor` 返回 `DoctorData`；success/error stdout仍是一个 JSON value，read outcome 为 `observed`，路径仍用 `PathValue`，pagination 的 `offset`/`total` 仍用 lossless decimal strings。
* `SKL-OPS-005` 与 `SKL-OPS-008` 要求 absent reads/default doctor 不创建任何 root且全部离线。`SKL-LIB-001` 允许但不要求 derived-name convenience selector；本切片的 `library get` 只接受完整 canonical source，避免在 alias/name precedence 与 ambiguity candidates 尚无完整产品契约时猜测。
* `SKL-OPS-002` 仍未完成，因为同一 durable database 的 Trust、global、manager、workspace 等 future tables 尚不存在；本切片只加入其要求的 FTS5 derived index而不声称完整 ownership model。
* `SKL-OPS-004`、`SKL-CACHE-006` 与 `SKL-CACHE-007` 仍未完成，因为完整 corruption restore、cache/deployment/workspace/manager inspection和所有 future repair 尚不可达。本切片只实现 real current-database doctor：只读 schema/base/FTS diagnosis、v1 migration和 FTS-only rebuild。Base corruption继续返回 `database_corrupt`并指向既有 `database-corruption-v1` operator procedure；不得增加隐藏 reset/restore 命令。

完成时的可观察路径是：在 isolated XDG roots 中导入两个条目；list 按 source 排序并分页；search 通过每个索引字段命中且把 `OR`、`NOT`、`*`、引号和 `name:...` 当普通文本；get 对 canonical source返回 entry、对 missing source返回 `not_found`。默认 doctor在 absent/healthy/v1/FTS-drift/corrupt/WAL-sidecar fixtures 上不改变 filesystem bytes或 timestamps；corrupt与 WAL-sidecar generation返回 typed `database_corrupt` error而不是成功 DoctorData。对 v1 fixture运行 `doctor --fix` 后，可看到一对验证通过的 backup/manifest和 `migrate` action，schema成为 v2、base rows与 `state_revision` 不变、search开始工作；对 FTS-only drift运行 fix只产生 `repair` action且不改 base metadata。

规划基线已经把 Revision 2 query semantics同步到 `docs/product-specs/README.md`、`docs/product-specs/library.md`，把技术选择同步到 `docs/design-docs/application-and-persistence.md`、`docs/design-docs/cli-json-and-release.md`，并新增 `docs/references/sqlite-fts5-library-search.md`。执行完成时只更新这些文件、`ARCHITECTURE.md` 和本 Plan 的实现状态与实际证据；不得再次改变这里固定的产品语义，除非先按 repository rule取得新的产品决定并提升 behavior revision。

## Design and Architecture Inputs


`ARCHITECTURE.md` 要求依赖向内：CLI 只解析参数、调用一个 application operation并渲染；application通过 focused ports协调；domain不能导入 CLI、SQLite、filesystem或process；SQLite adapter独占 SQL、XDG path、database identity、lock、backup、migration、sync和 FTS maintenance。Library只拥有 source/metadata；FTS是从 `library_entries` 与 `library_tags` 重建的派生索引，不能成为第二个 owner，也不能授予 Trust。当前无 Trust table，所有可达 `LibraryEntry.trust_state` 继续如实为 `missing`。

`docs/design-docs/application-and-persistence.md` 固定 `data/skilload.db`、`state/locks/database.lock`、pairwise-disjoint XDG roots、no-follow main-file identity gate、DELETE journal mode、一个 global durable-database mutation lock、transactional state mutation 与 descriptor-bound durability sync。当前 schema v1 有 `schema_info`、`state_revision`、`library_entries` 和 `library_tags`，没有 FTS。Schema v2只新增普通 content-bearing `library_fts` virtual table；不重建 base tables，也不引入 integer surrogate identity。每个 FTS row保存 unindexed canonical source与八类 indexed text columns；adapter在 import/metadata mutation 的同一 transaction中显式维护，migration/doctor从 base rows完整重建。

FTS tokenizer固定为 bundled SQLite 的 `unicode61 remove_diacritics 0`。Domain 使用 `crates/skilload-core/src/domain/unicode_15_1.rs` 的固定 `is_white_space`、NFC与 `full_case_fold`生成逻辑词项；adapter只负责把每个 literal中的 `"`写成`""`并包围双引号，然后将同一词项的 raw/folded alternatives以括号内 OR组合成一个 group，不同词项的 group之间以显式 `AND` 连接。FTS5 的 implicit AND只存在于裸 quoted phrases之间；括号表达式与后续 phrase或另一个 group之间不存在隐式组合，`("Review" OR "review") "code"` 是 syntax error，因此组合必须显式。用户字符串永远不作为 FTS grammar拼接。Tag display strings与 comparison keys分别用 ASCII newline聚合到不同列；newline是 tokenizer separator，不改变 tag storage。

`docs/references/sqlite-fts5-library-search.md` 记录 FTS5 string quoting、bare-phrase implicit AND的语法边界与显式 `AND` 组合、`unicode61`、content-bearing index、special `integrity-check`/`rebuild` 与 rusqlite backup API事实。`docs/references/sqlite-backup-and-corruption-recovery.md` 规定 live WAL generation不能靠复制 main file备份；migration必须用 SQLite online backup得到 standalone snapshot，并记录 read-only SQLite 打开 WAL-mode generation会创建 `-shm`/`-wal` sidecar、`immutable=1` 会忽略 WAL 内容的实验事实。为 `rusqlite 0.40.2` 启用 `backup` feature，并加入 `sha2 0.11.0`（`default-features = false`）流式计算 SHA-256；不得引入 async runtime、ORM、外部 search service或通用 migration framework。

Read兼容性是显式边界。完整 v1 base rows可供 list/get/export只读；search和所有 database writes返回现有 API-v2 `migration_required`，直到 `doctor --fix`。Default doctor从 identity-bound read-only source向内存 SQLite destination做 online backup，在副本上执行需要 writable connection的 FTS5 special check，因此 live XDG state保持不变。该不变性不能仅由 read-only flag推出：read-only SQLite connection打开 WAL-mode generation时会在可写 data directory创建并保留 `-shm`（header 为 WAL 而 sidecar缺失时连 `-wal` 也一并创建），`immutable=1` 虽不创建 sidecar却忽略 WAL 内容。因此所有对已存在 live database的 read-only opens（list/get/search/export/doctor inspect）在打开 SQLite 前必须先经 pre-open generation gate：用既有 no-follow held descriptor直接读取 main file的 100-byte header并盘点 `-wal`/`-shm` sibling；journal-mode bytes（偏移18/19）非 (1,1) 或存在任一 WAL sibling的 generation不属于任何 skilload 二进制可能发布的 DELETE-journal state，不经 SQLite 打开，直接按 base corruption返回 `database_corrupt`。Unknown newer schema只报告并拒绝 fix/write。FTS-only drift在 base tables、foreign keys和每个 domain row均验证成功后才能重建；schema migration和 derived repair都不推进 product `state_revision`。

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
- [ ] (2026-08-20) 处理 PR #5 第二轮实现评审的 7 个 inline 问题（FTS shadow 分类、backups 目录项同步、backup digest/symlink 校验、prune 保护当前 backup、mutation 路径 corruption 补全、锁内 FTS 重诊断、doctor identity 重验）并回复/resolve 全部 thread。

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
- Observation: `writable_schema` schema-row 手术可以在同一 transaction 内完整重建：ON → DELETE 6 行 schema → OFF → `PRAGMA schema_version` +1 强制重解析 → CREATE VIRTUAL TABLE → 按 base rows 填充 → validate → FTS5 `integrity-check` → commit 全部成功；旧 shadow pages 成为 orphan pages。
  Evidence: scratch probe 逐步输出（delete 6 行、create/insert/derived/icheck/commit 全 Ok）；`fts_shadow_corruption_stays_doctor_fixable` 经公开 `fix()` 路径复验。

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

- Decision: 对物理 damaged 的 FTS shadow b-tree，rebuild 先用逐 shadow `PRAGMA integrity_check` + 存在性检测识别（`fts_shadow_btree_is_damaged`），再以 `writable_schema` schema-row 手术（`detach_damaged_fts_schema`）替代常规 `DROP TABLE`；健康的 derived drift 仍走 DROP+recreate。
  Rationale: 2026-08-20 实验证实 damaged shadow 上的 DROP/DELETE 必然以 `SQLITE_CORRUPT` 失败并毒化 transaction；手术在同一 transaction 内删除 vtab + 5 个 shadow 的 schema 行并 bump `schema_version` 后重建，是 SQLite 文档记载的 schema 级恢复机制，base rows 与 state_revision 不受影响。代价是旧 shadow pages 成为 orphan pages（仅整库 integrity_check 可见，逐表 base 验证与 FTS5 special `integrity-check` 均不受影响），换取 `doctor --fix` 对 FTS-only 物理损坏真正可修复（PRRT_kwDOT7YN2s6a1M6h 的产品契约要求）。
  Date/Author: 2026-08-20 / Codex

## Outcomes & Retrospective


实现已完成（2026-08-20 13:36Z），全部 Product Baseline 行为已交付并验证。`SKL-LIB-004` Revision 2：嵌入式 FTS5 索引八类字段（含 tag display/key 双列），纯文本词项 AND 查询经 pinned Unicode 15.1.0 分词 + NFC/case-fold alternatives + 完整 FTS5 string quoting 编码（同词项括号 OR group、跨词项显式 `AND`），操作符/引号/列过滤全部保持 literal，空查询在 SQLite 前以 `validation_failed/library_search_query_empty` 拒绝。`SKL-LIB-005` Revision 1：`library list/search/get` 离线读取 canonical-source binary order、确定性分页（limit 1..=1000 默认 100、全量 u64 offset 默认 0、offset≥total 返回空页）、不创建任何 root、不改变 database bytes/mtime。`SKL-LIB-011` Revision 1：10,000-entry release 实测 exact get 0.06s、first list page 0.06s、full-text search 0.12s、deep-offset search 0.12s（Apple M4 Pro，预算各 10 秒），count/order 确定性由永久 tests 断言。`SKL-OPS-003` Revision 1：v1→v2 migration 在任何 live write 前发布 standalone online-backup pair（`data/backups/` + completed manifest：SHA-256/size/source identity/epoch-ns）并验证 digest/schema/base/revision，随后单事务创建+填充 FTS+更新 schema version，state revision 前后不变；failpoint 证据区分"v1+完整 backup"与"durable v2 但命令报错"；unknown newer schema 与 downgrade 保持拒绝。Doctor：默认只读（online backup 到 `:memory:` 运行 FTS5 integrity-check，filesystem 完全惰性，覆盖 absent/healthy/v1/FTS-drift/corrupt/WAL fixtures），`--fix` 交付 migrate/repair action 并可重复（healthy 重复 fix 返回 unchanged 且不产生第二个 backup）。Base corruption 与 WAL/sidecar generation 返回 typed `database_corrupt`（含已验证 backups 与 `database-corruption-v1`）；FTS-only drift 是 fixable finding。验证：`cargo test --workspace` 135 core + 28 CLI（11 bin + 17 contract）全部通过；focused 过滤覆盖 domain query（`library_search`）、repository/migration（`sqlite_library`）与 CLI（`library_reads`、`doctor`）；debug binary smoke 与 10k release 测量记录于 Artifacts。遗留：无——本计划范围内的所有 acceptance 均已满足；跨产品尚未满足的行为（`SKL-CLI-001` 完整 50 leaves、`SKL-OPS-002` 完整 ownership、`SKL-CACHE-006/007` 跨域 doctor）保持 planned 且未被误报为完成。

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

Status: open

Resolution: 已实现：`validate_base_database`（`crates/skilload-core/src/adapters/sqlite_library.rs`）改为对 `sqlite_master` 与四个 v1 base 表逐表运行 `PRAGMA integrity_check('<table>')`（新增 `BASE_INTEGRITY_TABLES`），FTS shadow 健康不再影响 base 分类；`derived_index_is_consistent` 中内存副本上的 FTS5 `integrity-check` 失败改为返回 `false`（可修复 finding）而不是向上传播。由于物理 damaged 的 shadow b-tree 连 `DROP TABLE`/`DELETE` 都以 `SQLITE_CORRUPT` 失败（实验证实），`rebuild_derived_index` 新增 damage 检测（`fts_shadow_btree_is_damaged`：逐 shadow `PRAGMA integrity_check` + 存在性检查）与 `writable_schema` schema-row 手术（`detach_damaged_fts_schema`：同一 transaction 内删除 vtab + 5 个 shadow 的 schema 行、bump `schema_version` 强制重解析后重建），使 `doctor --fix` 对物理 shadow 损坏也真正可 rebuild；旧 shadow pages 成为 orphan pages，不影响逐表 base 验证。逐表检查失去的唯一整库信号是 freelist/orphan-page 记账，不影响 base records 完整性证明。

Evidence: 新增回归测试 `fts_shadow_corruption_stays_doctor_fixable`：损坏 `library_fts_data` root page 尾部 cell 字节后，先断言 fixture 前提（整库 `PRAGMA integrity_check` 非 `ok`，输出 `Tree 9 page 9 cell 1: Extends off end of page`），再验证 `inspect()` 返回单个 `library_fts_invalid` finding（非 `database_corrupt`）、`fix()` 返回 `repair` action、search 恢复命中、`inspect()` 健康。既有 135 个 core tests（含 `corrupt_base_keeps_typed_details_with_known_backups` 的 page-1 corruption 仍被 `sqlite_master` 逐表检查捕获）全部通过；`cargo test --workspace` 11+17+141（debug 与 release）、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cargo build --workspace --locked` 均通过。commit 待推送后回填。

GitHub outcome: 待回复。

### PRRT_kwDOT7YN2s6a1M6o - 新建 backups 目录的目录项必须同步


Source: PRRT_kwDOT7YN2s6a1M6o / PRRC_kwDOT7YN2s7j0Wo3（https://github.com/bootids/skilload/pull/5#discussion_r3822152247）

Problem: 首次 v1 migration 时 `data/backups` 通常不存在，但 `publish_validated_backup` 丢弃了 `ensure_restrictive_directory` 返回的 `CreatedDirectory` 记录；fsync backups 目录只持久化其内容，不会让其在新父目录 `data/skilload` 中的新目录项 crash-durable，掉电后可能出现 schema v2 live database 却丢失必需 backup 的窗口。

Disposition: fixed

Status: open

Resolution: 已实现：`publish_validated_backup` 保留 `ensure_restrictive_directory` 的 `CreatedDirectory` 记录，在 staging 文件写入前调用 `sync_created_directory_entries(&created_directories, "XDG_DATA_HOME")`，使新建 `backups` 目录在其父 `data/skilload` 中的目录项在任何 staging 写入与 live schema write 之前 crash-durable。

Evidence: 修改位于 `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `publish_validated_backup` 开头；fsync 行为无法在测试中断言，回归由既有 `doctor_fix_migrates_v1_after_a_validated_backup`、`migration_failpoints_leave_a_coherent_state` 与新增 prune/backup 测试覆盖的迁移路径完整性保证。全部 workspace tests 通过。commit 待推送后回填。

GitHub outcome: 待回复。

### PRRT_kwDOT7YN2s6a1M6u - 已验证 backup 列表必须校验 SHA-256 并拒绝 symlink manifest


Source: PRRT_kwDOT7YN2s6a1M6u / PRRC_kwDOT7YN2s7j0Wo8（https://github.com/bootids/skilload/pull/5#discussion_r3822152252）

Problem: `known_validated_backups` 只检查 `complete` 与 `database_bytes`，从不比对 manifest 已携带的 SHA-256；backup 被等长篡改后仍会作为 validated backup 出现在每个 `database_corrupt` 响应中，且 `fs::read` 跟随 symlink，symlink manifest 也会被接受。

Disposition: fixed

Status: open

Resolution: 已实现：新增共享 predicate `backup_pair_is_valid(backups_root, stem)`——manifest 必须是 no-follow regular file（symlink 拒绝）、记录可解析、`complete`、database 为 regular file 且长度相等、`sha256` 与 `sha256_of_file` 流式哈希一致；`known_validated_backups` 与 `prune_old_backups` 共用同一 predicate。

Evidence: 新增回归测试 `tampered_or_symlinked_backups_are_never_validated`：(a) 等长翻转 backup 末字节（digest 漂移）后损坏 live database，`inspect()` 的 `DatabaseCorrupt.backups` 为空；(b) manifest 换成 symlink 后同样为空。既有 `corrupt_base_keeps_typed_details_with_known_backups` 仍验证未篡改 backup 列出 1 项。全部 workspace tests 通过。commit 待推送后回填。

GitHub outcome: 待回复。

### PRRT_kwDOT7YN2s6a1M6y - prune 必须保护本次 migration 刚发布的 backup


Source: PRRT_kwDOT7YN2s6a1M6y / PRRC_kwDOT7YN2s7j0WpD（https://github.com/bootids/skilload/pull/5#discussion_r3822152259）

Problem: `prune_old_backups` 先对全部 manifest 形态文件名应用保留截断再逐个验证，排序只依赖文件名中的 wall-clock timestamp；时钟回拨或外部 future-dated manifest 会使本次 migration 刚创建的 backup 落入删除切片并被立即删除，可能删掉上一 schema generation 的唯一 backup。

Disposition: fixed

Status: open

Resolution: 已实现：`prune_old_backups(backups_root, protected_stem)` 先用 `backup_pair_is_valid` 验证每个 pair 再进入保留排序（无效/外部条目不再占用保留名额，symlink pair 不再被误删）；`publish_validated_backup` 返回最终 stem，migration 把它作为 `protected_stem` 传入 prune，该 stem 永不进入删除集合；仅第 `RETAINED_COMPLETE_BACKUPS` 名之后的已验证 pair 会被删除。

Evidence: 新增回归测试 `prune_keeps_the_backup_of_the_current_migration`：预置 3 个 future-dated（20 位 epoch ns stem）但完全有效的 foreign pair 后执行 `fix()`，断言迁移后 4 个 pair 全部保留（旧逻辑会把排序最旧的本次迁移 backup 删除）、schema 为 v2、`inspect()` 健康。全部 workspace tests 通过。commit 待推送后回填。

GitHub outcome: 待回复。

### PRRT_kwDOT7YN2s6a1M60 - mutation 路径的 corruption 错误必须补全 backups


Source: PRRT_kwDOT7YN2s6a1M60 / PRRC_kwDOT7YN2s7j0WpE（https://github.com/bootids/skilload/pull/5#discussion_r3822152260）

Problem: `import`/`mutate_metadata` 的既有库分支与 `fix` 的 action 调用直接返回 `DatabaseCorrupt`，其 `backups` 为空；migration backup 恰好在写被拒绝时不被列出，与读取路径已统一执行的 `enrich_database_corruption` 不一致，违反 `DatabaseCorruptDetails` 必须列出已知 backups 的契约。

Disposition: fixed

Status: open

Resolution: 已实现：`import` 的三个 database 分支（dry-run `read_existing`、`import_existing`、`import_first`）与 `mutate_metadata` 的 `mutate_existing` 均经 `enrich_database_corruption` 包装；`fix()` 的 `migrate_v1`/`repair_fts` action 错误同样包装，所有公开入口的 `DatabaseCorrupt` 现在都带完整 `backups`。

Evidence: 新增回归测试 `mutation_paths_list_known_backups_on_base_corruption`：migration 产生 1 个 backup 后损坏 live database，`mutate_metadata` 与 `import` 的 `DatabaseCorrupt.backups` 均列出该 1 个 validated backup（修复前为空）。全部 workspace tests 通过。commit 待推送后回填。

GitHub outcome: 待回复。

### PRRT_kwDOT7YN2s6a1M62 - 取得修复锁后必须重新诊断 FTS drift


Source: PRRT_kwDOT7YN2s6a1M62 / PRRC_kwDOT7YN2s7j0WpH（https://github.com/bootids/skilload/pull/5#discussion_r3822152263）

Problem: 两个 `doctor --fix` 进程都在拿锁前诊断出 FTS drift 时，第一个修复后第二个进入 locked 路径，其中只校验 schema 与 base rows，随后无谓地 drop/rebuild 健康索引并返回 `changed`（stale finding 标记为 fixed），而不是幂等的 `unchanged`。

Disposition: fixed

Status: open

Resolution: 已实现：`repair_fts_locked` 改为 `Result<Option<DoctorAction>>`——先用只读 transaction 完成 schema/base 校验，再于 durable lock 内重新执行 `derived_index_is_consistent`，已一致时重验 identity 后返回 `None`（数据库字节不变），只有仍不一致才进入 rebuild transaction；`fix()` 收到 `None` 时对该 FtsInvalid 情形重新执行 `diagnosis_classification` 并以 `unchanged` 返回最新 findings 与 `database_writable`。

Evidence: 新增回归测试 `fts_repair_rechecks_drift_under_the_lock`：健康 v2 database 上直接调用 `repair_fts` 返回 `None` 且文件字节逐字节不变；制造 drift 后同一调用返回 `repair` action 且 search 恢复。`fix()` 的幂等 `unchanged` 分支由既有 `fts_drift_is_doctor_fixable_without_touching_base_rows` 的 repeated-fix 断言覆盖。全部 workspace tests 通过。commit 待推送后回填。

GitHub outcome: 待回复。

### PRRT_kwDOT7YN2s6a1M64 - doctor 返回诊断前必须重验 database identity


Source: PRRT_kwDOT7YN2s6a1M64 / PRRC_kwDOT7YN2s7j0WpO（https://github.com/bootids/skilload/pull/5#discussion_r3822152270）

Problem: `diagnosis_classification` 丢弃 `open_existing_database` 返回的 identity，不像 list/get/export 那样在快照前后重验；同账号其他进程原子替换 `skilload.db` 后，诊断继续描述旧 inode 却可能返回以替换后路径为 target 的健康结果或 finding，使恢复用的 doctor evidence 不安全。

Disposition: fixed

Status: open

Resolution: 已实现：`diagnosis_classification` 保留 `open_existing_database` 返回的 identity，并在返回诊断前按既有读取路径模式于 transaction commit 前后各执行一次 `revalidate_database_identity`；路径名被原子替换为其他 inode 时返回 `database_identity_drift` invalid_state 而不是基于旧 inode 的诊断。

Evidence: 新增回归测试 `doctor_never_reports_a_replaced_database`：以 `after_existing_database_open` hook 在打开后用 `rename` 原子替换 `skilload.db`，`inspect()` 与 `fix()` 均返回 `database_identity_drift`（open 尾部的既有重验与本条新增的快照前后重验共同构成该契约；若两级重验均被移除则测试失败）。全部 workspace tests 通过。commit 待推送后回填。

GitHub outcome: 待回复。

## Context and Orientation


仓库是两个 crate的 Rust workspace。`crates/skilload-core`保存 domain/application/ports/adapters；`crates/skilload-cli`保存 clap schema、dispatch与 JSON/human renderers。`skilload-core/src/domain/library.rs`已有 portable document、metadata mutations与 `LibraryEntry`；`domain/unicode_15_1.rs`暴露 pinned `normalize_tag`、`full_case_fold`和 `is_white_space`。`ports/library.rs`目前只有 transfer/import/export/mutate contracts，`application/library.rs`只有相应 operations。`adapters/sqlite_library.rs`是唯一 durable Library repository，负责 XDG resolution、database identity、schema validation、import/mutation transactions、locks与sync。`skilload-cli/src/args.rs`当前未注册 list/search/get/doctor，`main.rs`只 dispatch现有 config和 Library transfer/metadata leaves。

持久 Library位于有效 `XDG_DATA_HOME/skilload/skilload.db`；durable database process lock位于有效 `XDG_STATE_HOME/skilload/locks/database.lock`。Effective config/data/state/cache application roots必须两两不重叠。Absent read只能建立内存empty view，不能创建任何 root。SQLite main file必须是no-follow regular file，并在任意 SQL前用现有 `SQLITE_FCNTL_HAS_MOVED` helper绑定实际 inode；读取transaction前后还要重验 path identity。

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

Null optional metadata投影为空字符串。Tags按 comparison key稳定排序，display和comparison分别用 `\n`连接。每个 canonical source必须在 FTS table恰有一行；doctor比较这一完整集合与 base rows，不能只比较 row count。

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

把first import初始化切换为schema v2。新增一个接受transaction和完整 `PortableLibraryEntry`/stored entry的共享helper，按tag comparison order生成一行FTS projection。`apply_additions`在同一transaction插入base/tags/FTS；`apply_metadata_change`在同一transaction更新base/tags后替换目标FTS row。Changed mutation仍只推进一次 `state_revision`；unchanged不触发FTS DELETE/INSERT、transaction commit或sync。Existing v1 import/mutation在取得现有identity并验证base后返回 `MigrationRequired { found:1, supported:2 }`，不自动upgrade。

实现list/search/get read transactions。所有对已存在 database的 read-only opens先经 Design Inputs固化的 pre-open generation gate：WAL-mode header或 `-wal`/`-shm` sibling在 SQLite 前即返回 `database_corrupt`，不创建任何 sidecar。Absent list/search返回empty page且不创建roots；absent get返回`not_found`。V1 list/get从base rows工作；v1 search返回migration_required。V2 list/search先在同一transaction计算total，再在`offset >= total`时不做SQLite signed offset conversion并返回empty；否则用CTE选择canonical page并一次LEFT JOIN tags，按source/tag comparison order流式组装entries，避免每entry query。Search CTE只接收adapter从domain terms编码的bound MATCH string；用户raw query不得拼入SQL/FTS grammar。Get用exact selector和同样single-query tag assembly，missing返回`not_found`。

该里程碑的repository tests必须证明每个字段可搜索、raw/folded tag等价、operators保持literal、same-name sources共存、source-order先于pagination、adjacent pages不重叠、offset==total/`u64::MAX`为空、limit边界在SQL前失败、read transaction在concurrent writer下提供同一snapshot。验证v1 list/get/export继续工作而search/writes要求migration；v2 import/mutations每次保持FTS一致；corrupt/missing/extra FTS rows不会反向改base。

### Milestone 3：实现 backup、v1→v2 migration 与 doctor repair


在workspace dependency中给locked `rusqlite 0.40.2`增加`backup` feature，并加入 `sha2 = { version = "=0.11.0", default-features = false }`。不要升级其他locked crates。为 `SqliteLibraryRepository` 实现 `DatabaseMaintenance`，复用现有 XDG resolver、no-follow database identity、data-directory descriptor、global lock与durability helpers。

Default `inspect`在absent state返回healthy/empty `DoctorData`且不创建root。对existing database，先执行 pre-open generation gate：main file header的 journal-mode bytes非 (1,1) 或存在`-wal`/`-shm` sibling时，不打开 SQLite，直接返回typed `database_corrupt`错误，其 `DatabaseCorruptDetails` 含database `PathValue`、已验证backup manifests、仍可读portable exports与 `recovery_procedure: "database-corruption-v1"`，live filesystem零改动。通过gate的generation才以read-only identity-bound connection读取schema/base evidence；使用 `rusqlite::backup::Backup`复制到`:memory:` destination，在副本上执行完整v2 FTS content comparison与 `INSERT INTO library_fts(library_fts) VALUES('integrity-check')`。V1产生fixable `library_database_migration_required` finding并令 `database_writable=false`；healthy v2无finding且true；FTS-only drift产生fixable `library_fts_invalid`且false；newer产生unfixable `library_schema_newer` finding且false。Base corruption不进入 `DoctorData`：schema/base validation（schema version decode、integrity/foreign keys、domain rows）任何失败都按既有error mapping返回typed `database_corrupt`，满足 `SKL-OPS-004` 与 `docs/product-specs/database-recovery.md` 第1步对 doctor 返回 `database_corrupt` details的要求。可诊断generation的default outcome始终`observed`、`fix_requested=false`、每个finding `fixed=false`、actions empty。

`fix`先运行同一diagnosis，因此base corruption与WAL/sidecar generation同样在diagnosis阶段即返回`database_corrupt`，绝不进入action阶段或返回`unchanged`。Absent/healthy返回`unchanged`；newer保留finding并返回`unchanged`。V1 migration取得durable lock、重新绑定database/directory/schema/state revision baseline，然后在 `data/backups/` 的held no-follow directory中创建restrictive random staging database与manifest。Online backup完成后关闭destination、sync file、流式计算size/SHA-256、重开read-only验证schema/base/integrity/foreign keys、写completed manifest、sync并以no-clobber relative rename发布pair、sync parent。任何不匹配或collision保留foreign entry并返回typed error；只清理仍与held descriptors匹配的owned staging。

Backup durable后，在live connection的一个transaction创建 `library_fts`、从base rows填充、运行content comparison与FTS integrity check、最后把`schema_info.version`更新为2并commit。Migration前后 `state_revision`必须相等。Commit后执行现有 descriptor-bound database/file/parent sync和最终identity revalidation；sync后错误不得声称v1仍在。只有live v2 durable后，才可保守prune超过三代的完整validated backup pairs；永不删除invalid、unpaired、symlinked、digest-drifted或唯一immediately-previous-schema backup。

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

Repository acceptance使用真实bundled SQLite。每个name/description/alias/tag display/tag key/category/note/repository字段都有独立match fixture；`code review`可跨字段且不相邻；canonical source order在page之前；total/returned/offset/limit准确；offset `u64::MAX`不向SQLite做overflow conversion。Reads不持有mutation lock、不写state revision、不修改FTS或timestamps。V2 import/mutations在同一transaction保持base/index一致，failure rollback两者；unchanged不做derived rewrite。

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

Backup manifest是private versioned serde record，不进入API-v2或portable export。Digest用`sha2::Sha256`分块读取held backup file；大小和identity来自同一held descriptor。Manifest发布、pruning与live migration只使用validated directory handles和single-component relative names，复用现有native identity规则，不创建general filesystem abstraction。

## Plan Revision Note


2026-08-20：创建PLAN-0005 planning baseline。基于completed PLAN-0004与当前实现选择indexed offline Library reads；经用户决定把`SKL-LIB-004`提升到Revision 2并固定纯文本词项AND；固定schema v2 content-bearing FTS、explicit doctor migration/repair、v1 read compatibility、10,000-entry预算和dependency边界。11:36Z 将 initial commit `88eec453bbb7a08dea160601fa66093398be9c72` 推送到 delivery branch，创建 Draft PR https://github.com/bootids/skilload/pull/5，并写回 canonical URL、Progress 与 publication evidence。该修订只更新delivery metadata；在获得后续明确execution prompt前不实现runtime行为。

2026-08-20 12:03Z：处理 PR #5 首轮规划评审。三个 inline 问题（FTS group 间显式 `AND`、base corruption 走 `database_corrupt` error、pre-open generation gate 防 WAL sidecar）均按 planning 边界以文档修订处置：更新本 Plan 的 Design Inputs、Milestone 2/3、Validation、Product Baseline 可观察路径、Surprises & Discoveries、Decision Log、Progress 与 Review Conversation Log，同步 `docs/design-docs/application-and-persistence.md`、`docs/references/sqlite-fts5-library-search.md` 与 `docs/references/sqlite-backup-and-corruption-recovery.md`。产品语义（`SKL-LIB-004` Revision 2）不变；未改动任何运行时代码，Plan 保持 `plan`、PR 保持 Draft。

2026-08-20 12:20Z：进入执行。前置验证全部通过（依赖 completed、PR Draft、branch/HEAD 一致）；本文件移入 `docs/exec-plans/active/`，`status` 改为 `active`。未改动其他内容。

2026-08-20 13:36Z：完成全部四个 milestones 的实现与验收。运行时代码变更：`crates/skilload-core`（domain library/doctor、ports library/doctor、application library/doctor、adapters/sqlite_library、application/configuration 的 `Application::new` 签名）与 `crates/skilload-cli`（args/main/json/human/tests）；依赖仅按既定 Decision 增加 `rusqlite` `backup` feature 与 `sha2 =0.11.0`。同步 `docs/product-specs/README.md`、`docs/product-specs/library.md`、`docs/product-specs/cache-and-operations.md`、`ARCHITECTURE.md`、两个 design docs 的实现状态；Progress、Surprises & Discoveries、Decision Log、Outcomes & Retrospective 与 Artifacts 已记录实现证据。实现中的低风险决策（FTS drift 的 `invalid_state` 分类、`repository_display` 列、linkat backup 发布、corruption details enrichment、v1 测试 fixture 生成方式）均已记录在 Decision Log。


2026-08-20 13:44Z：进入 review。Ready 事务证据：`gh pr ready` 成功（"Pull request bootids/skilload#5 is marked as \"ready for review\""），随后 `gh pr view --json isDraft,headRefOid,state` 观察到 `isDraft: false`、`state: OPEN`、`headRefOid: 7f9fd769b12eb75f051c1f29aaece9dd4a292c6b`（等于已推送的 implementation HEAD）。最终 validation（fmt/clippy -D warnings/全 workspace tests 11+17+135/build --locked/`git diff --check`）全部通过，证据见 Artifacts。
