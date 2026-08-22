# Application and Persistence Design

Status: 部分实现的 0.1 CLI MVP design。`PLAN-0002` 实现 configuration-only `skilload-core` 路径；`PLAN-0003` 实现 SQLite-backed 可移植 Library import/export；`PLAN-0004` 复用 v1 schema 实现显式 Library metadata mutation；`PLAN-0005` 实现 schema v2 `library_fts` 派生索引、v1-compatible list/get/export 读取、`migration_required` 门控的 search/writes、SQLite online backup 的 standalone migration backup、v1→v2 transactional migration 与当前 durable database 的 doctor inspect/fix，其他 durable/domain system 仍为 planned。

This design supports the durable and application-layer portions of `SKL-LIB-*`, `SKL-TRUST-*`, `SKL-WSP-*`, `SKL-GLB-*`, `SKL-MGR-*`, `SKL-CACHE-*`, `SKL-OPS-*`, and `SKL-CLI-*`, within the boundaries in `ARCHITECTURE.md`.

## Behavior Traceability

* Repository and FTS ownership, metadata commands, and import/export implement `SKL-LIB-001` through `SKL-LIB-011`.
* Trust, global desired state, manager ownership, profiles, and known workspaces implement the persistence portions of `SKL-TRUST-001` through `SKL-TRUST-008`, `SKL-GLB-002` through `SKL-GLB-013`, `SKL-MGR-001` through `SKL-MGR-005`, and `SKL-WSP-023` through `SKL-WSP-027`.
* XDG layout, configuration, database migration/corruption handling, lazy creation, and locking implement `SKL-OPS-001` through `SKL-OPS-006`, `SKL-CACHE-008`, and `SKL-CACHE-009`.
* The command/query facade and presentation-neutral results support `SKL-CLI-004` through `SKL-CLI-012`; the CLI rendering contract remains owned by the CLI design.

## Goals

* Keep product rules independent of CLI, SQLite, GitHub, Git, filesystem, and Agent details.
* Give every command one application entry point and one transaction/result model.
* Keep durable, operational, and removable state visibly separate.
* Make schema upgrades, absent state, corruption, concurrency, and test isolation explicit.
* Leave a reusable application surface for future interfaces without building those interfaces in 0.1.

## Crate Composition

`crates/skilload-core` should expose an `Application` facade constructed from port implementations. The full 0.1 core will contain:

* `domain`: validated value types such as `CanonicalSource`, `RepositoryId`, `GitCommit`, `SkillPath`, `SkillName`, `Integrity`, `TrustState`, `ProfileId`, `DeploymentStatus`, and typed outcomes/errors.
* `application`: commands and queries. A command may mutate; a query is read-only by type and dependency contract.
* `ports`: traits for durable repositories, workspace documents, content acquisition, cache, Agent inspection, ownership, transaction journals, clocks, and identifiers.
* `adapters`: concrete SQLite, filesystem, GitHub HTTP, system Git, and Agent implementations.
* domain-focused modules (`library`, `trust`, `source`, `workspace`, `global`, `cache`, `agents`, `persistence`, `recovery`) that group rules and service implementations without changing dependency direction.

`crates/skilload-cli` owns `clap` command definitions, conversion into application requests, human rendering, JSON envelope serialization, and process exit status. It composes production adapters once at startup but does not expose them to command handlers.

当前实现包含 P1 的 `domain/configuration.rs`、`application/configuration.rs`、`ports/configuration.rs`、`adapters/xdg.rs`、`adapters/configuration.rs` 与 `error.rs`，以及 P2/P3 的 `domain/source.rs`、`domain/library.rs`、`domain/unicode_15_1.rs`、`application/library.rs`、`ports/library.rs`、`adapters/portable_library.rs`、`adapters/sqlite_library.rs` 和 local Unicode 15.1.0 build generator。`Application` 同时接收 focused configuration store、Library repository 与 transfer store；构造不打开 SQLite。P3 的八个 metadata application methods 在进入 port 前构造受限 domain change，repository 在同一既有 database lock、snapshot、transaction 和 sync protocol 中返回 changed/unchanged、portable entry 与 changed field。P2 对 absent Library 的 export/dry-run 返回内存空视图，真实 import 只在完整 input/schema/domain/conflict plan 后创建 data/state，并以 staging SQLite publish `data/skilload.db`；P3 对 absent database 或 entry 返回 `not_found`，不创建根。

Representative application interfaces should have this shape (names may be refined without changing the boundary):

    pub trait CommandHandler<C> {
        type Output;
        fn execute(&self, command: C) -> Result<Self::Output, AppError>;
    }

    pub trait QueryHandler<Q> {
        type Output;
        fn query(&self, query: Q) -> Result<Self::Output, AppError>;
    }

    pub struct Application { /* injected ports */ }

Application output is structured domain data such as `Changed`, `Unchanged`, `ConfirmationRequired`, or a typed error. It never contains preformatted terminal lines. Host paths remain a `NativePath` byte value through the application boundary; only the CLI adapter derives the terminal display and lossless `PathValue` JSON object required by `SKL-CLI-004`.

## XDG Layout

Use an XDG environment value only when it is nonempty and absolute. Treat an unset, empty, or relative XDG value as absent and use these fallbacks on both supported operating systems:

    config: $XDG_CONFIG_HOME/skilload
            or $HOME/.config/skilload
    data:   $XDG_DATA_HOME/skilload
            or $HOME/.local/share/skilload
    state:  $XDG_STATE_HOME/skilload
            or $HOME/.local/state/skilload
    cache:  $XDG_CACHE_HOME/skilload
            or $HOME/.cache/skilload

The expected files/subdirectories are:

    config/config.toml
    data/skilload.db
    data/backups/
    state/transactions/
    state/locks/
    state/workspaces/             # local manifest mirrors or indirection
    state/cache-index.json        # derived size/LRU observations
    state/debug/                  # only when explicitly enabled
    cache/objects/
    cache/quarantine/
    cache/staging/

The fallback requires a nonempty absolute `HOME`. If an XDG value needs fallback and `HOME` is missing, empty, or relative, return a typed `invalid_environment_path` before inspecting or creating any state path. Resolve each root once from environment input; never join a relative environment value to the current directory. Append `skilload`, normalize lexical `.`/`..` components to an absolute path without consulting the current directory, and resolve every existing path prefix through filesystem identity without creating a missing component. Compare the resulting application roots by path-component ancestry and existing-directory identity. All six pairs among config, data, state, and cache must be non-equal and neither ancestor nor descendant; reject equal, nested, or filesystem-aliased roots with `overlapping_state_roots` before opening any skilload-owned file. If an existing prefix is inaccessible, changes identity during resolution, or cannot be resolved without an unsafe symlink traversal, return `invalid_environment_path` instead of guessing. Mutations revalidate root identities with their final baseline so a symlink swap cannot redirect a staged write. Adapters create the minimum parent only when a successful mutation reaches its staging phase. Tests replace all roots and HOME with temporary directories.

The cache index is rebuildable operational metadata rather than durable product truth. It stores object size and a monotonic last-use sequence outside immutable cache objects; losing it affects eviction order only and never source identity, pins, Trust, or integrity.

## Durable SQLite Model

P2 使用随二进制 bundled 的 SQLite（FTS5 编译能力已验证）实现 v1 最小 schema：`schema_info`、`state_revision`、`library_entries` 与 `library_tags`。P3 在不迁移 schema 的前提下更新既有 alias/category/note 列或 tag row，并在 changed transaction 中恰好推进一次 `state_revision`；unchanged 不执行 SQL write 或 durability sync。

P5 已将持久 schema 升级到 v2：base 表（`schema_info`、`state_revision`、`library_entries`、`library_tags`）保持 P2 形状，另增普通 content-bearing `library_fts` FTS5 virtual table（`canonical_source UNINDEXED` 加八类 indexed text columns，tokenizer 固定 `unicode61 remove_diacritics 0`）。Import 与 metadata mutation 在同一 SQLite transaction 中通过共享 projection helper 显式维护该索引；migration 与 doctor repair 从 base rows 完整重建，不推进 `state_revision`。对 v1 database，list/get/export 继续只读可用，search 与所有 writes 返回 `migration_required`，直到显式 `doctor --fix` 在 durable lock 下完成 standalone backup 与 migration。P2/P3 不创建 Trust、global、profile、workspace、ownership、confirmation 或 journal 表；下列完整 ownership model 仍是后续交付的设计边界：

* `schema_info`: current schema version and migration metadata.
* `state_revision`: a monotonic semantic revision incremented by committed product-state mutations, not by confirmation-token bookkeeping or derived-index maintenance.
* `library_entries`: canonical source key with structured branch/tag/SHA ref intent, repository ID, derived metadata, alias/category/note, and metadata revision.
* `library_tags`: many-to-one tags with the Unicode-15.1 NFC display spelling and unique full-case-folded comparison key required by `SKL-LIB-008`；v1 schema 必须以唯一 `canonical_source → library_entries.canonical_source ON DELETE CASCADE` foreign key 维持关系，读路径验证该声明而不只依赖 `foreign_key_check`。
* current Library base schema 不允许 user trigger；base validation 发现任一 `sqlite_master` trigger 即返回 `database_corrupt`。这使 v1 migration 的备份、最终 baseline 与 `schema_info.version` update 都不会执行未验证的 schema-side write code。
* `library_fts`: derived FTS5 index over the fields required by `SKL-LIB-004`, including each tag's display spelling and comparison key.
* `trust_records`: exact source, repository ID, state, approval evidence revision, and revocation state. No credential or Skill bytes.
* `global_sources`: source intent and one shared commit/integrity/name pin.
* `global_targets`: active source-to-profile desired associations and status.
* `detached_global_targets`: non-active orphan warnings with the prior source, profile, exact path/link, pin, integrity, ownership evidence, and detach reason; these rows do not participate in update/pin or cache protection.
* `profiles`: opaque profile ID whose unique identity is `(Agent, canonical global Skill root)`, plus replaceable optional executable, validated HOME/Agent-root environment, Agent-configuration, compatibility-root, and environment-fingerprint observations.
* `manager_installs`: Agent/profile, embedded asset version/digest, target, marker, and observed ownership status.
* `known_workspaces`: unique random `workspace_instance_id`, canonical workspace path, manifest/exclude location, last committed lock digest, and latest transaction evidence.
* `workspace_targets`: unique `(canonical workspace, Agent, canonical project Skill root)` identity plus replaceable optional executable, validated HOME/Agent-root environment, Agent-configuration, compatibility-root, and environment-fingerprint observations.
* `owned_links`: exact target, expected link target, owner domain and workspace-target/profile identity, source/pin, and transaction revision.
* `confirmation_tokens`: token hash, canonical preview-plan digest, semantic state revision, optional workspace digest, expiry, and consumed state.
* `committed_transactions`: transaction IDs that act as recovery anchors after SQLite commit.

Use foreign keys and uniqueness constraints for exact namespace-preserving source, alias, tag comparison key within one Library entry, one `workspace_instance_id`, the `(Agent, canonical global Skill root)` profile identity, the `(canonical workspace, Agent, canonical project Skill root)` workspace-target identity, active target ownership, and one pin per global source. Updating environment observations for an existing global profile or workspace target never allocates a second owner for the same filesystem target. A journaled workspace rebind changes all canonical-workspace foreign keys for one instance in one SQLite transaction only after the deployment adapter proves the old/new filesystem plan. FTS is derived: triggers or an explicit transaction-maintained index keep it synchronized, and doctor may rebuild it from base rows.

Schema v2 的 `library_fts` 保存 `canonical_source UNINDEXED`，并分别索引 name、description、alias、tag display spellings、tag comparison keys、category、note 和 repository。它不使用 external-content triggers：Library adapter 在新增或 metadata mutation 的同一 SQLite transaction 中显式替换一个 entry 的派生 FTS row，migration 与 `doctor --fix` 则从 base rows 完整重建。Base metadata始终保留原始 UTF-8；共享 projection helper 对非 NFC 的 free-text column 保留原值并以 ASCII newline 追加 NFC representation，newline 是 tokenizer separator，因此 normalized query term可命中 composed/decomposed 等价值而不改变 list/get/export 输出。这样 tag 聚合只有一套 Rust/SQL 路径，FTS corruption 也可在不改变 `state_revision` 的情况下丢弃并重建。Tokenizer 固定为 bundled FTS5 的 `unicode61 remove_diacritics 0`；产品查询先按本地 Unicode 15.1.0 `White_Space` 分词，对每个词项生成 NFC 原文与完整默认大小写折叠后的 NFC alternative，再逐项做 FTS5 string quoting，同词项 alternatives 以括号内 OR 组成一个 group，不同词项的 group 之间以显式 `AND` 连接（FTS5 不支持括号表达式与后续 phrase/group 间的 implicit AND）。SQLite 的 `OR`、`NOT`、`NEAR`、prefix 或 column-filter grammar 不成为产品接口。
任何 import 或 metadata mutation 在写 product row 前还必须在其已可写的 transaction 上执行 FTS5 special `integrity-check`；visible projection 与普通 `MATCH` 均可读但 special check 失败的 shadow drift 必须以 `library_fts_invalid` 拒绝 mutation，不能让 unrelated write 报告成功。
Schema v2 `library search` 也必须在任何 `MATCH` count 或 page query 前，以 read-only snapshot 的 bounded in-memory backup 运行同一 FTS5 special integrity check；即使 FTS content projection 可读、inverted-index block 让 MATCH 静默返回零，也必须返回 `library_fts_invalid`。page count 与 page size 的 checked product 超过 536,870,912 bytes 时不复制 generation：default doctor报告不可离线验证的 error finding `library_fts_diagnostic_snapshot_too_large`，`doctor --fix` 不写入也不把它误报为已修复；search继续以既有 `library_fts_invalid` 拒绝未验证 index。这保留 default doctor/search 对 live XDG state 的 filesystem-inert 约束并避免无界内存分配。

Agent-root environment validation is independent of the XDG algorithm. A present `CLAUDE_CONFIG_DIR` or `CODEX_HOME`, and `HOME` whenever an adapter needs it, must be nonempty and absolute before root construction; invalid explicit values do not fall back and are never joined to CWD. Persist only observations that passed this rule. Removal-only plans may retain a `NULL` executable observation while still requiring a resolved, accessible root and exact ownership.

SQLite transactions cover all database changes for one application mutation. Filesystem changes remain journaled separately; the database's committed transaction ID determines whether recovery rolls external work forward or back.

Confirmation-token creation, consumption, and expiry cleanup use transactions but do not increment `state_revision`; otherwise issuing a token would invalidate its own baseline. The operation performed with a valid token consumes it in the same transaction that applies any product-state change, and only that product-state change increments the semantic revision.

## Database Opening and Migration

Queries against absent state use an in-memory empty repository view and do not create `skilload.db`. A mutating command, including explicit `doctor --fix`, opens/creates the database only after input validation reaches a persistent stage.

On open:

1. 验证文件类型、可用时的受限 ownership/permissions、SQLite header、integrity status 和 schema version。对已存在 database，先在 root-resolution anchors 前后验证 `data/skilload`，以 no-follow directory descriptor 持有该目录；read-only gate 只通过该 descriptor 的 `openat(..., O_RDONLY|O_NOFOLLOW|O_NONBLOCK)` 打开单组件 `skilload.db`，以 `fstat` 证明 regular file、以 `statat` 证明仍是 held directory entry，再从同一 file descriptor 检查 header。SQLite read-only connection 从该 descriptor 的 `/dev/fd/<fd>` 打开；任意 SQL 前后均重验 held data directory、entry 与原 pathname identity，因此 root directory、main-file、symlink、FIFO 或 ABA replacement 都不能使 gate 与实际读取的 generation 分离。
   `schema_info.version` 只有 1 到 API-v1 lossless unsigned 上限之间的整数才是可读 schema generation；零、负值或超出上限都是 base corruption，而不是 unknown newer schema。
   descriptor-bound read-only connection 必须在同一 transaction 中先执行最小 `PRAGMA schema_version` read 来取得 SQLite SHARED snapshot，再通过 held data-directory descriptor 重新盘点 `-journal`、`-wal`、`-shm`，之后才读取 Library schema/data。第二次盘点发现 companion 时返回 `database_corrupt`；companion 在 snapshot 后才出现时不能在该 transaction 完成前以 EXCLUSIVE 更新 main file，因此不能改变返回的 generation。
   对已存在 `data/skilload` 内 `skilload.db` 不存在的共用 probe 也先持有并验证该目录的 no-follow descriptor，再以 descriptor-relative `statat(..., SYMLINK_NOFOLLOW)` 检查 main file 和 `-journal`/`-wal`/`-shm` siblings；probe 前后及返回结果前后都重验 held directory 与已解析 roots，因此临时换入空目录再恢复的 ABA 不能使 list/search/get/export/default doctor 采纳 empty 或 `not_found` generation。若 `data/skilload` 本身不存在，probe 不创建它，并在返回 absent 前后重验 roots。
2. `schema_info.version` 指向 unknown newer generation 时，必须在解释任何 v1/v2 base table 之前返回 `schema_newer`。default doctor 以 `library_schema_newer` finding 拒绝 fix，所有 mutation 拒绝写入；portable export 仍是独立的安全 projection，仅在其自身 entries/tags proof 成功时可用。
3. 在 forward migration 前，先以 current source 的 checked `page_count × page_size` 证明 standalone backup不超过 268,435,456 bytes，再通过 SQLite backup API 在 `data/backups/` 创建 standalone durable backup，而不是逐字节复制 live WAL database。backup每个 512-page step 后重验其 reported page count；staging descriptor length在 SHA-256 前再次受同一上限约束。任一步超过上限返回 `migration_backup_too_large`，不 hash、不发布 pair、不写 live schema。名称包含 source schema、target schema 与 UTC creation time；写入 sibling manifest，包含 schema versions、byte size、SHA-256 digest、source database file identity 和 completed marker。
   migration 的每次 v1 baseline validation 还要求 exact non-FTS `sqlite_master` inventory：只允许四个 fixed base tables、v2 的 owned FTS virtual/shadow names和 SQLite internal objects；任何 extra table、index、view 或 trigger 都是 `database_corrupt`。第一次 proof发生在发布 standalone backup 前，第二次发生在最终 version update 的同一 SQLite transaction 内，因此异常 schema object不能把已验证 Library rows 改写后误报 migration success。已知 FTS name的 malformed/missing tolerance仍只属于 schema v2 derived repair，不放宽 v1 migration inventory。
4. 验证 standalone backup，在一个 SQLite transaction 中执行 migration、更新 `schema_info`，并运行 integrity 与 foreign-key checks。当前 adapter 保留每个 complete validated backup pair；它不自动 prune，因为可移植的 unlink 无法将待删除 pathname 原子绑定到已验证 inode，任何未来 cleanup 必须先提供 ownership-bound deletion。

`database_corrupt` 的 backup inventory 不把 filename、timestamp 或 digest 单独视为证明。它从 held backup-directory descriptor 以 `openat(..., O_NOFOLLOW)` 打开 pair，最多读取 4 KiB private manifest，要求 format version、source schema 1、current target schema、`complete`、size、SHA-256 与 directory-entry identity 一致；随后以 held standalone database descriptor 在 SQLite SHARED snapshot 中验证 header、schema v1 和完整 base rows，并在 snapshot 前后盘点该 `.db` 的 `-journal`、`-wal`、`-shm` companion。任何 candidate 的 no-follow open、metadata、manifest read、SQLite snapshot、hash或entry revalidation operational failure必须作为 typed `XDG_DATA_HOME` I/O error传播；只有 genuinely absent、symlink/nonregular、格式不兼容、corrupt或带 companion的 content candidate才是 `false` 并从 inventory排除。`Dir::read_from` 初始化或逐项读取失败同样传播，只有 genuinely absent `backups` child 才表示空 inventory。
候选 standalone database 的 held descriptor长度必须不超过 268,435,456 bytes，且该判断位于 SQLite open、snapshot validation 与 SHA-256 之前；这把 hostile sparse recovery file 的诊断读取限制在现有 67,108,864-byte portable document 资源模型的四倍预算内。
5. Never automatically downgrade or replace a corrupt database.

`library export` 把每个已发布 migration backup pair 的 `.db` 与 `.manifest.json` entry 视为 protected output target，并用 pathname、resolved-path 与 inode checks 拒绝直接路径或 hard-link alias。为使 inventory 不受同账号 ABA pathname swap 影响，adapter 在 root-revalidated `data/skilload` 与 `backups` 的 held no-follow descriptors 上相对枚举 pair，并在返回前重验两个目录 identity；每个 regular pair entry 的 held identity 随 inventory 保留，child/open/enumeration/revalidation failure 一律 fail closed。因此后续 portable export 不能替换 recovery asset。

FTS-only corruption 可以在 `doctor --fix` 下从 base rows drop/recreate derived index structures。Base-row corruption 继续阻止写入；其 typed diagnostic 使用 `docs/product-specs/api-v2.md` 的 `DatabaseCorruptDetails`，列出 readable exports 与 complete backup manifests，并指向 `docs/product-specs/database-recovery.md` 的 `database-corruption-v1` procedure；没有隐藏 CLI command 执行它。Schema v2 search 已完成 base 和 FTS content-row validation 后，`MATCH` 的 `SQLITE_CORRUPT`/`SQLITE_NOTADB` 归为可由 doctor repair 的 `library_fts_invalid`，而不是 base `database_corrupt`。
即使 FTS content-row projection 与 base rows 相等，search仍在 count/page 前执行 special integrity check；因此 zeroed or logically damaged inverted-index block不能返回 successful empty result。仅当初始诊断已证明实际 FTS drift时，`doctor --fix` 持锁后的健康重诊才直接在 writable connection运行该检查，以避免 concurrent repair的重复 rebuild；snapshot-budget resource finding不进入此路径。
`library export` 的 recoverable projection 只依赖完整的 `library_entries`/`library_tags` schema、逐表 integrity、foreign-key 与 domain-document validation；它不依赖已损坏的 `schema_info` 或 `state_revision`，所以 `database_corrupt` diagnostics 仅在该 projection 真可读时列出 `library.export`。human error renderer 与 API-v2 都显示已验证 backup paths 和该 recovery export identifier。
`validate_derived_database` 只把已完成 base validation 后的 derived schema/content mismatch 与 SQLite `SQLITE_CORRUPT`/`SQLITE_NOTADB` 归为 `library_fts_invalid`。SQLite busy、lock、I/O、memory 和其他 operational failure 必须保持原有 typed error，default doctor 不得在未证明 drift 时把它们报告为可修复 finding。

若 `library_fts` virtual-table row 缺失、malformed，或任一 shadow row 缺失/损坏，adapter 将它视为可修复 derived corruption：第一笔 writable transaction 只经 `writable_schema` 删除 FTS/shadow schema rows、`RESET` reload schema并提升 schema cookie，然后先 commit。仍持有 global durable-database lock 的 connection 在没有 open transaction 时运行 `VACUUM` 回收已分离 b-tree pages，并重验 database identity；第二笔 transaction 才 drop/create 固定 virtual table、从已验证 base rows填充并运行 special integrity check。这样 interrupted repair 保持“FTS missing/invalid、可重试”状态，不会把带 unreachable pages 的重建 FTS误报 healthy；`canonical_source` 而非 SQLite rowid 是产品身份，VACUUM 的可能 rowid 重新分配不改变 Library 行为或 `state_revision`。
若仅 `library_fts` 的 virtual-table SQL text malformed，connection-local `PRAGMA writable_schema=ON` 仍允许 base-only validation、list/get/export 与 default doctor 读取经验证的 base rows；search 与 write 仍返回 `library_fts_invalid`。`VACUUM` 只能在无 open transaction 时运行，失败保持 FTS invalid 以供后续 `doctor --fix` 重试，绝不改 base rows 或 `state_revision`。

Schema v2 的 read 兼容性与 doctor 语义已由 `PLAN-0005` 实现：所有对已存在 database 的读操作先经 descriptor-bound pre-open generation gate；main-file header 的 journal-mode bytes 非 (1,1) 或存在 `-wal`/`-shm` sibling 时按 `database_corrupt` 拒绝，不打开 SQLite。gate 先持有并重验 resolved `data/skilload` directory，再以其 `openat` 的 `O_NOFOLLOW|O_NONBLOCK` regular-file descriptor 读取同一 generation；因此 directory replacement 返回 `database_identity_drift`，FIFO 不会等待 writer，main-file replacement 不会向 replacement 创建 sidecar。默认 `doctor` 以 read-only source connection 做 online backup 到 `:memory:`，在副本上运行需要 writable connection 的 FTS5 `integrity-check`，live filesystem 保持不变；`doctor --fix` 只对已证明 base rows 完整的 v1 migration 或 FTS-only drift 执行 action。v1 migration 在 `data/backups/` 发布 standalone database 与 completed sibling manifest（SHA-256、字节数、source identity、epoch-nanosecond 时间…
默认 doctor 与 search 的 read-only special-check copy使用 536,870,912-byte checked page budget；超限 generation保持 base rows可读，search以 `library_fts_invalid` 拒绝未验证 FTS，default doctor以不可 fix 的 `library_fts_diagnostic_snapshot_too_large` error finding说明资源边界，`doctor --fix` 返回 unchanged且不改 live database。这避免外部增大的 FTS shadow/freelist在 default diagnostic中按数据库全量分配进程内存或无限重复 no-op repair。
DELETE-mode rollback journal 同样是 main file generation 的成员：任何 `skilload.db-journal`、`-wal` 或 `-shm` sibling 都使 generation non-standalone。因为 descriptor-bound `/dev/fd/<fd>` SQLite open 不能安全重新关联 pathname sibling，read-only list/get/search/export/default-doctor 必须在 pre-open inventory 与 SHARED snapshot-bound inventory 中拒绝已观察到的 companion 并返回 `database_corrupt`；snapshot 后出现的 active writer 不能改变仍持有的 read generation，所有 member 保持不变。

The restore procedure treats a database plus its WAL/SHM as one generation and never copies a live WAL database as a backup. Candidate validation uses isolated pairwise-disjoint XDG roots and the same binary's read-only doctor; a supported older candidate is migrated only on a second disposable copy through `doctor --fix` and the ordinary backed-up transaction above. Only the resulting standalone current-schema database is staged in the live data directory. File replacement is same-directory rename plus file/parent fsync; the prior database/WAL/SHM set remains an external rollback generation until post-restore doctor and domain reads pass. Journals resume only when their recorded database transaction anchor matches the restored generation. SQLite's backup, WAL-generation, integrity-check, and salvage constraints are archived in [`../references/sqlite-backup-and-corruption-recovery.md`](../references/sqlite-backup-and-corruption-recovery.md).

An explicit reset is detected only as an absent live database after the operator has moved the corrupt database/WAL/SHM set out of place. Lazy creation then follows the ordinary empty-state path, but no adapter may infer ownership from surviving manifests, links, manager copies, or cache objects. Library import is the only portable database repopulation; Trust and every deployment/ownership domain require explicit normal commands and validation. Recovery tests execute restore success, rollback, no-valid-backup, and reset/re-establishment paths from the normative procedure.

## Configuration

`config.toml` is a small strict document:

    version = 1
    cache_limit_bytes = 536870912

    [agents.claude]
    executable = "/opt/claude/bin/claude"

    [agents.codex]
    executable = "/opt/codex/bin/codex"

Beyond required `version`, only `cache_limit_bytes`, `agents.claude.executable`, and `agents.codex.executable` are permitted; credentials, Trust, desired deployments, dynamic roots, command lines, and arbitrary Agent tables are not. `version` maps only to read-only `schema_version` metadata, never a set/unset key. The absent/unset cache value resolves to 536,870,912 bytes. `config set cache_limit_bytes <BYTES>` accepts a positive signed-64-bit TOML integer, converts it to checked `u64`, returns it through JSON as `DecimalU64`, and `config unset cache_limit_bytes` returns to that default. Each Agent executable setter accepts one nonempty valid-UTF-8 absolute path, performs CWD-independent lexical normalization without probing it, and stores it under the matching table; unset removes the key and restores no-override lookup of the fixed basename. Get/list return `{ configured: false, value: null, default_command: "claude"|"codex" }` for an absent Agent key and a native-path value plus `configured: true` when set; the CLI serializes that value as `PathValue`. Parsing uses a structured TOML decoder with unknown-field denial and validated types/ranges. Read commands operate on defaults when the file is absent. `config set` stages a complete canonical document and atomically renames it; it does not preserve comments.

No automatic config migration exists in 0.1. An unsupported version is an error until a future product behavior and explicit migration command are approved.

## Repository and Unit-of-Work Ports

Application services need focused ports rather than a generic SQL handle:

    trait LibraryRepository { /* get/search/add/metadata/export/import */ }
    trait TrustRepository { /* exact lookup/add/revoke/migrate */ }
    trait DeploymentRepository { /* profiles, desired state, ownership */ }
    trait WorkspaceStore { /* read/stage config-lock pair and manifest */ }
    trait TransactionJournal { /* prepare, phase, complete, recover */ }
    trait ProcessLock { /* bounded exclusive/shared acquisition */ }

A mutating application service asks a `UnitOfWorkFactory` for one durable transaction after external staging and baseline revalidation. Port methods take/return domain values, not SQL rows or unvalidated strings.

`LibraryRepository::list` and `search` accept a validated `LibraryPage { limit: u16, offset: u64 }`, where limit is already within 1-1,000. In one read transaction, compute the complete count and canonical-source order, return empty without an SQL offset conversion when `offset >= total`, and otherwise bind only a checked signed-64-bit offset; SQLite cannot contain more rows than that bound, so an offset above it is necessarily beyond total. Return the requested offset, limit, array length, and complete count as the API-v1 page metadata. This preserves one snapshot under concurrent writers and prevents platform `usize` or SQLite conversions from truncating a valid unsigned offset.

`WorkspaceStore` performs the byte-capped, non-expanding YAML event validation from `SKL-WSP-004` and `SKL-WSP-005` before schema deserialization. It returns measured counters with typed errors and never exposes a partially built config/lock model to application services. Its manifest mutation methods require a sanitized `GitRepositoryView` containing canonical worktree, per-worktree Git-directory, effective-index, and exec-path identities plus a fresh bound literal tracked-state observation in both their input baseline and commit callback; `tracked_local_manifest` or resource drift preserves the journal and all ownership rows rather than treating the manifest as an ordinary writable file.

The `source migrate` unit of work may update only `library_entries`/tags/FTS, `trust_records`, `global_sources`, and their global target/global-domain ownership relationships. Queries may join `known_workspaces` and workspace-domain `owned_links` to report impact, but the transaction denies workspace-table and workspace-domain ownership writes. The separately journaled `workspace migrate-source` owns portable config/lock rewrites and matching workspace ownership rows, so its preflight always compares the original source spelling on both sides.

## Concurrency

Use a global durable-database mutation lock plus a canonical-path workspace lock when relevant. Acquire locks in a single documented order: global/database first, then lexicographically sorted workspace/profile target locks. Bounded acquisition returns typed `busy` with lock domain and elapsed limit.

持久 database lock 的 pathname 是所有 contenders 的稳定协调身份；一旦其他进程可能已经打开它，失败清理不得 unlink 或重建该路径。Revision 5 的首次 import 在 `COMMIT` 前失败必须返回 error 且不得报告 state absence：它只清理仍与 held descriptor 一致的 staging entry 或未完成 live link，`database.lock`、directory、sidecar 或其他 provenance/identity 不能证明的 residual 必须保留。这避免等待者落在旧 inode 而后续 importer 锁定新 inode，也避免用 pathname observation 误删 foreign state。

Network and Git acquisition may occur before the final lock to reduce contention. The stage records database revision, workspace config/lock digest, Trust revision, global source revision, and target ownership observations. After reacquiring locks, the command revalidates every baseline. Drift restarts safe resolution or returns a typed stale result; it never commits on stale assumptions.

P2 的首次导入若在取得锁前按数据库不存在进行规划、但在取得全局持久数据库锁后发现另一位持锁者已经发布 `skilload.db`，MUST 在同一已持有锁内以原始已验证文档重读持久条目并重新生成既有数据库计划；它不得把这一正常串行化结果误报为 identity drift，也不得重入同一锁。

SQLite busy timeout is a second line of defense, not the primary product lock. No application service holds a filesystem lock while waiting indefinitely on network input or human confirmation.

P2/P3 将每个 SQLite connection 的 busy timeout 固定为与 global durable-database lock 相同的两秒；SQLite 返回 `DatabaseBusy` 或 `DatabaseLocked` 时必须投影为 `BusyDetails { lock_domain: "database", waited_ms: 2000 }`，不能误报为损坏或无效状态。

## Import and Export

Library export is built from domain records, sorted by canonical source, and serialized as a versioned portable JSON document. It contains no database row IDs or local timestamps needed only for operations.

`LibraryImportReader` 只能在 no-follow、nonblocking 打开的 descriptor 经 `fstat` 证明为 regular file，且与预检 path identity 一致后才读取 input；symlink、directory、FIFO、socket、device 或 identity drift 在 scanner 前返回 typed validation error。随后它执行一次没有 schema model 或 `ImportPlan` 的 streaming JSON event/token pass：计数至多 67,108,864 input bytes、10,000 entry objects、1,000,000 total values、八层 container、每个 string 1,048,576 UTF-8 bytes 和每个 number 128 bytes，同时拒绝 malformed JSON 与 duplicate object keys。它在第一个 exceeded dimension 停止并返回 measured/allowed values。只有成功 pass 才可反序列化 versioned schema，并将 `SKL-LIB-008` 的 alias/category/note/tag limits 验证为 `ImportPlan`；同一 batch 的 canonical source 重复作为 `internal_duplicate` conflict，而不是让 SQLite 主键或输入顺序选择 metadata。

该 pass 直接从 held descriptor 的有界 buffered reader 增量推进；它只在 scanner 尚未报错时累积后续 `serde_json` 所需 bytes，并在任何失败时丢弃该缓冲。命中 string、number、depth、value、entry 或全文件 byte ceiling 时立即停止 scanner，不会先 materialize 完整输入。

由于 `LibraryExportData` 是当前唯一传输文档，P2 在导入计划阶段把既有条目与拟新增条目组合为完整确定性文档，并以与导出完全相同的 domain checks/JSON encoder 验证至多 10,000 entries 与 67,108,864 bytes。超过任一上限时，在预演结果或 SQLite mutation 前返回 `validation_failed` / `library_portable_document_entries` 或 `library_portable_document_bytes`；导出也在创建暂存文件前使用同一上限，因此有效持久状态不会产生本二进制拒绝读取的输出。

number scanner 在每次推进 integer、fraction 或 exponent byte 时检查 128-byte ceiling，因此第 129 byte 即停止；它不为长 token 先遍历到末尾再计算长度。

对于原本不存在的 `data/skilload.db`，持久 import 在完成 input/schema/domain/conflict planning 后，在同一 data directory 建立 restrictive staging database；只有 schema 和 SQLite `COMMIT` 成功、SQLite connection 已关闭且根重新验证后才尝试发布 live database。发布不得提前以 live `skilload.db` 名称创建空 guard：它在已验证 data-directory descriptor 内以当前 staging basename 调用 no-clobber `linkat`。无并发 source replacement 时，link 成功令 live name 首次出现即为完整 committed generation；竞争方已创建 target 时 link 失败为 typed identity drift，绝不覆盖其文件。每个 post-link hook 与最终身份检查都必须比较 live entry 和 held staging inode；若被替换，未知 target 保留并返回 error，不报告 success。

Revision 5 不把 create 后 pathname metadata、matching basename 或 first-observed no-follow FD 当成 directory 或 SQLite `-journal`/`-wal`/`-shm` 的 ownership proof。`mkdir` 不返回可移植 held descriptor，sidecar observation 也不能证明 SQLite 创建该 inode；因此任何 first-import error 均保留这些目录与 sidecar，而不是清理后宣称 data/state absence。仍与 held staging FD 一致的原 staging entry 或尚未完成 live link 可以在 Drop 中清理；错误结果必须明确保持 error，而不声称 Library 未改变。

暂存 SQLite connection 与既有 database 的 read-write connection 都必须在任意 SQL 前验证 SQLite 实际持有的 main-file inode：随机 staging basename 与既有 write path 以 no-follow、无 create 的 pathname SQLite 打开标志打开，随后执行 bundled SQLite main-file `SQLITE_FCNTL_HAS_MOVED` file control，并重验 held data-directory entry 后才配置 connection 或执行 SQL。read-only existing connection 改从已验证 file descriptor 的 `/dev/fd/<fd>` 打开；Linux 会把该 temporary source name 报给 `HAS_MOVED`，所以 read path 以 held descriptor、held data directory、relative entry 与原 pathname 的连续重验替代该 file control。该调用仍是 core crate 唯一局部审计的 FFI 例外；若同账户进程将 basename 或 data directory 替换为 symlink、FIFO 或其他 inode（包括仅在 SQLite open 时的 ABA），adapter 返回 identity drift，保留未知 replacement，绝不向其 target 读写。

所有 publish hook 或最终验证完成后，first import 必须再次比较 held staging FD、data-directory descriptor 与 live `skilload.db` entry，才可删除原 staging link 并将它标为 published。link 后任一不匹配都返回 typed drift error，且未完成的 staging Drop 只移除仍为 held inode 的 live link；外部 replacement 必须保留。发布后完成 parent/data-directory 与新建目录 sync 后，first import 必须最后一次比较 live `skilload.db` entry 与 held staging FD，任何 drift 都返回 typed error 而不报告 `changed`。

`linkat` 的 target-exists failure 仍保留 foreign database，但其 source 参数是 staging pathname 而非 held file descriptor；在 macOS/Linux 当前安全接口中，同账号若在最终 verify 后替换该 source，adapter 只能在 link 后检测 drift 并保留 foreign target，不能证明 publication source 一定是 held inode。Revision 5 将这一情况纳入保守 recovery contract：它返回 error、不报告 success 或 state absence，且不会以 cleanup 删除 target；正常成功才按 held identity 删除原 staging link。

Library export 在创建 staging 文件前比较 no-follow output target 与有效 Library database generation（database、WAL、SHM）及 database lock，拒绝任何碰撞。其他 target 使用同目录 staging、file sync 与 parent-directory sync；父目录以 no-follow descriptor 打开并绑定 device/inode。export 对既有 regular output 记录 identity，以 held publication link 和 `RenameFlags::EXCHANGE` 做可逆替换；对 absent output 不在 requested name 创建 zero-byte guard，而是在 hidden publication link 已验证为 held staging inode 后以 descriptor-relative `RenameFlags::NOREPLACE` 发布。该 no-clobber rename 使完整 document ready 前 requested path 保持 absent；target collision 保留 foreign entry。parent sync 后仍须比较 held staging FD 与 output entry。若检测到父目录、publication link 或 output identity drift，命令返回错误而不报告成功，且不删除未知 replacement。rename 前失败保留旧 target 或无 target并清理已证明所有权的 staging，rename 后 parent sync failure 不假称旧 target 尚在。
同一 protected-target inventory 还包含所有已发布 `data/backups/skilload-db-v1-to-v2-*.db` 及 matching `.manifest.json` entries；inventory 在 held `data/skilload`/`backups` descriptors 上相对读取并在返回前重验根与子目录 identity，regular entry 的 held inode 用于拒绝 same-inode hard-link alias。任何无法完整取得或重验此 inventory 的情形都拒绝 export，确保 forward migration 的 standalone recovery evidence 不会被 portable document 覆盖。
对既有 target 的成功 exchange，P2 不会 unlink 随机 publication entry 中的旧 output。POSIX/Darwin/Linux 没有“仅当 pathname 仍绑定 held inode 才 unlink”的原语；先 identity check 再 `unlinkat` 的同账号替换窗口会删除未知 replacement。保留该 entry 虽留下旧 document，但保持请求 output 的新 document 与任何后来出现的 foreign entry 都不被误删。


有效 generation 的 protected members 同时包括 SQLite DELETE-mode 的 `skilload.db-journal`；export 在 staging 前以与 database/WAL/SHM/lock 相同的 identity guard 拒绝它，避免干预活动 writer 的 rollback recovery。

任何 publish 前的导出失败都必须按持有暂存 inode 分别清理原暂存名称和随机 publication 名称。existing-target exchange 后 identity mismatch 必须先尝试反向交换；absent-target no-clobber publish 后的 identity mismatch 只保留未知 replacement 并返回 error。无法证明所有权的 publication 或 output replacement 一律保留。

既有 database import 在 transaction commit 后，以 held data-directory descriptor 对相对 `skilload.db` 执行 no-follow open、FD sync、entry identity revalidation、parent descriptor sync 和最终 revalidation；任一 sync 后的 generation 或目录替换必须返回 drift error，绝不报告 `changed`。

## Testing Consequences

P2 默认测试使用 temporary XDG/HOME roots、bundled SQLite 与 generated input；它们覆盖 Unicode 15.1.0 tag normalization、canonical source/positive portable evidence、六种 non-model import ceiling 与 API-v2 `library_input_limit_exceeded`、combined 10,000-entry/byte transfer closure、duplicate keys、nonregular/identity-drift input、first-import conservative cleanup、foreign-sidecar preservation 与 orphaned database sidecar rejection、pre-open 与 read-only SQLite ABA inode replacement、直接 live database link 和已发布 database replacement、SQLite transaction rollback、post-commit error、既有数据库 no-follow/identity race、缺失 schema 列、foreign-key parent-key mismatch 与超出 API-v2 `UInt` 的损坏诊断、state revision 溢出拒绝、deterministic export、database/WAL/SHM/lock collision、held parent descriptor 的 rename race、publication link replacement、absent output 在 no-clobber publish 前保持 absent，以及 symlinked parent 后 `..` 的 native path resolution。

发布 regression 必须证明 first import 在 live name 出现时已经是 committed SQLite generation，并在 link 后、final identity check 后的 database replacement 中保留 foreign target；source-path replacement 前的 held-FD provenance 不得被误报为已证明。sidecar 和 directory tests 必须证明 persistence failure 保留无法证明 provenance 的 entries，并且不再断言 data/state roots absence；只有仍匹配 held staging identity 的 entry 才可自动清理。

## Decisions Deferred Beyond the Configuration Foundation

`PLAN-0002` 固定 Rust toolchain、`clap`、`serde`、TOML、error、filesystem-staging、JSON 与 test dependencies。`PLAN-0003` 将 `rusqlite 0.40.2`（`default-features = false`、`bundled`）、`unicode-normalization =0.1.23`、`libc =0.2.189` 与 `rustix =1.1.4`（`fs`）锁入 workspace，并固定 P2 SQL names 与 local Unicode 15.1.0 数据。`PLAN-0005` 为同一 `rusqlite 0.40.2` 启用 `backup` feature，并加入 `sha2 =0.11.0`（`default-features = false`）以流式计算 migration manifest 的 SHA-256；它没有增加 HTTP、异步 runtime、通用 migration framework 或第二个 database owner。FTS schema v2、v1→v2 backup/migration 与当前 durable database 的 doctor repair 已由该 Plan 交付；HTTP 与其余 durable-domain dependency decisions 继续由后续交付决定。
