# Deployment Transactions and Recovery Design

Status: planned design for the 0.1 CLI MVP. It supports `SKL-WSP-*`, `SKL-GLB-*`, and `SKL-CACHE-*` without claiming an impossible cross-filesystem atomic primitive.

## Behavior Traceability

* Workspace discovery, document, lock, migration, sync, ownership, status, deletion, and list mechanisms implement `SKL-WSP-001` through `SKL-WSP-027`.
* Global desired state, shared pins, multi-target transactions, ownership, lifecycle, and reads implement `SKL-GLB-003` through `SKL-GLB-013`.
* Manager copy transactions support `SKL-MGR-003` through `SKL-MGR-005`; Agent-specific paths and asset rules remain in the Agent design.
* Cache retention/clear/recovery/information and journal/doctor mechanisms implement `SKL-CACHE-001` through `SKL-CACHE-010` and support `SKL-OPS-001`, `SKL-OPS-002`, `SKL-OPS-005`, `SKL-OPS-008`, and `SKL-OPS-010`.

## State Model

Deployment separates desired, pinned, owned, and observed state:

* Workspace desired sources live in `.skilload.yaml`; exact pins live in `.skilload.lock`.
* Global desired target associations and shared pins live in SQLite.
* Workspace/local ownership is represented by a git-excluded workspace manifest plus matching durable database index rows.
* Global external and manager ownership is represented by durable database rows and on-disk links/markers.
* Observed state is read fresh from target roots and cache manifests. It is never adopted solely because it resembles expected state.

Application services construct a complete `DeploymentPlan` from these inputs before touching a target. A plan names its baseline revisions/digests, requested Agents/profiles, cache objects, exact creates/removes, conflicts, degraded results, workspace document changes, database changes, and confirmation requirements.

## Workspace Documents

Workspace-scoped requests bind to the canonical exact current directory and read `.skilload.yaml` only there. They do not search ancestors for an active workspace. Before the first add, a separate non-mutating ancestor scan detects a parent `.skilload.yaml` solely to enforce the default nested-workspace refusal and explicit `--allow-nested` override. A workspace does not require a Git repository.

Use structured YAML parsing and canonical serialization. A conceptual version-1 config is:

    version: 1
    skills:
      - source: github:openai/skills#skills/example@main

The version-1 lock is a sorted list rather than a map whose serializer order might vary:

    version: 1
    skills:
      - source: github:openai/skills#skills/example@main
        repository_id: "123456"
        commit: 0123456789abcdef0123456789abcdef01234567
        name: example
        integrity: sha256:...

The implementation may add explicitly specified format fields, but never machine paths, timestamps, Trust, Agent selections, or Library IDs. Portable YAML and JSON encode a `RepositoryId(u64)` as a quoted decimal string so consumers cannot lose precision. Canonical serialization uses LF endings, two-space indentation, stable key order, a final newline, and source-sorted records. Comments and input formatting are not preserved.

`workspace add`, `remove`, `lock`, `update`, `pin`, `migrate-source`, and `migrate-format` stage both files as a pair. Even if only one document's logical content changes, the transaction validates both and preserves byte identity for a no-op.

## Workspace Deployment Manifest

Place a small versioned manifest in the workspace under `.skilload/state/deployments.json` and add that exact path to `.git/info/exclude` when the workspace is a Git repository. The path is local derived state and must not be committed. If a future implementation chooses a different local path, it must preserve the same exclusion and ownership semantics.

The manifest contains workspace canonical path, environment fingerprint, lock digest, and for each Agent: resolved root, exact link path, expected cache target, source, repository ID, commit, integrity, and last committed transaction ID. The durable `known_workspaces`/`owned_links` rows mirror enough data to find and clean known deployments when the workspace manifest is absent or inaccessible.

Modify `.git/info/exclude` through a managed block with a stable marker and only exact skilload-owned entries. Preserve every user line and never edit `.gitignore`. Removal deletes only entries skilload previously inserted and only when no managed state needs them.

## Action Planning and Conflict Classes

Each Agent adapter returns observations; the application layer assigns policy:

* `owned_exact`: target and ownership evidence exactly match; it may be retained, replaced, or removed.
* `missing`: safe to create when its parent passes preflight.
* `foreign_exact`: file/directory/symlink occupies the exact target without exact ownership; hard failure, never overridable.
* `internal_duplicate`: two desired sources share a verified name; hard failure.
* `reserved_name`: external deployment uses `skilload-manager`; hard failure.
* `agent_disabled`: effective Agent configuration disables the name; hard failure.
* `semantic_name_conflict`: same name is discoverable elsewhere but exact target is free; confirmation may produce `degraded_name_conflict`.
* `inaccessible`: target/profile cannot be inspected or written; hard failure for the whole selected operation.
* `drifted_owned`: a previously owned target no longer matches expected type/target/marker; report and refuse destructive action until a verifiable recovery path exists.

Confirmation never converts a hard failure into permission to overwrite. A plan contains every conflict for all selected Agents so the user never approves one name only to encounter a hidden later target.

## Journal Format

Create one directory per mutation under `XDG_STATE_HOME/skilload/transactions/<uuid>/` with restrictive permissions:

    journal.json
    staged/
    backups/

`journal.json` is versioned and records transaction ID, operation, phase, baseline revisions/digests, ordered resources, old/new database intent digest, old/new workspace bytes or their durable backup paths, exact link before/after descriptions, temporary names, and expected ownership/integrity. Write updates through temp-file plus rename and fsync the transaction directory.

Phases are monotonic:

1. `prepared`: all preflight and confirmation complete; staged data and rollback evidence are durable.
2. `external_applied`: reversible workspace/target/cache actions are installed, but the SQLite commit anchor is absent.
3. `committed`: one SQLite transaction writes desired/ownership/index changes plus this transaction ID.
4. `verified`: observed files/links/database match the new state.
5. `complete`: backups may be pruned and the journal archived/removed.

SQLite `committed_transactions` is the recovery authority. If its transaction ID is absent, recovery rolls external actions back to the recorded old state. If present, recovery rolls them forward to the recorded new state. Recovery operations are idempotent and revalidate ownership before every remove/replace.

## Safe Filesystem Changes

Stage a regular file in the destination's parent filesystem, fsync it, then rename. For an owned existing file/link, rename it to the journal's unique backup name before installing the new entry. A new symlink is created under a unique temporary name, verified with `lstat`/`readlink`, then renamed to the final name. Never follow a final target path while deciding ownership.

Workspace config and lock are a journaled pair: stage both, move old files to transaction backups, install both new files, then commit the database/index anchor. First-add records old absence. Rollback removes only exact transaction-created entries and restores recorded backups.

For link removal, rename the exact owned link into the transaction backup area in the same parent instead of unlinking immediately. After the commit anchor and verification, delete the backup. Foreign or drifted paths stop recovery and surface an explicit manual blocker rather than risking user content.

Cross-device renames are avoided by staging final filesystem entries in their destination parent. Cache object promotion stages under the cache root. Journals may retain hashes and paths to those staged objects rather than moving large trees into the state root.

## Operation Semantics

`workspace lock` resolves only missing/new desired sources and removes stale lock records. It carries existing pins unchanged. `workspace update` resolves selected mutable refs; the unselected form plans all mutable sources in one transaction. `workspace pin` resolves an explicit commit while leaving config source intent unchanged.

`workspace sync` reads a complete valid lock, requires explicit Agents, and plans the entire locked set for each selected Agent. It removes obsolete exact owned links for those Agent profiles and creates/restores required cache links. Agent selection is not written to portable config.

`global install` adds desired target associations and links. `global uninstall` removes selected associations and owned links. `global sync` restores the existing shared pin. `global update` and `global pin` plan every target of each source because one source has one global pin. The no-selector update batches all mutable sources.

Manager install/uninstall uses the same transaction engine but stages embedded copied assets and version markers rather than external cache links.

## Cache Prune and Clear

Build the protected-cache set from exact verified managed links in durable ownership plus accessible workspace manifests. A lockfile, Trust record, or desired record without a link is not protected. `cache prune` locks the cache index, verifies candidates are not protected, then renames each selected object to a transaction quarantine before final deletion. Least-recently-used data is operational metadata and does not affect object integrity. Its monotonic recency advances only when a successful mutation promotes an object or creates/restores a managed link; read-only commands and already-satisfied no-op mutations do not rewrite recency, preserving `SKL-CLI-007` idempotence.

`cache clear` first discovers all known managed external links. Normal mode requires every relevant known workspace/profile to be accessible and every removable link to match ownership; any mismatch aborts before changes. It then transactionally removes links, preserves desired/lock/Trust/Library state, and deletes external cache objects. `--force` may skip inaccessible/mismatched links but records each orphan/broken risk in the result and durable status. Manager copies are outside the external cache set.

Before any mutation promotes a new object, quota planning calculates current bytes, protected bytes, reclaimable LRU bytes, and requested bytes. It prunes before committing domain state. An explicit quota override is bound into confirmation and result.

## Corruption and Cache Miss

Every mutating use verifies the cache manifest and expected integrity before exposing a target. On mismatch, atomically move the object under `cache/quarantine/<uuid>`, record why, and fetch the same repository ID/commit/path once. Promote only the expected digest. Read-only status, info, and doctor observers report a mismatch without moving or refetching the object. Never change a workspace/global pin because remote bytes differ.

A cache miss is restored only by network-capable sync/pin/update/add flows with active Trust. Status and doctor report the miss but remain offline. A lock whose commit is unavailable and has no verified cache entry is an explicit unrecoverable-at-present state, not an invitation to use the ref head.

## Status and Doctor

Status joins desired/pinned, ownership, and observed state into typed findings: healthy, missing, stale, degraded name conflict, foreign exact, drifted owned, disabled, inaccessible, cache missing/corrupt, Trust blocked, and recovery pending. Default workspace status reports every registered deployment for that exact workspace. An explicit option may rerun current Agent preflight.

Doctor uses the same observers across global database, cache, manager, profiles, journals, and known workspaces. `--fix` invokes only repair actions with complete local proof: finish/rollback a journal, rebuild FTS, recreate an exact link from a verified cache object when exact Trust is active, or reconstruct a workspace manifest when all fields match. It never fetches or adopts, and revoked Trust turns a missing external link into a reported blocker rather than a repair action.

## Failure-Injection Acceptance

The transaction adapter exposes failpoints after every journal write, rename, database commit, and verification. Tests execute each operation with every failpoint, restart a fresh application, run recovery, and assert one coherent old/new state, no unowned change, no dangling temporary target, and a complete/blocked journal explanation. Multi-Agent and multi-source batches receive the same matrix.
