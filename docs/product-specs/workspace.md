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

**Behavior.** `.skilload.yaml` MUST contain a format version and canonical sources only. It MUST NOT contain Library IDs, Trust, Agent targets, resolved commits, integrity, profile IDs, timestamps, credentials, or machine paths. It MUST be one UTF-8 YAML document of at most 1,048,576 bytes and at most 200 source records. Before constructing the complete document model, a non-expanding event pass MUST reject more than 4,096 scalar/mapping/sequence nodes, more than eight nested mapping/sequence containers, any scalar longer than 4,096 UTF-8 bytes, aliases, anchors, explicit tags, directives, multiple documents, duplicate mapping keys, and non-string mapping keys. The byte reader and event pass MUST stop at the first exceeded ceiling; they MUST NOT expand aliases or allocate the full oversized document.

**Acceptance.** A valid configuration copied to another machine has enough data to resolve desired sources and contains no machine-local identifier or deployment selection. Exact-boundary fixtures accept 1,048,576 bytes, 200 sources, 4,096 nodes, eight container levels, and a 4,096-byte scalar when the schema is otherwise valid; the next byte/source/node/level/scalar byte and alias/anchor/tag/directive/multi-document/duplicate/non-string-key fixtures fail with a structured workspace-document error before source resolution, Trust lookup, network access, or persistent allocation.

## SKL-WSP-005 - Deterministic lock content (Revision 1)

**Behavior.** `.skilload.lock` MUST contain a format version and deterministic records for canonical source, numeric repository ID, resolved commit, verified Skill name, and canonical integrity. It MUST use the same one-document UTF-8 YAML subset and 1,048,576-byte, 200-record, 4,096-node, eight-container-level, and 4,096-byte-scalar ceilings defined for config in `SKL-WSP-004`, enforced before full deserialization. Neither workspace file stores timestamps or machine paths. Both files are committed, machine-managed documents; commands MAY canonicalize formatting and do not preserve comments.

**Acceptance.** Repeating a no-op workspace mutation produces byte-identical files. Reordering input does not create nondeterministic lock ordering. Lock fixtures enforce the same exact parsing boundaries and forbidden YAML features as config before any record is trusted or allocated as a complete model.

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

**Behavior.** Every desired source MUST have the required frontmatter name validated by `SKL-SRC-007`. Name identity is exact lowercase-ASCII byte equality with no trimming, case folding, or Unicode normalization. Two workspace sources resolving to the same verified name MUST be a hard failure during add, lock, update, or pin, with no override.

**Acceptance.** A candidate batch with byte-identical valid names makes no config, lock, cache-promotion, or deployment change and reports both conflicting sources. Invalid spellings such as `Review_Skill`, `review--skill`, or a source-directory/name mismatch fail validation rather than forming a second comparison domain.

## SKL-WSP-013 - Upstream name changes require acknowledgement (Revision 1)

**Behavior.** If an update or pin changes a source's verified Skill name, skilload MUST show the old and new name and require explicit acknowledgement. It MUST preflight duplicates, external conflicts, exact targets, and the reserved `skilload-manager` name before committing.

**Acceptance.** An unacknowledged rename makes no change. A rename to a duplicate or reserved name remains a hard failure even after acknowledgement.

## SKL-WSP-014 - Explicit source and format migration (Revision 1)

**Behavior.** `workspace migrate-source` MUST handle only the same-repository rename/transfer rule in `SKL-SRC-015`. `workspace migrate-format` MUST be the only command that rewrites an older supported workspace schema. Read, status, lock, update, pin, and sync MUST NOT silently migrate format.

**Acceptance.** Reading an older schema returns a migration-required diagnostic without changing bytes. Explicit migration produces deterministic current-format files or leaves the originals intact on failure.

## SKL-WSP-015 - Explicit Agent selection (Revision 1)

**Behavior.** `workspace sync` MUST require at least one explicit `--agent` and MUST accept one or more supported Agent values. Workspace config MUST NOT persist the target selection, and no per-Skill Agent filter is supported. The optional relocation flag `--rebind-from <OLD-WORKSPACE>` from `SKL-WSP-023` MUST be accepted only by sync, and its Agent selection MUST include every Agent recorded by the old workspace so ownership cannot be partially moved.

**Acceptance.** Sync without `--agent` fails before changing links. Sync with Claude and Codex selects both for that invocation while `.skilload.yaml` remains unchanged. A rebind that omits one previously deployed Agent fails before changing either old or new registration.

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

## SKL-WSP-022 - Trusted executable and functional Agent preflight (Revision 1)

**Behavior.** Before deriving a selected Agent deployment or conflict root, skilload MUST validate every environment path that adapter will use. A present `CLAUDE_CONFIG_DIR` or `CODEX_HOME`, and `HOME` whenever the selected adapter requires it, MUST be nonempty and absolute; an unset required `HOME` or a present empty/relative value MUST return structured `invalid_environment_path` before Agent-root filesystem access. skilload MUST NOT expand `~`, join an environment path to the current directory, or silently substitute an invalid explicit Agent value.

Every plan that may create, replace, repair, or restore content; fetch content; inspect Agent disable/conflict settings; or otherwise depend on current Agent behavior MUST resolve every selected Agent executable. An Agent override MUST come only from the absolute configured path in `SKL-OPS-006`; without one, skilload MUST search the fixed Agent basename only in nonempty absolute PATH directories and MUST ignore empty or relative components rather than interpreting them against the current directory. Before any version/help probe, it MUST resolve the candidate and symlink chain, require a regular executable file, and reject a final path inside the canonical current workspace, its enclosing Git worktree, or a skilload external source/cache/staging root. It MUST record the canonical path plus filesystem identity.

The same containment and identity rule MUST cover the complete interpreter chain before any script executes. Revision 1 accepts a native Mach-O/ELF executable or an executable script whose first line ends by newline or file EOF within 4,096 bytes. Its byte grammar is either `#!<absolute-interpreter>` followed optionally by one ASCII space and one `<literal-argument>`, or exactly `#!/usr/bin/env <basename>`. `<absolute-interpreter>` MUST match `/[A-Za-z0-9._+/-]+`, `<literal-argument>` MUST match `[-A-Za-z0-9._+/=:,]+`, and `<basename>` MUST match `[A-Za-z0-9][A-Za-z0-9._+-]*`; `/usr/bin/env` MUST use only the second form. It MUST resolve the absolute interpreter or env-selected fixed basename through the same safe resolver, recurse through at most four script identities, and reject cycles, relative direct interpreters, `env -S`, NUL/carriage return, tabs, assignments/options/paths or extra operands in the env form, or any other grammar as `unsupported_interpreter`. It MUST construct an explicit argv chain without a shell, provide a PATH containing only resolver-accepted absolute directories, record every file identity, and revalidate the complete chain immediately before each probe, execution, and mutation commit. A drifted or unsafe interpreter MUST prevent the original candidate from running.

Normal functional preflight MUST parse every repository-controlled Agent settings and conflict input under the `agent-project-input-v1` ceilings in `SKL-OPS-010` before returning any partial observation. It then runs any bounded noninteractive probe from a private empty working directory and preflights resolved roots, writable parent paths, disable settings, conflicts, and ownership capability before commit. The same resolver and interpreter rules MUST govern system Git, optional `gh`, the SSH client required by `SKL-SRC-016`, and Agent-visible `skilload` discovery; only Agent CLIs have configuration overrides. An unsafe candidate MUST never be probed, and if no safe candidate remains the operation MUST return structured `unsafe_executable_path`, `unsupported_interpreter`, or `executable_not_found` with safe path details. skilload MUST NOT enforce a numeric Agent version threshold.

A plan MAY omit Agent executable discovery and functional/settings probes only after validated root resolution and filesystem inspection prove it is removal-only: it performs no create, replace, repair, restore, fetch, or settings mutation/inspection and removes only exact owned links or manager copies plus their selected durable associations. This exception covers an empty-workspace sync, normal global uninstall, manager uninstall, cache cleanup, and equivalent recovery of the same subtractive plan. Every selected root MUST remain accessible and every target MUST match complete ownership evidence; an inaccessible, foreign, drifted, additive, or mixed plan still fails or requires the normal full preflight. Missing Agent software alone MUST NOT strand exact owned cleanup.

**Acceptance.** A missing executable fails an install, nonempty sync, repair, update, pin, or manager install before any selected Agent changes, while an empty sync, exact global uninstall, or exact manager uninstall succeeds without probing the absent executable when every selected target is accessible and owned. An inaccessible or drifted cleanup target still leaves the whole selected cleanup unchanged. With a fake `codex` in the workspace, `PATH=.:/usr/bin`, an empty PATH component, or an absolute worktree `bin` directory never executes that file; a symlink from an outside absolute PATH directory back into the worktree is also rejected before its probe. A safe outside script using `#!/usr/bin/env node` executes only a separately resolved outside `node` under the resolver-built PATH; workspace `node`, an overlong/unsupported shebang, a five-script chain, a cycle, and interpreter identity drift create no execution marker. A configured relative override is rejected by configuration, a configured absolute worktree override is rejected by preflight, and a safe outside executable with no known numeric version may proceed when functional checks pass. Present `CLAUDE_CONFIG_DIR=.`, `CODEX_HOME=relative`, or required `HOME=` fails from different current directories without accessing a derived root. Equivalent resolver fixtures cover `git`, optional `gh`, `ssh`, and Agent-visible `skilload`, and the Agent-input fixtures from `SKL-OPS-010` fail before settings/conflict model construction.

## SKL-WSP-023 - Resolved workspace deployment identity (Revision 1)

**Behavior.** A workspace deployment target MUST be identified only by `(canonical workspace path, Agent, canonical resolved project Skill root)`. Deployment state MUST also record the validated effective `HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, optional executable for a removal-only plan, compatibility/conflict roots, and other adapter observations used for preflight, but those observations are replaceable and MUST NOT allocate a second owner while the identity tuple is unchanged. Each local deployment manifest and matching durable workspace record MUST also share a random opaque `workspace_instance_id` and latest committed transaction evidence; this local identifier MUST NOT enter portable config or lock. A different canonical project Skill root is normally a distinct target and MUST NOT be silently conflated with an old record.

An explicit `workspace sync --rebind-from <OLD-WORKSPACE> --agent ...` MAY replace the canonical workspace component only when all of the following are proved before mutation: the current exact workspace contains the local manifest and a Git workspace proves it is untracked; its instance ID, prior canonical path, lock digest, transaction ID, Agents, targets, sources, pins, integrity, and raw link text exactly match one durable old record; the supplied old path canonicalizes to that record and is provably absent rather than merely inaccessible; no other current path or record claims the instance ID; the selected Agents include every recorded Agent; and every moved exact target at the new path is either absent or is a symlink with the recorded type, name, and raw link text carried with the manifest. A tracked/foreign manifest, foreign file, directory, symlink, duplicate instance, missing evidence, accessible old workspace, or inaccessible ownership resource MUST block rebind. A valid rebind MUST be one recoverable transaction that repairs moved links to their expected cache objects, transfers manifest/exclude/index/target/link ownership to the new canonical paths, removes only exact old exclusion ownership when it still exists, and then performs normal complete-set sync. Without the explicit flag, a path mismatch MUST remain read-only-detectable and MUST NOT auto-adopt or delete anything.

**Acceptance.** In a fixed workspace, changing only `CODEX_HOME` retains the Codex target at `<workspace>/.agents/skills`, refreshes conflict observations on the same deployment record, and does not make its existing exact links appear foreign. Changing an override or canonical workspace resolution so the actual project Skill root differs produces a distinct preflight/status target and never deletes an entry under the former root without verifying its recorded ownership. Renaming a synced workspace makes status report `relocation_required` with old/new paths and required Agents; an explicit fully proved rebind repairs both Agents and leaves one new registration, while copying the manifest without removing the old workspace, changing one link, omitting an Agent, or making the old path merely inaccessible fails with no ownership change.

## SKL-WSP-024 - Visibility timing guarantee (Revision 1)

**Behavior.** skilload MUST guarantee that a successful sync is visible to the selected local Agent on its next launch. It MUST NOT promise hot reload for a running Agent, even if a particular Agent version currently detects some changes live.

**Acceptance.** End-to-end acceptance starts a fresh Agent process after sync and observes the Skill. Tests do not fail solely because a running session needs restart.

## SKL-WSP-025 - Ownership, exclusion, and status (Revision 1)

**Behavior.** skilload MUST maintain a git-excluded local workspace manifest and a durable global workspace index containing the shared `workspace_instance_id`, exact owned links, cache targets, Agents, lock digest, target identities, last transaction evidence, and replaceable environment observations. In Git repositories it MUST use the sanitized Git process contract in `SKL-SRC-016` to resolve and record the canonical worktree root, per-worktree Git directory, effective index path, and effective `info/exclude` file without caller `GIT_*` state rather than assuming the workspace is the worktree root or `.git` is a directory. It MUST write an anchored, literal-escaped worktree-relative pattern for the manifest, maintain only its exact entries, and never edit shared `.gitignore`. Before every create, replace, or delete of the manifest, including journal recovery, it MUST revalidate those repository resources and use fixed `--git-dir`/`--work-tree` arguments, an application-owned `GIT_INDEX_FILE` equal to the recorded effective index, and a literal Git pathspec to prove the path is untracked; it MUST repeat that bound check immediately before the manifest filesystem action and durable commit. If the path is tracked or the recorded repository/index identity drifted, skilload MUST return actionable `tracked_local_manifest` or a stale-resource error, preserve the manifest, links, durable ownership, journal, and exclude entries, and require a safe retry; it MUST NOT continue writing machine-local state into a tracked file. A workspace path that cannot be represented safely as one literal exclude pattern MUST fail before manifest creation. A missing manifest MAY be rebuilt only when name, link target, commit, and integrity all match. `workspace status` is read-only, reports registered deployments by default, MAY explicitly rerun Agent preflight, and MUST report rather than mutate a matching manifest/database instance whose canonical workspace paths differ.

**Acceptance.** Status creates no state and distinguishes healthy, missing, foreign, stale, degraded, inaccessible, and tracked-local-manifest entries. Manifest reconstruction refuses a near match. After `git add -f .skilload/state/deployments.json`, sync and recovery leave its bytes, links, database/index, journal, and exclude block unchanged and name the index-removal remedy; inherited `GIT_INDEX_FILE`, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, and dynamic Git configuration cannot make that tracked file appear untracked. After the user untracks the exact path, retry can recover or mutate normally. Literal-path fixtures cover Git metacharacters. For a workspace at `<worktree>/packages/app`, the managed pattern matches only `/packages/app/.skilload/state/deployments.json` relative to that worktree rather than an incorrect root-level `.skilload` path. Root, nested, ordinary, and linked-worktree fixtures all bind the correct per-worktree index and exclude the manifest through their Git-resolved `info/exclude` file, including literal Git-ignore metacharacters in a directory name, while user `.gitignore` and pre-existing exclude content remain unchanged.

## SKL-WSP-026 - Workspace deletion and scale (Revision 1)

**Behavior.** `workspace delete` MUST succeed only when desired config/lock are empty and no managed deployment remains. The user must sync previously deployed selected Agents to remove owned links first. The workspace feature set MUST support at least 200 desired Skills without changing semantics.

**Acceptance.** Delete with a remaining managed link fails and reports its Agent/path; after empty sync removes all owned links, including when the Agent executable has already been uninstalled but exact targets remain accessible, delete removes workspace registration and files without touching unrelated content. A 200-Skill fixture passes lock/status/sync planning within an implementation-defined performance budget.

## SKL-WSP-027 - Offline workspace list (Revision 1)

**Behavior.** `workspace list` MUST be a read-only, offline projection of the exact current workspace's desired sources and corresponding lock state. It MUST use deterministic source ordering, identify configured sources that are unlocked or stale, and MUST NOT run Agent preflight, restore cache content, or rewrite either workspace file.

**Acceptance.** With networking denied, list reports each configured source and its locked commit/integrity when present, flags a config-only or stale lock entry, and leaves workspace bytes and local state unchanged. Outside an exact workspace it returns workspace-not-found.
