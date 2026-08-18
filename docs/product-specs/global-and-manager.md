# Global Deployment and Manager Skill

Status: planned baseline for the skilload CLI MVP.

A **global deployment** makes a selected external Library Skill visible in every local session for one Agent profile. The built-in **manager Skill** is a separate skilload-owned asset that teaches an Agent to invoke skilload's JSON CLI; it is not an external source or Library entry.

## SKL-GLB-001 - Library and Trust eligibility (Revision 1)

**Behavior.** A new global external deployment MUST select a source already in Library and MUST require active exact Trust. Users MAY select one or several Library entries in an invocation. A direct non-Library GitHub source is not eligible for `global install`.

**Acceptance.** Installing a trusted Library entry succeeds; an untrusted entry stops for the normal approval requirement, and an absent Library source returns an eligibility error without adding it implicitly.

## SKL-GLB-002 - Target profile identity (Revision 1)

**Behavior.** A global deployment target MUST be identified only by Agent plus canonical resolved global Skill root. skilload MUST expose an opaque local `profile_id` together with the resolved path. Effective `HOME`, Agent-root environment values, optional executable path for a removal-only plan, Agent-specific configuration, compatibility/conflict roots, and their environment fingerprint MAY be retained as current observations, but they MUST first satisfy `SKL-WSP-022`'s absolute environment-path rules and MUST NOT create a second profile when Agent and canonical global root are unchanged. Equal Agent names with different canonical global roots are distinct targets.

**Acceptance.** Two Claude configurations with different resolved global roots receive different profile IDs and independent target records even when they run under the same user account. With fixed `HOME`, changing only `CODEX_HOME` keeps the same Codex profile ID and `$HOME/.agents/skills` target while refreshing compatibility/conflict observations.

## SKL-GLB-003 - Durable desired state (Revision 1)

**Behavior.** Global desired state MUST be stored in the local durable database, separate from Library membership, workspace files, cache, and export. It records source, shared pin, target profiles, status, and ownership needed to reconcile links.

**Acceptance.** Library export and workspace files contain no global target. Removing a Library entry or clearing cache leaves the global desired record queryable.

## SKL-GLB-004 - One shared source pin (Revision 1)

**Behavior.** One global source MUST have one resolved commit and integrity shared across all of its active target Agent profiles. Installing that source to another profile reuses the existing pin. It MUST NOT independently drift by profile.

**Acceptance.** Listing a source deployed to Claude and Codex shows one commit/integrity plus two targets. A version change either switches both targets or neither.

## SKL-GLB-005 - Install and uninstall commit desired state with links (Revision 1)

**Behavior.** `global install` and normal `global uninstall` MUST atomically change durable desired state and verified managed links for every requested target. Multi-Skill or multi-Agent normal completion MUST be all-or-nothing. Uninstall MUST remain available after Trust revocation and, under the removal-only rule in `SKL-WSP-022`, after the selected Agent executable is removed when every selected root remains accessible and every removed link is exactly owned. The inaccessible-profile detach exception is defined by `SKL-GLB-009`; it changes desired state without claiming a target filesystem change.

**Acceptance.** A failed target preflight leaves database and every requested root unchanged. Successful uninstall removes only proven owned links and its selected desired associations; a missing Agent executable alone does not block that exact removal, while an inaccessible, foreign, or drifted link still blocks normal uninstall.

## SKL-GLB-006 - Sync restores without advancing (Revision 1)

**Behavior.** `global sync` MUST reconcile desired target links to the existing shared pin and MUST NOT advance a mutable ref. Cache-miss restoration requires active Trust and exact commit verification. Under `SKL-GLB-008`, each explicit Agent resolves its current profile unless stored profile selection is used.

**Acceptance.** After deleting a verified managed link, sync restores it at the same commit. If upstream moved, the commit remains unchanged.

## SKL-GLB-007 - Atomic update and historical pin (Revision 1)

**Behavior.** `global update [selector]` MUST advance mutable refs. With no selector it MUST update all mutable global sources as one atomic batch. `global pin` MUST support a historical commit while retaining mutable source intent. Updating or pinning one source MUST switch all active target profiles for that source atomically; SHA sources return `already_immutable` for update.

**Acceptance.** Failure in any selected source/target leaves all pins and links unchanged. A successful historical pin changes every target for that source, and a later update may advance it again.

## SKL-GLB-008 - Profile selection semantics (Revision 1)

**Behavior.** When `global install`, `uninstall`, `sync`, or `status` targets the current environment, it MUST require one or more explicit `--agent` values and resolve the current profile for each selected Agent. These operations MUST also support stored `--profile` and `--all-profiles` selection where applicable; they MUST NOT guess an Agent. `global update` and `global pin` operate on all active targets of each selected source because the version is shared. `--detach-inaccessible` MUST be valid only for `global uninstall` with exactly one stored `--profile <id>` selection, never with current-environment `--agent` or broad `--all-profiles` selection.

**Acceptance.** An uninstall with `--agent claude` changes only Claude's current-environment target association, while one invocation may select both Agents. Omitting Agent/profile selection fails before mutation. A source update cannot be constrained to one of its profiles and reports every target it will switch.

## SKL-GLB-009 - Inaccessible targets and explicit detach (Revision 1)

**Behavior.** If any active target profile required by an install, update, pin, normal uninstall, or selected sync is inaccessible, the operation MUST fail before commit. skilload MUST NOT create a pending version or desired-state divergence for later application. As the only exception, `global uninstall --profile <id> --detach-inaccessible` MAY detach the selected source associations only after standard preflight classifies that one stored profile as inaccessible. The detach MUST perform no target-root write or deletion and MUST NOT claim that the link was successfully inspected; it MUST atomically remove the associations from active desired state, move their exact path/link/source/pin/integrity ownership evidence into durable detached-orphan records, and report that no link was removed. Detached orphans MUST NOT participate in update/pin batches or cache-prune protection, and global reads MUST continue to expose them as warnings. If the profile becomes accessible later, normal uninstall MAY prove and remove the exact orphaned link and clear its detached record; it MUST still refuse a foreign or drifted path.

**Acceptance.** Making one of three active target roots inaccessible causes a source update and normal uninstall to leave the shared pin, associations, and all three links unchanged, with a structured target-specific diagnostic. Explicitly detaching that stored profile removes only its selected active associations, reports `link_removed: false` plus the orphan path, and allows a later update of the two remaining active targets; using the flag on an accessible, current-environment, or all-profiles selection changes nothing.

## SKL-GLB-010 - Exact target ownership protection (Revision 1)

**Behavior.** A global target path occupied by an unowned file, directory, or foreign symlink is a hard failure. skilload MUST never replace or delete it, including during install, sync, update, uninstall, cache clear, or doctor fix.

**Acceptance.** A user-owned target remains untouched after each operation and the diagnostic names the exact path and ownership mismatch.

## SKL-GLB-011 - Conflicts and independent lifecycle (Revision 1)

**Behavior.** A semantic same-name conflict outside the exact target MAY proceed only after confirmation and then MUST report `degraded_name_conflict`; internal duplicates, exact target conflicts, disabled Skills, and `skilload-manager` remain hard failures. Library removal MUST NOT change global state. Trust revoke MUST preserve current links but block update plus every sync/restore action that treats the external content as valid; explicit exact-owned uninstall/cleanup remains available under `SKL-TRUST-006`. Cache clear MUST remove verified managed external links while preserving desired records for a later trusted sync.

**Acceptance.** Each lifecycle action affects only its owned domain. A confirmed semantic conflict is deployed and remains visibly degraded, while a revoked source remains linked but cannot be restored after cache clear until Trust is re-established.

## SKL-GLB-012 - Global deployment scale (Revision 1)

**Behavior.** Global list, status, planning, sync, update, and ownership checks MUST support at least 100 Agent/profile deployment targets without changing atomicity or conflict semantics.

**Acceptance.** A 100-target fixture produces deterministic status and transaction plans within an implementation-defined performance budget and never partially applies a failed batch.

## SKL-GLB-013 - Offline global reads (Revision 1)

**Behavior.** `global list` MUST deterministically report every durable global source's canonical identity, shared pin, active target-profile associations, and detached-orphan warnings without inspecting Agent roots. `global status` MUST remain offline and read-only while joining selected target profiles with observed link, cache, Trust, conflict, accessibility, and detached-orphan state under `SKL-GLB-008` selection rules. A detached orphan MUST be labelled as non-active desired state rather than an installation that will receive updates. Neither command MAY restore content or repair drift.

**Acceptance.** With networking denied, list returns the same source/pin/active-target/detached-orphan projection from any directory. Status for explicitly selected profiles distinguishes healthy, missing, degraded, foreign, inaccessible, detached orphan, cache-missing, and Trust-blocked state without changing database, cache, or links.

## SKL-MGR-001 - Separate built-in manager domain (Revision 1)

**Behavior.** `manager install`, `manager status`, and `manager uninstall` MUST manage a built-in `skilload-manager` Skill. It MUST NOT be represented as a Library entry, Trust record, GitHub source, external cache entry, workspace source, or global external desired record.

**Acceptance.** Manager installation works with an empty Library and offline; Library export, Trust list, and cache info contain no manager source or content.

## SKL-MGR-002 - Embedded Agent-specific assets (Revision 1)

**Behavior.** The repository MUST contain separate Claude and Codex manager Skill assets, and the release build MUST embed them in the skilload binary. Agent-specific frontmatter MAY differ, but the supported skilload command and JSON behavior described by both assets MUST agree.

**Acceptance.** Automated parsing accepts both embedded `SKILL.md` variants and a contract test proves they reference only commands present in `SKL-CLI-001`.

## SKL-MGR-003 - Owned copy and marker (Revision 1)

**Behavior.** Manager installation MUST atomically copy the embedded asset into the selected Agent's global Skill root rather than linking to external cache. It MUST write a skilload ownership/version marker sufficient to distinguish an exact owned install from user content.

**Acceptance.** The installed manager remains after external cache clear. Uninstall removes an exact matching owned copy and refuses a modified or foreign target.

## SKL-MGR-004 - Multi-Agent and profile transaction (Revision 1)

**Behavior.** Manager install, uninstall, and status MUST require one or more explicit Agents and use the same resolved profile identity rules as global deployment. Install MUST pass executable, `skilload` PATH, functional, and target preflight for every selected Agent. Uninstall MAY omit executable and functional probes only under `SKL-WSP-022`'s removal-only rule, while still requiring an accessible root and an exact marker plus payload digest for every selected manager copy. Install and uninstall MUST apply to all selected targets or none. Status remains read-only and reports each selected target independently.

**Acceptance.** A missing Codex executable prevents a requested Claude-plus-Codex install from modifying Claude's manager target, but does not block a Claude-plus-Codex uninstall when both copied targets are accessible and exactly owned. A foreign, modified, or inaccessible manager target leaves that multi-Agent uninstall unchanged. A single-Agent request remains independently valid, while multi-Agent status reports one result per requested Agent without mutation.

## SKL-MGR-005 - Explicit upgrade and drift status (Revision 1)

**Behavior.** Installing a newer embedded manager version over an owned older version MUST require the explicit manager install/upgrade action. Unrelated skilload commands MUST NOT mutate it. `manager status` and doctor MUST report missing, current, older, newer/unknown, modified, or foreign state.

**Acceptance.** Upgrading the skilload binary alone changes no installed manager files; status reports version drift until explicit install completes.

## SKL-MGR-006 - Agent-visible PATH requirement (Revision 1)

**Behavior.** Manager installation MUST verify that the selected Agent can resolve both its own executable and `skilload` through the trusted absolute-directory rules in `SKL-WSP-022`. The manager asset MUST name `skilload` rather than embed the install-time absolute binary path, and its instructions MUST require runtime resolution through the same rule before invoking the resulting absolute executable path. Empty, relative, current-workspace, enclosing-worktree, and external-cache PATH candidates MUST never satisfy install preflight or manager invocation.

**Acceptance.** A PATH without a safe `skilload` causes preflight failure and no target changes. A fake workspace `skilload` reachable through `PATH=.` is neither accepted during install nor invoked by the manager instructions. Moving/upgrading the real binary while preserving safe PATH resolution does not require rewriting the manager asset.

## SKL-MGR-007 - On-demand JSON management (Revision 1)

**Behavior.** The manager Skill MUST instruct the Agent to use stable `--json` CLI operations and MUST NOT directly edit SQLite, workspace files, ownership state, or configuration. skilload MUST NOT inject the user's Library, workspace, or other dynamic context into the manager asset; the Agent queries current state on demand. The skilload binary MUST NOT implement a natural-language interpreter: the installed Agent and manager Skill translate user language into explicit CLI calls.

**Acceptance.** The installed asset contains no serialized user data or local path. Its management scenarios resolve through documented JSON commands and confirmation tokens.

## SKL-MGR-008 - Reserved install name (Revision 1)

**Behavior.** `skilload-manager` MUST be reserved across workspace and global external deployment. An external source with that verified name MAY be stored in Library for reference but MUST NOT be deployed by skilload.

**Acceptance.** Library add succeeds after normal approval, while workspace add/sync and global install reject deployment with a structured reserved-name error.

## SKL-MGR-009 - Automated acceptance, optional model smoke (Revision 1)

**Behavior.** Release acceptance MUST test manager asset parsing, embedded versioning, install/upgrade/uninstall ownership, Agent variants, and every referenced command/JSON contract. A real Agent/model conversation MAY run as a nonblocking smoke test but MUST NOT be a release gate.

**Acceptance.** The release suite passes with no network or model access and fails if an asset references a nonexistent command or invalid JSON workflow. Optional live smoke results are reported separately.
