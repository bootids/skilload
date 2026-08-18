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
      - source: github:openai/skills#skills/example@refs/heads/main

The version-1 lock is a sorted list rather than a map whose serializer order might vary:

    version: 1
    skills:
      - source: github:openai/skills#skills/example@refs/heads/main
        repository_id: "123456"
        commit: 0123456789abcdef0123456789abcdef01234567
        name: example
        integrity: sha256:...

The implementation may add explicitly specified format fields, but never machine paths, timestamps, Trust, Agent selections, or Library IDs. Portable YAML and JSON encode a `RepositoryId(u64)` as a quoted decimal string so consumers cannot lose precision. Canonical serialization uses LF endings, two-space indentation, stable key order, a final newline, and source-sorted records. Comments and input formatting are not preserved.

`workspace add`, `remove`, `lock`, `update`, `pin`, `migrate-source`, and `migrate-format` stage both files as a pair. Even if only one document's logical content changes, the transaction validates both and preserves byte identity for a no-op.

## Workspace Deployment Manifest

Place a small versioned manifest in the workspace under `.skilload/state/deployments.json` and add its exact worktree-relative path to Git's effective `info/exclude` file when the workspace is inside a Git worktree. The path is local derived state and must not be committed. If a future implementation chooses a different local path, it must preserve the same exclusion and ownership semantics.

On first deployment, generate a random opaque `workspace_instance_id` with at least 128 bits of entropy. Store it only in the local manifest and matching durable `known_workspaces` row, never in `.skilload.yaml` or `.skilload.lock`. The manifest also contains workspace canonical path, lock digest, latest committed transaction ID, exclude ownership, and for each workspace target keyed by `(canonical workspace, Agent, canonical project Skill root)`: replaceable environment fingerprint/observations, exact link path, raw link text, expected cache target, source, repository ID, commit, and integrity. An environment-only change updates observations on the same target; it never creates competing ownership for an unchanged root. The durable `known_workspaces`/`owned_links` rows mirror every field needed to compare the manifest and find or clean known deployments when the workspace manifest is absent or inaccessible. The instance ID identifies one local deployment history, not portable workspace desire or permission by itself.

Determine repository membership, the canonical worktree root, and the exclude location by invoking Git directly with fixed noninteractive arguments from the workspace. Resolve `git rev-parse --show-toplevel` and `git rev-parse --git-path info/exclude`; require the canonical workspace to be the root or a descendant of the returned worktree root, interpret a relative exclude result against the workspace, and retain an absolute result. Do not construct `<workspace>/.git/info/exclude`, because a linked worktree has a `.git` file and normally resolves the exclude path into its common repository.

Compute the manifest path relative to the worktree root, normalize separators to `/`, and prefix `/` so Git evaluates it from that root. Encode it as one literal Git-ignore pattern by backslash-escaping `\`, `*`, `?`, `[`, `]`, `!`, and `#`; reject NUL, CR, LF, or any path that cannot round-trip as one pattern. Thus a nested workspace writes an equivalent of `/packages/app/.skilload/state/deployments.json`, not `.skilload/state/deployments.json`. Preflight the resolved exclude file without following an unsafe final path, then modify it through a managed block with a stable marker and only exact skilload-owned patterns. Verify the candidate with a temporary `git check-ignore` fixture during adapter tests. Preserve every user line and never edit `.gitignore`. Because linked worktrees may share one resolved exclude file, key and reference-count managed entries by `(resolved exclude file, exact escaped pattern)` and delete an inserted pattern only when no known managed workspace using that file still needs it.

## Action Planning and Conflict Classes

Each Agent adapter returns observations; the application layer assigns policy:

* `owned_exact`: target and ownership evidence exactly match; it may be retained, replaced, or removed.
* `missing`: safe to create when its parent passes preflight.
* `foreign_exact`: file/directory/symlink occupies the exact target without exact ownership; hard failure, never overridable.
* `internal_duplicate`: two desired sources share a verified name; hard failure.
* `reserved_name`: external deployment uses `skilload-manager`; hard failure.
* `agent_disabled`: effective Agent configuration disables the name; hard failure.
* `semantic_name_conflict`: same name is discoverable elsewhere but exact target is free; confirmation may produce `degraded_name_conflict`.
* `inaccessible`: target/profile cannot be inspected or written; hard failure for the whole selected operation except the stored-profile detach transition below, which plans no target filesystem action.
* `drifted_owned`: a previously owned target no longer matches expected type/target/marker; report and refuse destructive action until a verifiable recovery path exists.
* `relocation_required`: the current local manifest and one durable instance agree except for canonical workspace-derived paths; report the candidate old/new paths and complete recorded Agent set, but mutate only through explicit proved rebind.

Confirmation never converts a hard failure into permission to overwrite. Detach is not such an override: it is valid only for one explicitly selected inaccessible stored profile, removes active desire without touching the target, and preserves orphan evidence. A plan contains every conflict for all selected Agents so the user never approves one name only to encounter a hidden later target.

## Journal Format

Create one directory per mutation under `XDG_STATE_HOME/skilload/transactions/<uuid>/` with restrictive permissions:

    journal.json
    staged/
    backups/

`journal.json` is versioned and records transaction ID, operation, phase, baseline revisions/digests, ordered resources, old/new database intent digest, old/new workspace bytes or their durable backup paths, old/new workspace instance/path/exclude identities for rebind, exact link before/after descriptions, temporary names, and expected ownership/integrity. Write updates through temp-file plus rename and fsync the transaction directory.

Phases are monotonic:

1. `prepared`: all preflight and confirmation complete; staged data and rollback evidence are durable.
2. `external_applied`: reversible workspace/target/cache actions are installed, but the SQLite commit anchor is absent.
3. `committed`: one SQLite transaction writes desired/ownership/index changes plus this transaction ID.
4. `verified`: observed files/links/database match the new state.
5. `complete`: backups may be pruned and the journal archived/removed.

SQLite `committed_transactions` is the recovery authority. If its transaction ID is absent, recovery rolls external actions back to the recorded old state. If present, recovery rolls them forward to the recorded new state. Recovery operations are idempotent and revalidate ownership before every remove/replace.

## Safe Filesystem Changes

Stage a regular file in the destination's parent filesystem, fsync it, then rename. For an owned existing file/link, rename it to the journal's unique backup name before installing the new entry. A new symlink is created under a unique temporary name, verified with `lstat`/`readlink`, then renamed to the final name. Never follow a final target path while deciding ownership.

Workspace config and lock are a journaled pair: stage both, move old files to transaction backups, install both new files, then commit the database/index anchor. First-add records old absence. An interruption between renames can expose a one-file transitional state before recovery; the journal makes that state identifiable rather than instantaneously atomic. Rollback removes only exact transaction-created entries and restores recorded backups, while roll-forward completes both new files. A normally returned command and a recovered restart expose only the coherent old or new pair.

For link removal, rename the exact owned link into the transaction backup area in the same parent instead of unlinking immediately. After the commit anchor and verification, delete the backup. Foreign or drifted paths stop recovery and surface an explicit manual blocker rather than risking user content.

Cross-device renames are avoided by staging final filesystem entries in their destination parent. Cache object promotion stages under the cache root. Journals may retain hashes and paths to those staged objects rather than moving large trees into the state root.

## Operation Semantics

`workspace lock` resolves only missing/new desired sources and removes stale lock records. It carries existing pins unchanged. `workspace update` resolves selected mutable refs; the unselected form plans all mutable sources in one transaction. `workspace pin` resolves an explicit commit while leaving config source intent unchanged.

`workspace sync` reads a complete valid lock, requires explicit Agents, and plans the entire locked set for each selected Agent. It removes obsolete exact owned links for those Agent profiles and creates/restores required cache links. Agent selection is not written to portable config.

When the manifest's recorded canonical workspace differs from the exact current workspace, normal sync returns `relocation_required` without change. `workspace sync --rebind-from <OLD-WORKSPACE>` first canonicalizes the supplied old path without requiring it to exist and requires it to equal the manifest and durable record. Preflight then requires: byte-equal instance ID, lock digest, latest committed transaction ID, Agent/target/source/pin/integrity/raw-link evidence between manifest and database; `git ls-files --error-unmatch` or equivalent proving the manifest is not tracked when the workspace is Git-backed; no second record with the instance ID; an old path proved nonexistent with accessible ancestors rather than permission-denied or indeterminate; selected Agents containing every recorded Agent; the current config/lock pair matching the manifest lock digest; and each moved target at its derived new path either absent or a symlink with the exact recorded raw text and expected name. A tracked manifest, regular file/directory, different symlink, accessible old workspace, duplicate/copy, partial Agent set, inaccessible exclude/target resource, or changed evidence is a hard failure. Exact raw text plus matching local/durable instance and transaction evidence permits replacement of a moved relative symlink whose resolution changed solely because its parent moved; it does not authorize any other lookalike.

The rebind `DeploymentPlan` includes normal complete-set sync plus one atomic ownership transition: repair moved links to the same verified cache objects, rewrite manifest canonical/target paths while retaining the instance ID, change every matching `known_workspaces`, `workspace_targets`, and `owned_links` key, and transfer Git exclude ownership. If the old and new workspace resolve to the same exclude file, replace only the exact old anchored pattern with the exact new pattern. If they use different files, remove the exact old owned pattern when its file remains accessible, or accept absence of the old file; an inaccessible extant file blocks. Add the new pattern only after ordinary exclude safety checks. The journal makes crash recovery restore the complete old registration/link/pattern set or finish the complete new set. Only after that transition does normal sync add any newly selected Agent; no portable file receives the instance ID.

`global install` adds desired target associations and links. Normal `global uninstall` removes selected associations and exact owned links. When exactly one stored `--profile` is inaccessible, `global uninstall --profile <id> --detach-inaccessible` instead commits a database-only transition: selected associations leave the active target set and their source/profile/path/link/pin/integrity evidence moves to detached-orphan rows. The result says the link was not removed. Detached rows do not join later update/pin plans or protect cache objects, but list/status continue warning about them. If that profile later becomes accessible, normal uninstall may remove an exact matching orphaned link and delete the orphan row; foreign or drifted content remains untouched. `global sync` restores the existing shared pin. `global update` and `global pin` plan every active target of each source because one source has one global pin. The no-selector update batches all mutable sources.

Manager install/uninstall uses the same transaction engine but stages embedded copied assets and version markers rather than external cache links.

## Cache Prune and Clear

Build the protected-cache set from exact verified active managed links in durable ownership plus accessible workspace manifests. A lockfile, Trust record, desired record without a link, or detached-orphan record is not protected. `cache prune` locks the cache index, verifies candidates are not protected, then renames each selected object to a transaction quarantine before final deletion. Least-recently-used data is operational metadata and does not affect object integrity. Its monotonic recency advances only when a successful mutation promotes an object or creates/restores a managed link; read-only commands and already-satisfied no-op mutations do not rewrite recency, preserving `SKL-CLI-007` idempotence.

`cache clear` first discovers all known managed external links. Normal mode requires every relevant known workspace/profile to be accessible and every removable link to match ownership; any mismatch or unrebound relocation candidate aborts before changes and reports the explicit rebind command when locally provable. It then transactionally removes links, preserves desired/lock/Trust/Library state, and deletes external cache objects. `--force` may skip inaccessible/mismatched links but records each orphan/broken risk in the result and durable status. Manager copies are outside the external cache set.

Before any mutation promotes a new object, quota planning calculates current bytes, protected bytes, reclaimable LRU bytes, requested bytes, and projected post-operation bytes for the whole batch. The absent configuration default is 536,870,912 bytes. A valid `--cache-limit-bytes` value replaces the configured limit only for that invocation and must be at least the configured value; planning rejects the flag on non-promoting operations. The plan prunes before committing domain state and binds configured/effective/projected/override values into confirmation and human/JSON results. Nothing writes the override back to configuration or later domain state.

## Corruption and Cache Miss

Every promotion, link creation/replacement, and mutating use verifies the cache manifest and expected integrity before exposing or reusing a target. On mismatch, atomically move the object under `cache/quarantine/<uuid>`, record why, and fetch the same repository ID/commit/path once. Promote only the expected digest. Read-only status, info, and doctor observers report a mismatch without moving or refetching the object. Never change a workspace/global pin because remote bytes differ. A deployed native symlink is read directly by the Agent, so skilload cannot prevent a same-account edit or disk fault from being consumed between the modification and the next integrity observation; the guarantee begins again once skilload detects the mismatch and refuses to reuse it as valid.

A cache miss is restored only by network-capable sync/pin/update/add flows with active Trust. Status and doctor report the miss but remain offline. A lock whose commit is unavailable and has no verified cache entry is an explicit unrecoverable-at-present state, not an invitation to use the ref head.

## Status and Doctor

Status joins desired/pinned, ownership, and observed state into typed findings: healthy, missing, stale, degraded name conflict, foreign exact, drifted owned, disabled, inaccessible, detached orphan, relocation required, duplicate workspace instance, cache missing/corrupt, Trust blocked, and recovery pending. Detached orphan rows are warnings, not active desired targets. Default workspace status reports every registered deployment for that exact workspace; when the current manifest's instance matches one old record but canonical paths differ, it reports old/new `PathValue` fields, required Agents, and `workspace sync --rebind-from` guidance without changing either record. An explicit option may rerun current Agent preflight.

Doctor uses the same observers across global database, cache, manager, profiles, journals, and known workspaces. `--fix` invokes only repair actions with complete local proof: finish/rollback a journal, rebuild FTS, recreate an exact link from a verified cache object when exact Trust is active, or reconstruct a workspace manifest when all fields match. It never fetches or adopts, and revoked Trust turns a missing external link into a reported blocker rather than a repair action.

## Failure-Injection Acceptance

The transaction adapter exposes failpoints after every journal write, rename, database commit, and verification. Tests execute each operation with every failpoint, allow direct pre-recovery inspection to observe a journal-described transitional state, restart a fresh application, run recovery, and then assert one coherent old/new state, no unowned change, no dangling temporary target, and a complete/blocked journal explanation. Git exclusion fixtures cover root and nested workspaces, literal metacharacters, ordinary repositories, and linked worktrees; `git check-ignore -v` must identify the exact manifest through the Git-resolved exclude file while a root-level lookalike and neighboring paths remain visible and user lines remain unchanged. Relocation matrices cover same-filesystem rename, copy-then-remove moves with preserved symlinks, changed relative-link resolution, different/same/missing exclude files, both Agents, missing Agent selection, accessible/inaccessible old paths, duplicate manifests, changed lock/transaction/link evidence, foreign new targets, and every rebind failpoint; only the fully proved case ends with one new registration and no old owned path/pattern. Multi-Agent and multi-source batches receive the same matrix.
