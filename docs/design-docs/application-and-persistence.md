# Application and Persistence Design

Status: partially implemented design for the 0.1 CLI MVP. `PLAN-0002` implements the configuration-only `skilload-core` path; SQLite and every durable/domain system beyond configuration remain planned.

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

The current P1 implementation contains `domain/configuration.rs`, `application/configuration.rs`, `ports/configuration.rs`, `adapters/xdg.rs`, `adapters/configuration.rs`, and `error.rs`. Its `Application` facade owns the four configuration use cases and receives a focused configuration store; the store resolves all four XDG roots, reads absent configuration as an in-memory default, uses a bounded lock only for real writes, and atomically replaces `config.toml`. It deliberately creates no SQLite database or dormant domain module.

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

Use SQLite compiled into the binary with FTS5 enabled once the durable-domain delivery begins. The exact SQL remains a later delivery decision, but ownership is fixed:

* `schema_info`: current schema version and migration metadata.
* `state_revision`: a monotonic semantic revision incremented by committed product-state mutations, not by confirmation-token bookkeeping or derived-index maintenance.
* `library_entries`: canonical source key with structured branch/tag/SHA ref intent, repository ID, derived metadata, alias/category/note, and metadata revision.
* `library_tags`: many-to-one tags with the Unicode-15.1 NFC display spelling and unique full-case-folded comparison key required by `SKL-LIB-008`.
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

Agent-root environment validation is independent of the XDG algorithm. A present `CLAUDE_CONFIG_DIR` or `CODEX_HOME`, and `HOME` whenever an adapter needs it, must be nonempty and absolute before root construction; invalid explicit values do not fall back and are never joined to CWD. Persist only observations that passed this rule. Removal-only plans may retain a `NULL` executable observation while still requiring a resolved, accessible root and exact ownership.

SQLite transactions cover all database changes for one application mutation. Filesystem changes remain journaled separately; the database's committed transaction ID determines whether recovery rolls external work forward or back.

Confirmation-token creation, consumption, and expiry cleanup use transactions but do not increment `state_revision`; otherwise issuing a token would invalidate its own baseline. The operation performed with a valid token consumes it in the same transaction that applies any product-state change, and only that product-state change increments the semantic revision.

## Database Opening and Migration

Queries against absent state use an in-memory empty repository view and do not create `skilload.db`. A mutating command, including explicit `doctor --fix`, opens/creates the database only after input validation reaches a persistent stage.

On open:

1. Verify file type, restrictive ownership/permissions where available, SQLite header, integrity status, and schema version.
2. Refuse writes for an unknown newer schema.
3. Before a forward migration, create a standalone durable backup in `data/backups/` using SQLite's backup API rather than copying a live WAL database byte-for-byte. Name it with source schema, target schema, and UTC creation time; write a sibling manifest containing schema versions, byte size, SHA-256 digest, source database file identity, and completed marker.
4. Validate the standalone backup, apply the migration in one SQLite transaction, update `schema_info`, and run integrity and foreign-key checks. Only after the migrated database is durable may pruning retain the newest three complete validated backups while never deleting the only backup of the immediately preceding schema generation.
5. Never automatically downgrade or replace a corrupt database.

FTS-only corruption can be repaired by dropping/recreating derived index structures from base rows under `doctor --fix`. Base-row corruption stays a write blocker. Its typed diagnostic uses `DatabaseCorruptDetails` from `docs/product-specs/api-v1.md`, enumerates readable exports and complete backup manifests, and directs the operator to `docs/product-specs/database-recovery.md` procedure `database-corruption-v1`; no hidden CLI command performs it.

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

Network and Git acquisition may occur before the final lock to reduce contention. The stage records database revision, workspace config/lock digest, Trust revision, global source revision, and target ownership observations. After reacquiring locks, the command revalidates every baseline. Drift restarts safe resolution or returns a typed stale result; it never commits on stale assumptions.

SQLite busy timeout is a second line of defense, not the primary product lock. No application service holds a filesystem lock while waiting indefinitely on network input or human confirmation.

## Import and Export

Library export is built from domain records, sorted by canonical source, and serialized as a versioned portable JSON document. It contains no database row IDs or local timestamps needed only for operations.

`LibraryImportReader` 只能在 no-follow、nonblocking 打开的 descriptor 经 `fstat` 证明为 regular file，且与预检 path identity 一致后才读取 input；symlink、directory、FIFO、socket、device 或 identity drift 在 scanner 前返回 typed validation error。随后它执行一次没有 schema model 或 `ImportPlan` 的 streaming JSON event/token pass：计数至多 67,108,864 input bytes、10,000 entry objects、1,000,000 total values、八层 container、每个 string 1,048,576 UTF-8 bytes 和每个 number 128 bytes，同时拒绝 malformed JSON 与 duplicate object keys。它在第一个 exceeded dimension 停止并返回 measured/allowed values。只有成功 pass 才可反序列化 versioned schema，并将 `SKL-LIB-008` 的 alias/category/note/tag limits 验证为 `ImportPlan`；同一 batch 的 canonical source 重复作为 `internal_duplicate` conflict，而不是让 SQLite 主键或输入顺序选择 metadata。

对于原本不存在的 `data/skilload.db`，持久 import 在完成 input/schema/domain/conflict planning 后，在同一 data directory 建立 restrictive staging database；只有 schema 和 SQLite `COMMIT` 成功、sidecar 已处理且根重新验证后才 rename 为 live database。`COMMIT` 前失败必须关闭并移除仅由该调用创建的 staging database、sidecar、database lock 与空 data/state directory；不得删除任何预先存在或 identity 不匹配的路径。若 `COMMIT` 已完成但文件或父目录 durability sync 失败，返回错误而不声称 absence 或旧 generation；该情况必须由独立 fault-injection fixture 覆盖。

Library export 在创建 staging 文件前比较 no-follow output target 与有效 Library database generation（database、WAL、SHM）及 database lock，拒绝任何碰撞。其他 target 使用同目录 staging、file sync、rename、parent-directory sync；rename 前失败保留旧 target 或无 target 并清理 staging，rename 后 parent sync 失败返回错误但新 target 可能已经发布。成功仅在该最终 sync 完成后返回。

## Testing Consequences

Default tests use temporary XDG/HOME roots and an in-memory or temporary-file SQLite database compiled with the same FTS5 features as production. Repository contract tests run against both an in-memory fake and SQLite adapter. Tag and metadata fixtures cover whitespace trimming, NFC composition, full default case folding, Turkish locale independence, direct alias/category/note/tag-count limits, first-spelling retention, import/export ordering, removal by equivalent spelling, and FTS matches through both display and comparison forms. Pagination fixtures compare fake/SQLite default and adjacent pages, same-snapshot totals, offset-at/beyond-total emptiness, unsigned-64-bit offsets above SQLite's signed bound without conversion, and stable ordering under unchanged data. Import-reader fixtures exercise every exact byte/entry/value/depth/string/number boundary, duplicate keys, early termination, no partial model/plan, and atomic conflict failure. Path tests cover unset, empty, relative, and absolute values for every XDG variable; prove that relative values fall back identically from different current directories; prove invalid fallback `HOME` fails before filesystem access; reject equal, nested, or symlink-aliased effective application roots; and separately reject present empty/relative `CLAUDE_CONFIG_DIR`/`CODEX_HOME` plus required invalid `HOME` before Agent-root access. Configuration fixtures cover all three exact keys, unknown keys/tables, cache integers through signed-64-bit maximum plus overflow/zero rejection and decimal-string projection, absolute/relative/empty/non-UTF-8 Agent paths, unset defaults, idempotence, and `PathValue` projection. Workspace-store fixtures enforce exact byte/record/node/depth/scalar ceilings and forbidden YAML features without returning a partial model, while manifest fixtures inject alternate Git directories/worktrees/indexes/configuration and prove the bound real-index check preserves both persistent representations and pending recovery. Persistence fixtures keep branch/tag same-name source keys distinct, allow a `NULL` executable only for proved removal, move every workspace foreign key atomically during a proved instance rebind, and prove database-wide source migration cannot update any workspace row. Other tests prove that query construction creates no path; migration backup manifests survive injected failure and obey the newest-three/previous-generation retention rule; isolated restore validation rejects corrupt, newer, foreign-key-invalid, and WAL-mixed candidates and migrates a supported older candidate only on a disposable copy; atomic restore and rollback preserve one complete generation; explicit reset adopts no surviving artifact; FTS rebuild preserves base rows; unknown schema blocks writes; and concurrent mutations return deterministic commit/busy results.

## Decisions Deferred Beyond the Configuration Foundation

`PLAN-0002` selected and locked the Rust toolchain, `clap`, `serde`, TOML, error, filesystem-staging, JSON, and test dependencies for the configuration slice. A later durable-domain delivery selects final SQLite/FTS5 and HTTP dependency versions and SQL names, and must prove embedded FTS5 and release portability. Those dependency choices remain implementation details unless they change the ownership or boundary model.
