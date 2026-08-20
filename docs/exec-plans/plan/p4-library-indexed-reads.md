---
plan_id: PLAN-0005
branch: codex/p4-library-indexed-reads
pull_request: pending
status: plan
depends_on: [PLAN-0004]
---

# 实现 Library 索引化离线读取


本交付让用户在导入 Library 后，直接通过 `library list`、`library search` 和 `library get` 离线浏览本地元数据；搜索覆盖名称、描述、alias、tags、category、note 与 repository，并且不会把 SQLite 查询语言暴露给调用者。已有 schema v1 用户可先继续 list/get/export，再用显式 `doctor --fix` 生成可验证 backup 并升级到带 FTS5 索引的 schema v2；用户可从 JSON/human 结果、确定性分页、迁移证据和实际二进制 smoke 看到它工作。

本 ExecPlan 是 living document。执行期间必须持续维护 `Progress`、`Surprises & Discoveries`、`Decision Log`、`Outcomes & Retrospective` 与 `Review Conversation Log`。

仓库根目录的 `docs/PLANS.md` 是本计划内容、状态迁移和失败恢复的权威规则；本文件必须始终与其保持一致，并能让第一次进入仓库的实现者只依赖当前 worktree 和本文件完成交付。

## Delivery Metadata


本计划直接依赖 `PLAN-0004`。该前置计划已在默认分支的 `docs/exec-plans/completed/p3-library-metadata-mutations.md` 中完成，并通过传递依赖取得 `PLAN-0003` 的 portable Library、SQLite schema v1、API-v2 producer、Unicode 15.1.0 tag 规则和 `PLAN-0002` 的 Rust/configuration 基础；本计划不重复列出传递依赖。

交付只增加 Library 的三个离线读取叶子、支持它们的 FTS5 schema v2、一次 v1→v2 forward migration，以及对当前 durable database 的真实 `doctor [--fix]` 路径。它不实现 `library add|remove|refresh`、GitHub resolution、Trust mutation、cache、workspace、global、manager、deployment、HTTP、TUI、Web 或完整未来 doctor domain inventory。未实现命令继续是 usage error，不能注册 placeholder。首次推送后创建 Draft PR，把 frontmatter 的 `pull_request: pending` 改为 canonical HTTPS URL，再独立提交推送；在后续明确执行授权前，Plan 必须保持 `plan` 且 PR 必须保持 Draft。

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

完成时的可观察路径是：在 isolated XDG roots 中导入两个条目；list 按 source 排序并分页；search 通过每个索引字段命中且把 `OR`、`NOT`、`*`、引号和 `name:...` 当普通文本；get 对 canonical source返回 entry、对 missing source返回 `not_found`。默认 doctor在 absent/healthy/v1/FTS-drift fixtures 上不改变 filesystem bytes或 timestamps。对 v1 fixture运行 `doctor --fix` 后，可看到一对验证通过的 backup/manifest和 `migrate` action，schema成为 v2、base rows与 `state_revision` 不变、search开始工作；对 FTS-only drift运行 fix只产生 `repair` action且不改 base metadata。

规划基线已经把 Revision 2 query semantics同步到 `docs/product-specs/README.md`、`docs/product-specs/library.md`，把技术选择同步到 `docs/design-docs/application-and-persistence.md`、`docs/design-docs/cli-json-and-release.md`，并新增 `docs/references/sqlite-fts5-library-search.md`。执行完成时只更新这些文件、`ARCHITECTURE.md` 和本 Plan 的实现状态与实际证据；不得再次改变这里固定的产品语义，除非先按 repository rule取得新的产品决定并提升 behavior revision。

## Design and Architecture Inputs


`ARCHITECTURE.md` 要求依赖向内：CLI 只解析参数、调用一个 application operation并渲染；application通过 focused ports协调；domain不能导入 CLI、SQLite、filesystem或process；SQLite adapter独占 SQL、XDG path、database identity、lock、backup、migration、sync和 FTS maintenance。Library只拥有 source/metadata；FTS是从 `library_entries` 与 `library_tags` 重建的派生索引，不能成为第二个 owner，也不能授予 Trust。当前无 Trust table，所有可达 `LibraryEntry.trust_state` 继续如实为 `missing`。

`docs/design-docs/application-and-persistence.md` 固定 `data/skilload.db`、`state/locks/database.lock`、pairwise-disjoint XDG roots、no-follow main-file identity gate、DELETE journal mode、一个 global durable-database mutation lock、transactional state mutation 与 descriptor-bound durability sync。当前 schema v1 有 `schema_info`、`state_revision`、`library_entries` 和 `library_tags`，没有 FTS。Schema v2只新增普通 content-bearing `library_fts` virtual table；不重建 base tables，也不引入 integer surrogate identity。每个 FTS row保存 unindexed canonical source与八类 indexed text columns；adapter在 import/metadata mutation 的同一 transaction中显式维护，migration/doctor从 base rows完整重建。

FTS tokenizer固定为 bundled SQLite 的 `unicode61 remove_diacritics 0`。Domain 使用 `crates/skilload-core/src/domain/unicode_15_1.rs` 的固定 `is_white_space`、NFC与 `full_case_fold`生成逻辑词项；adapter只负责把每个 literal中的 `"`写成`""`并包围双引号，然后将同一词项的 raw/folded alternatives以 OR、不同词项以 implicit AND组合。用户字符串永远不作为 FTS grammar拼接。Tag display strings与 comparison keys分别用 ASCII newline聚合到不同列；newline是 tokenizer separator，不改变 tag storage。

`docs/references/sqlite-fts5-library-search.md` 记录 FTS5 string/implicit-AND、`unicode61`、content-bearing index、special `integrity-check`/`rebuild` 与 rusqlite backup API事实。`docs/references/sqlite-backup-and-corruption-recovery.md` 规定 live WAL generation不能靠复制 main file备份；migration必须用 SQLite online backup得到 standalone snapshot。为 `rusqlite 0.40.2` 启用 `backup` feature，并加入 `sha2 0.11.0`（`default-features = false`）流式计算 SHA-256；不得引入 async runtime、ORM、外部 search service或通用 migration framework。

Read兼容性是显式边界。完整 v1 base rows可供 list/get/export只读；search和所有 database writes返回现有 API-v2 `migration_required`，直到 `doctor --fix`。Default doctor从 identity-bound read-only source向内存 SQLite destination做 online backup，在副本上执行需要 writable connection的 FTS5 special check，因此 live XDG state保持不变。Unknown newer schema只报告并拒绝 fix/write。FTS-only drift在 base tables、foreign keys和每个 domain row均验证成功后才能重建；schema migration和 derived repair都不推进 product `state_revision`。

## Purpose / Big Picture


当前用户可以通过 portable import建立 Library并修改 metadata，但只能重新 export才能查看全集，也不能按 note/tag/name搜索。完成后，用户可在网络关闭时列出、分页、检索和精确读取本地条目；human 与 API-v2显示同一 entries、query和 page metadata。已有 v1 database不会被读取命令暗中升级：用户先看到 doctor finding，再显式 fix并保留可验证 backup。任何 migration或 FTS repair失败都不会把 base Library或用户 metadata当作派生索引牺牲品。

## Progress


- [x] (2026-08-20 11:26Z) 从 clean、updated `main` 建立 `codex/p4-library-indexed-reads`；核对所有四个 completed Plans、产品规格、架构、设计、references与当前 Rust实现；确认没有现有 PLAN-0005、同名 branch或 Draft PR。
- [x] (2026-08-20 11:26Z) 取得产品决定：`library search`采用纯文本词项 AND，不采用完整短语或 raw FTS5 language；将 `SKL-LIB-004`提升为 Revision 2并同步规划设计/reference。
- [x] (2026-08-20 11:26Z) 创建本 `plan`-status ExecPlan；当前尚未提交、推送或创建 Draft PR。
- [ ] 提交并推送 planning baseline，创建 Draft PR，写回 canonical URL与 publication evidence，再提交推送 metadata update；随后等待明确 human execution trigger。
- [ ] 收到执行授权后，用 `execute-exec-plan`验证 `PLAN-0004`已在默认分支 completed、PR仍为 Draft，进入 `active`并实现所有 milestones。
- [ ] 实现 domain query/page/doctor values与 focused ports/application operations。
- [ ] 实现 schema v2 FTS、v1-compatible reads、transaction-maintained index与 list/search/get repository queries。
- [ ] 实现 standalone backup、v1→v2 migration、read-only doctor snapshot与 FTS-only repair。
- [ ] 实现 CLI parsing、API-v2/human projections、focused/unit/integration/fault/scale tests和实际 binary smoke；同步产品、架构、设计与本 Plan证据。
- [ ] 在 implementation、acceptance、documentation和 retrospective全部 committed/pushed后运行 `gh pr ready`，验证 `isDraft: false`及 `headRefOid`等于 pushed implementation HEAD，再自动移入 `review`并推送 status commit。
- [ ] 收到明确 merge prompt后执行完成态 preflight、合并、更新 `main`并删除本地 delivery branch。

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

## Outcomes & Retrospective


当前只完成规划：选定 independently acceptable scope、取得搜索产品决定、建立 delivery branch并同步 Revision 2产品/技术/reference baseline。没有运行时代码、schema、command或用户 database被修改；`pull_request`仍为 `pending`。首次发布后补充两个 planning commits与 Draft PR证据；进入 review前必须记录最终行为、schema migration SHA/backup evidence、10,000-entry measurements、实际 smoke和与 Product Baseline的逐项对照。

## Review Conversation Log


No review conversation has been processed.

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

“纯文本词项 AND”指：保留原始 query供 API回显；按 pinned `is_white_space`切分；对每个词项产生 NFC raw与 `full_case_fold(raw).nfc()`，去除同一词项的重复alternative；adapter将 literal `a"b`编码为 FTS string `"a""b"`；同词项 alternatives用括号内 OR，不同词项仅以空格连接形成 implicit AND。没有词项时不调用 SQLite。Search结果不按 relevance，而在 count/page transaction中按 `canonical_source COLLATE BINARY`排序。

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

实现list/search/get read transactions。Absent list/search返回empty page且不创建roots；absent get返回`not_found`。V1 list/get从base rows工作；v1 search返回migration_required。V2 list/search先在同一transaction计算total，再在`offset >= total`时不做SQLite signed offset conversion并返回empty；否则用CTE选择canonical page并一次LEFT JOIN tags，按source/tag comparison order流式组装entries，避免每entry query。Search CTE只接收adapter从domain terms编码的bound MATCH string；用户raw query不得拼入SQL/FTS grammar。Get用exact selector和同样single-query tag assembly，missing返回`not_found`。

该里程碑的repository tests必须证明每个字段可搜索、raw/folded tag等价、operators保持literal、same-name sources共存、source-order先于pagination、adjacent pages不重叠、offset==total/`u64::MAX`为空、limit边界在SQL前失败、read transaction在concurrent writer下提供同一snapshot。验证v1 list/get/export继续工作而search/writes要求migration；v2 import/mutations每次保持FTS一致；corrupt/missing/extra FTS rows不会反向改base。

### Milestone 3：实现 backup、v1→v2 migration 与 doctor repair


在workspace dependency中给locked `rusqlite 0.40.2`增加`backup` feature，并加入 `sha2 = { version = "=0.11.0", default-features = false }`。不要升级其他locked crates。为 `SqliteLibraryRepository` 实现 `DatabaseMaintenance`，复用现有 XDG resolver、no-follow database identity、data-directory descriptor、global lock与durability helpers。

Default `inspect`在absent state返回healthy/empty `DoctorData`且不创建root。对existing database，以read-only identity-bound connection读取schema/base evidence；使用 `rusqlite::backup::Backup`复制到`:memory:` destination，在副本上执行完整v2 FTS content comparison与 `INSERT INTO library_fts(library_fts) VALUES('integrity-check')`。V1产生fixable `library_database_migration_required` finding并令 `database_writable=false`；healthy v2无finding且true；FTS-only drift产生fixable `library_fts_invalid`且false；newer/base corruption产生unfixable finding且false。Default outcome始终`observed`、`fix_requested=false`、每个finding `fixed=false`、actions empty。

`fix`先运行同一diagnosis。Absent/healthy/newer/unfixable base corruption不写state并返回`unchanged`；后两者保留finding。V1 migration取得durable lock、重新绑定database/directory/schema/state revision baseline，然后在 `data/backups/` 的held no-follow directory中创建restrictive random staging database与manifest。Online backup完成后关闭destination、sync file、流式计算size/SHA-256、重开read-only验证schema/base/integrity/foreign keys、写completed manifest、sync并以no-clobber relative rename发布pair、sync parent。任何不匹配或collision保留foreign entry并返回typed error；只清理仍与held descriptors匹配的owned staging。

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

Migration acceptance从P2/P3产生的真实v1 schema开始。Default doctor完全filesystem-inert；fix只在durable validated backup pair发布后开始live transaction；backup digest、manifest、SQLite integrity/foreign keys和source identity均可重验。每个failpoint留下old readable v1加完整/无backup，或完整v2且命令明确报告post-commit uncertainty；从不报告partial success。Schema v2的base rows、portable export和state revision与v1相同。Newer schema无write。FTS-only repair保持base database semantic records byte-equivalent或query-equivalent、state revision不变，并使content comparison与special integrity-check通过。

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

因此product在SQLite前显式拒绝empty query，而不是泄露engine error。External facts与来源保存在`docs/references/sqlite-fts5-library-search.md`。

执行时在此追加：planning commits、Draft PR URL/head、migration backup manifest示例、failpoint summary、focused/full validation、actual smoke输出、10,000-entry release timings、implementation SHA、ready verification与review-state commit。只保留证明行为所需的短摘录，不粘贴完整test logs。

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


2026-08-20：创建PLAN-0005 planning baseline。基于completed PLAN-0004与当前实现选择indexed offline Library reads；经用户决定把`SKL-LIB-004`提升到Revision 2并固定纯文本词项AND；固定schema v2 content-bearing FTS、explicit doctor migration/repair、v1 read compatibility、10,000-entry预算和dependency边界。创建Draft PR并写回URL后，在本节记录publication metadata变化；在获得后续明确execution prompt前不实现runtime行为。
