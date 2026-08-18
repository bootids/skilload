# Application and Persistence Design

Status: planned design for the 0.1 CLI MVP. No crate or schema described here exists yet.

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

`crates/skilload-core` should expose an `Application` facade constructed from port implementations. Internally it contains:

* `domain`: validated value types such as `CanonicalSource`, `RepositoryId`, `GitCommit`, `SkillPath`, `SkillName`, `Integrity`, `TrustState`, `ProfileId`, `DeploymentStatus`, and typed outcomes/errors.
* `application`: commands and queries. A command may mutate; a query is read-only by type and dependency contract.
* `ports`: traits for durable repositories, workspace documents, content acquisition, cache, Agent inspection, ownership, transaction journals, clocks, and identifiers.
* `adapters`: concrete SQLite, filesystem, GitHub HTTP, system Git, and Agent implementations.
* domain-focused modules (`library`, `trust`, `source`, `workspace`, `global`, `cache`, `agents`, `persistence`, `recovery`) that group rules and service implementations without changing dependency direction.

`crates/skilload-cli` owns `clap` command definitions, conversion into application requests, human rendering, JSON envelope serialization, and process exit status. It composes production adapters once at startup but does not expose them to command handlers.

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

Use SQLite compiled into the binary with FTS5 enabled. The exact SQL is a P1 deliverable, but ownership is fixed:

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

Beyond required `version`, only `cache_limit_bytes`, `agents.claude.executable`, and `agents.codex.executable` are permitted; credentials, Trust, desired deployments, dynamic roots, command lines, and arbitrary Agent tables are not. `version` maps only to read-only `schema_version` metadata, never a set/unset key. The absent/unset cache value resolves to 536,870,912 bytes. `config set cache_limit_bytes <BYTES>` accepts a positive finite integer and `config unset cache_limit_bytes` returns to that default. Each Agent executable setter accepts one nonempty valid-UTF-8 absolute path, performs CWD-independent lexical normalization without probing it, and stores it under the matching table; unset removes the key and restores no-override lookup of the fixed basename. Get/list return `{ configured: false, value: null, default_command: "claude"|"codex" }` for an absent Agent key and a native-path value plus `configured: true` when set; the CLI serializes that value as `PathValue`. Parsing uses a structured TOML decoder with unknown-field denial and validated types/ranges. Read commands operate on defaults when the file is absent. `config set` stages a complete canonical document and atomically renames it; it does not preserve comments.

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

`WorkspaceStore` performs the byte-capped, non-expanding YAML event validation from `SKL-WSP-004` and `SKL-WSP-005` before schema deserialization. It returns measured counters with typed errors and never exposes a partially built config/lock model to application services. Its manifest mutation methods require a sanitized `GitRepositoryView` containing canonical worktree, per-worktree Git-directory, effective-index, and exec-path identities plus a fresh bound literal tracked-state observation in both their input baseline and commit callback; `tracked_local_manifest` or resource drift preserves the journal and all ownership rows rather than treating the manifest as an ordinary writable file.

The `source migrate` unit of work may update only `library_entries`/tags/FTS, `trust_records`, `global_sources`, and their global target/global-domain ownership relationships. Queries may join `known_workspaces` and workspace-domain `owned_links` to report impact, but the transaction denies workspace-table and workspace-domain ownership writes. The separately journaled `workspace migrate-source` owns portable config/lock rewrites and matching workspace ownership rows, so its preflight always compares the original source spelling on both sides.

## Concurrency

Use a global durable-database mutation lock plus a canonical-path workspace lock when relevant. Acquire locks in a single documented order: global/database first, then lexicographically sorted workspace/profile target locks. Bounded acquisition returns typed `busy` with lock domain and elapsed limit.

Network and Git acquisition may occur before the final lock to reduce contention. The stage records database revision, workspace config/lock digest, Trust revision, global source revision, and target ownership observations. After reacquiring locks, the command revalidates every baseline. Drift restarts safe resolution or returns a typed stale result; it never commits on stale assumptions.

SQLite busy timeout is a second line of defense, not the primary product lock. No application service holds a filesystem lock while waiting indefinitely on network input or human confirmation.

## Import and Export

Library export is built from domain records, sorted by canonical source, and serialized as a versioned portable JSON document. It contains no database row IDs or local timestamps needed only for operations.

`LibraryImportReader` first performs one streaming JSON event/token pass with no schema model or `ImportPlan`: it counts at most 67,108,864 input bytes, 10,000 entry objects, 1,000,000 total values, eight container levels, 1,048,576 UTF-8 bytes per string, and 128 bytes per number, while rejecting malformed JSON and duplicate object keys. It stops on the first exceeded dimension and returns measured/allowed values. Only a successful pass may deserialize the versioned schema and validate the direct alias/category/note/tag limits from `SKL-LIB-008` into an `ImportPlan` containing additions, kept entries, explicit metadata replacements, and conflicts. Tag parsing uses the pinned Unicode-15.1 normalization domain value documented in [`../references/unicode-15-1-tag-normalization.md`](../references/unicode-15-1-tag-normalization.md) before planning: duplicate tag comparison keys retain the first document-order display spelling, and committed/exported rows sort by comparison key rather than locale. Dry-run returns that plan. Commit revalidates the database revision and applies the entire plan in one SQLite transaction.

## Testing Consequences

Default tests use temporary XDG/HOME roots and an in-memory or temporary-file SQLite database compiled with the same FTS5 features as production. Repository contract tests run against both an in-memory fake and SQLite adapter. Tag and metadata fixtures cover whitespace trimming, NFC composition, full default case folding, Turkish locale independence, direct alias/category/note/tag-count limits, first-spelling retention, import/export ordering, removal by equivalent spelling, and FTS matches through both display and comparison forms. Import-reader fixtures exercise every exact byte/entry/value/depth/string/number boundary, duplicate keys, early termination, no partial model/plan, and atomic conflict failure. Path tests cover unset, empty, relative, and absolute values for every XDG variable; prove that relative values fall back identically from different current directories; prove invalid fallback `HOME` fails before filesystem access; reject equal, nested, or symlink-aliased effective application roots; and separately reject present empty/relative `CLAUDE_CONFIG_DIR`/`CODEX_HOME` plus required invalid `HOME` before Agent-root access. Configuration fixtures cover all three exact keys, unknown keys/tables, positive cache integers, absolute/relative/empty/non-UTF-8 Agent paths, unset defaults, idempotence, and `PathValue` projection. Workspace-store fixtures enforce exact byte/record/node/depth/scalar ceilings and forbidden YAML features without returning a partial model, while manifest fixtures inject alternate Git directories/worktrees/indexes/configuration and prove the bound real-index check preserves both persistent representations and pending recovery. Persistence fixtures keep branch/tag same-name source keys distinct, allow a `NULL` executable only for proved removal, move every workspace foreign key atomically during a proved instance rebind, and prove database-wide source migration cannot update any workspace row. Other tests prove that query construction creates no path; migration backup manifests survive injected failure and obey the newest-three/previous-generation retention rule; isolated restore validation rejects corrupt, newer, foreign-key-invalid, and WAL-mixed candidates and migrates a supported older candidate only on a disposable copy; atomic restore and rollback preserve one complete generation; explicit reset adopts no surviving artifact; FTS rebuild preserves base rows; unknown schema blocks writes; and concurrent mutations return deterministic commit/busy results.

## Decisions Deferred to P1

P1 selects and locks exact Rust crate versions and final SQL names. It may use `rusqlite`, `clap`, `serde`, and a mature HTTP client, but must prove embedded FTS5 and release portability. Those dependency versions are implementation details; changing them later does not change this design unless the ownership or boundary model changes.
