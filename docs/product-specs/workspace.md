# Workspace

Status: planned baseline for the skilload CLI MVP.

A **workspace** is the exact current directory containing `.skilload.yaml`. The configuration states desired sources, the lock records exact resolved content, and sync maintains derived Agent links. Workspace files are portable; local deployment ownership is not.

## SKL-WSP-001 - Exact-directory workspace discovery (Revision 1)

**Behavior.** skilload MUST NOT provide `init`. A workspace-dependent command MUST require `.skilload.yaml` in the current working directory and MUST NOT search parents. Library, Trust, source, global, manager, cache, config, and global doctor operations MAY run from any directory.

**Acceptance.** Running `workspace status` in a child directory fails with a structured workspace-not-found result even when a parent is a workspace. Running `library list` there still succeeds.

## SKL-WSP-002 - First add creates both files atomically (Revision 1)

**Behavior.** The first successful `workspace add` MUST commit `.skilload.yaml` and `.skilload.lock` together as one journaled, recoverable operation. A validation, approval, or write failure returned normally MUST leave neither file and MUST leave Agent Skill directories unchanged. An interruption between the two file renames MAY expose a transitional one-file state only until mandatory journal recovery restores the old absence or completes the new pair.

**Acceptance.** Fault injection at every precommit/write boundary restarts the application and runs recovery before asserting either both valid files or neither. A raw pre-recovery inspection may observe the recorded transitional state, but no recovered state or normally returned command may leave a one-file workspace.

## SKL-WSP-003 - Nested and non-Git workspaces (Revision 1)

**Behavior.** A workspace MAY exist outside Git. If the first add occurs below an existing parent workspace, skilload MUST refuse by default and MAY proceed only with explicit `--allow-nested`. The nested workspace then follows exact-directory discovery independently.

**Acceptance.** First add in a non-Git directory succeeds. First add under a parent workspace fails without the flag and creates independent config/lock files with it.

## SKL-WSP-004 - Portable source-only configuration (Revision 1)

**Behavior.** `.skilload.yaml` MUST contain a format version and canonical sources only. It MUST NOT contain Library IDs, Trust, Agent targets, resolved commits, integrity, profile IDs, timestamps, credentials, or machine paths.

**Acceptance.** A configuration copied to another machine has enough data to resolve desired sources and contains no machine-local identifier or deployment selection.

## SKL-WSP-005 - Deterministic lock content (Revision 1)

**Behavior.** `.skilload.lock` MUST contain a format version and deterministic records for canonical source, numeric repository ID, resolved commit, verified Skill name, and canonical integrity. Neither workspace file stores timestamps or machine paths. Both files are committed, machine-managed documents; commands MAY canonicalize formatting and do not preserve comments.

**Acceptance.** Repeating a no-op workspace mutation produces byte-identical files. Reordering input does not create nondeterministic lock ordering.

## SKL-WSP-006 - Add and remove mutate desired state only (Revision 1)

**Behavior.** `workspace add` and `workspace remove` MUST update config and lock together as one recoverable operation. They MUST NOT write Agent Skill directories; only `workspace sync` changes deployments. A missing removal target returns `not_found`, and an existing exact source add is idempotent.

**Acceptance.** After add/remove, Agent directories are unchanged. A crash leaves the pair at the old or new consistent state after journal recovery.

## SKL-WSP-007 - Direct source add and Library independence (Revision 1)

**Behavior.** `workspace add` MAY accept a Library entry or direct GitHub source. A direct source add MAY establish Trust through the normal approval flow but MUST NOT add the source to Library automatically. An already trusted Library source needs no repeated Trust approval.

**Acceptance.** Direct add creates workspace state and Trust when approved, while `library get` returns `not_found` until the user explicitly adds it to Library.

## SKL-WSP-008 - Empty workspace files are retained (Revision 1)

**Behavior.** Removing the last desired Skill MUST leave valid empty `.skilload.yaml` and `.skilload.lock` files. It MUST NOT implicitly delete workspace registration or managed Agent links.

**Acceptance.** The files remain parseable with zero entries. Status reports any still-deployed links as stale desired-state differences until sync removes them.

## SKL-WSP-009 - Lock reconciliation without advancement (Revision 1)

**Behavior.** `workspace lock` MUST reconcile configuration and lock by resolving new sources, preserving the pinned commit for unchanged existing sources, and removing stale lock entries. It MUST NOT advance an existing mutable branch or tag merely because upstream moved.

**Acceptance.** After adding one source manually to canonical config and moving another branch upstream, lock resolves only the new source and leaves the existing commit unchanged.

## SKL-WSP-010 - Atomic mutable-source update (Revision 1)

**Behavior.** `workspace update [selector]` MUST resolve and advance selected mutable refs. With no selector it MUST treat all mutable sources as one atomic batch. A SHA source returns `already_immutable`. Any validation, Trust, name, conflict, or availability failure MUST leave all commits and files unchanged.

**Acceptance.** In a two-source update where one candidate fails, neither pin advances. A successful batch writes one mutually consistent config/lock result.

## SKL-WSP-011 - Historical pin retains source intent (Revision 1)

**Behavior.** `workspace pin` MUST allow an explicit historical commit reachable for the same repository source, verify its content, and update the lock without replacing a mutable source ref in config. A later update MAY advance that source again.

**Acceptance.** Pinning a branch source to an older commit changes only its resolved lock data; config still names the branch and the next update can move forward.

## SKL-WSP-012 - Duplicate verified names are forbidden (Revision 1)

**Behavior.** Every desired source MUST have a valid required frontmatter `name`. Two workspace sources resolving to the same verified name MUST be a hard failure during add, lock, update, or pin, with no override.

**Acceptance.** A candidate batch with duplicate names makes no config, lock, cache-promotion, or deployment change and reports both conflicting sources.

## SKL-WSP-013 - Upstream name changes require acknowledgement (Revision 1)

**Behavior.** If an update or pin changes a source's verified Skill name, skilload MUST show the old and new name and require explicit acknowledgement. It MUST preflight duplicates, external conflicts, exact targets, and the reserved `skilload-manager` name before committing.

**Acceptance.** An unacknowledged rename makes no change. A rename to a duplicate or reserved name remains a hard failure even after acknowledgement.

## SKL-WSP-014 - Explicit source and format migration (Revision 1)

**Behavior.** `workspace migrate-source` MUST handle only the same-repository rename/transfer rule in `SKL-SRC-015`. `workspace migrate-format` MUST be the only command that rewrites an older supported workspace schema. Read, status, lock, update, pin, and sync MUST NOT silently migrate format.

**Acceptance.** Reading an older schema returns a migration-required diagnostic without changing bytes. Explicit migration produces deterministic current-format files or leaves the originals intact on failure.

## SKL-WSP-015 - Explicit Agent selection (Revision 1)

**Behavior.** `workspace sync` MUST require at least one explicit `--agent` and MUST accept one or more supported Agent values. Workspace config MUST NOT persist the target selection, and no per-Skill Agent filter is supported.

**Acceptance.** Sync without `--agent` fails before changing links. Sync with Claude and Codex selects both for that invocation while `.skilload.yaml` remains unchanged.

## SKL-WSP-016 - Complete-set, multi-Agent sync (Revision 1)

**Behavior.** Sync MUST reconcile the entire workspace locked set for every selected Agent, removing obsolete owned links and creating/restoring required links. Under normal command completion, all selected Agents succeed as one operation or none changes. Crash recovery follows `SKL-CACHE-008`'s journal guarantee rather than claiming instantaneous cross-directory atomicity.

**Acceptance.** Selecting both Agents cannot leave only one updated after a reported success or ordinary error. Failure injection demonstrates recovery to one coherent old or new deployment set.

## SKL-WSP-017 - Native project targets (Revision 1)

**Behavior.** The Claude adapter MUST target `<workspace>/.claude/skills/<name>`. The Codex adapter MUST target `<workspace>/.agents/skills/<name>`. skilload MUST use the workspace directory itself, not an ancestor discovered by the Agent.

**Acceptance.** A two-Agent sync creates only the expected entries under those two roots and records their exact absolute targets in local ownership state.

## SKL-WSP-018 - Cache-backed links and install names (Revision 1)

**Behavior.** Each managed project target MUST be a symlink to the verified immutable external cache entry. Its directory name MUST equal the verified frontmatter `name`; Library alias MUST never affect it. Both Agents selected for the same lock record MUST reference the same cache materialization.

**Acceptance.** Resolving both links reaches identical repository ID, commit, path, and integrity content. Changing an alias produces no link rename.

## SKL-WSP-019 - Exact occupied target is a hard failure (Revision 1)

**Behavior.** skilload MUST never replace or delete an exact target path it cannot prove it owns, including a regular directory, file, or foreign symlink. `--force` MUST NOT override this ownership rule.

**Acceptance.** A user-created `.claude/skills/review` makes preflight fail and remains byte-for-byte untouched after sync, cleanup, cache clear, and doctor fix.

## SKL-WSP-020 - Semantic conflict may be degraded (Revision 1)

**Behavior.** A known same-name Skill discoverable through another global, workspace, plugin, or Agent source, but not occupying the exact target, MUST require confirmation. If the user proceeds, sync MAY complete but status MUST record `degraded_name_conflict`. JSON mode uses the confirmation-token contract.

**Acceptance.** Declining leaves all selected Agent deployments unchanged. Proceeding creates only owned targets and status identifies the conflicting name/source and degraded state.

## SKL-WSP-021 - Agent-disabled Skill is a hard failure (Revision 1)

**Behavior.** If the selected Agent's effective configuration disables the Skill, sync MUST fail with `agent_disabled` and MUST NOT modify that configuration or auto-enable the Skill.

**Acceptance.** Preflight against a disabled name returns the Agent, name, and relevant config location; all links and config files remain unchanged.

## SKL-WSP-022 - Functional Agent preflight (Revision 1)

**Behavior.** Every selected Agent executable MUST be installed and resolvable. skilload MUST NOT enforce a numeric version threshold, but MUST preflight resolved roots, writable parent paths, disable settings, conflicts, and ownership capability before commit.

**Acceptance.** A missing executable or inaccessible root fails before any selected Agent changes. A resolvable Agent with no known numeric version may proceed when functional checks pass.

## SKL-WSP-023 - Environment-specific deployment identity (Revision 1)

**Behavior.** Deployment state MUST record the effective `HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and resolved Agent roots relevant to the selected adapters. Guarantees apply only to the same resolved environment. A different environment is a distinct target and MUST NOT be silently conflated with an old record.

**Acceptance.** Changing an override so an Agent root resolves elsewhere produces a distinct preflight/status target and never deletes an entry under the former root without verifying its recorded ownership.

## SKL-WSP-024 - Visibility timing guarantee (Revision 1)

**Behavior.** skilload MUST guarantee that a successful sync is visible to the selected local Agent on its next launch. It MUST NOT promise hot reload for a running Agent, even if a particular Agent version currently detects some changes live.

**Acceptance.** End-to-end acceptance starts a fresh Agent process after sync and observes the Skill. Tests do not fail solely because a running session needs restart.

## SKL-WSP-025 - Ownership, exclusion, and status (Revision 1)

**Behavior.** skilload MUST maintain a git-excluded local workspace manifest and a durable global workspace index containing exact owned links, cache targets, Agents, lock digest, and environment roots. In Git repositories it MUST maintain only its exact entries in Git's effective `info/exclude` file, resolving that file through Git rather than assuming `.git` is a directory, and MUST never edit shared `.gitignore`. A missing manifest MAY be rebuilt only when name, link target, commit, and integrity all match. `workspace status` is read-only, reports registered deployments by default, and MAY explicitly rerun Agent preflight.

**Acceptance.** Status creates no state and distinguishes healthy, missing, foreign, stale, degraded, and inaccessible entries. Manifest reconstruction refuses a near match. Ordinary repositories and linked worktrees both exclude the manifest through their Git-resolved `info/exclude` file, and user `.gitignore` and pre-existing exclude content remain unchanged.

## SKL-WSP-026 - Workspace deletion and scale (Revision 1)

**Behavior.** `workspace delete` MUST succeed only when desired config/lock are empty and no managed deployment remains. The user must sync previously deployed selected Agents to remove owned links first. The workspace feature set MUST support at least 200 desired Skills without changing semantics.

**Acceptance.** Delete with a remaining managed link fails and reports its Agent/path; after empty sync removes all owned links, delete removes workspace registration and files without touching unrelated content. A 200-Skill fixture passes lock/status/sync planning within an implementation-defined performance budget.

## SKL-WSP-027 - Offline workspace list (Revision 1)

**Behavior.** `workspace list` MUST be a read-only, offline projection of the exact current workspace's desired sources and corresponding lock state. It MUST use deterministic source ordering, identify configured sources that are unlocked or stale, and MUST NOT run Agent preflight, restore cache content, or rewrite either workspace file.

**Acceptance.** With networking denied, list reports each configured source and its locked commit/integrity when present, flags a config-only or stale lock entry, and leaves workspace bytes and local state unchanged. Outside an exact workspace it returns workspace-not-found.
