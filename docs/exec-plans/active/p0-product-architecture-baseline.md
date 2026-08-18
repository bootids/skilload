---
plan_id: PLAN-0001
branch: codex/p0-product-architecture-baseline
pull_request: https://github.com/bootids/skilload/pull/1
status: active
depends_on: []
---

# Establish the skilload product and architecture baseline

This ExecPlan defines the first authoritative documentation baseline for skilload. When the delivery is complete, a newcomer will be able to understand the CLI MVP's promised behavior, the system boundaries that constrain later implementation, and the technical design chosen to satisfy those promises without relying on chat history or the original handoff. The result is observable by reading the versioned behavior specifications, following their acceptance clauses, and checking that `ARCHITECTURE.md` and the design documents consistently trace back to those behaviors.

This ExecPlan is a living document. As work proceeds, the `Progress`, `Surprises & Discoveries`, `Decision Log`, `Outcomes & Retrospective`, and `Review Conversation Log` sections must be kept current. It must be maintained in accordance with `docs/PLANS.md`.

## Delivery Metadata

This is the repository's first ExecPlan and has no prerequisite delivery. It is a documentation-only P0 baseline: it records product intent and implementation constraints but does not create a Rust workspace, expose commands, or claim that any specified behavior is implemented. The delivery is associated with Draft PR https://github.com/bootids/skilload/pull/1.

## Product Baseline

No authoritative product specification exists at Plan creation time and `ARCHITECTURE.md` is empty. The source material is a completed product interview whose decisions are reproduced in the `Context and Orientation` section so this Plan does not depend on that conversation. This delivery establishes revision 1 of the behavior IDs below in new files under `docs/product-specs/`; it does not provide implementation evidence for them. Every behavior definition must state its user-observable acceptance, and the documents must explicitly label implementation status as planned for the 0.1 CLI MVP rather than delivered.

The target behavior inventory is:

* `SKL-PROD-001` through `SKL-PROD-007` in `docs/product-specs/product-and-release-scope.md`: product purpose and first user, the 0.1 CLI-only boundary, explicit non-goals, supported local Agents and platforms, release version mapping, compatibility and provenance promises, and Apache-2.0 license continuity.
* `SKL-SRC-001` through `SKL-SRC-016` and `SKL-TRUST-001` through `SKL-TRUST-008` in `docs/product-specs/source-and-trust.md`: accepted GitHub source forms, canonical identity, candidate discovery, ref resolution, immutable SHA behavior, complete Skill-directory materialization, symlink/file safety, size limits, canonical integrity, conditional reproducibility, immutable repository identity, rename/transfer migration, private-repository authentication, safe validation, exact Trust binding, approval previews/tokens, cloned-state handling, separation from Library membership, and revocation behavior.
* `SKL-LIB-001` through `SKL-LIB-011` in `docs/product-specs/library.md`: Library identity and metadata, approval on first GitHub add, Trust separation, idempotent add, offline search/list, explicit refresh, removal semantics, metadata mutation, export/import, alias conflicts, and scale.
* `SKL-WSP-001` through `SKL-WSP-027` in `docs/product-specs/workspace.md`: workspace discovery without `init`, first add, nested workspaces, source-only configuration, deterministic lock data, atomic config/lock mutation, Trust and Library effects, lock/update/pin semantics, naming rules, format/source migrations, explicit Agent selection, complete-set deployment, Claude and Codex target paths, cache links, conflict classes, disabled-Agent behavior, Agent preflight, environment identity, next-launch visibility, ownership state, Git exclusion, status/delete semantics, deterministic offline listing, and scale.
* `SKL-GLB-001` through `SKL-GLB-013` and `SKL-MGR-001` through `SKL-MGR-009` in `docs/product-specs/global-and-manager.md`: Library-only global deployment, Trust requirements, target profile identity, one shared pin, atomic desired-state/link updates, source-wide update/pin, profile selection, conflicts, Trust/cache interactions, offline list/status projections, target scale, built-in manager ownership, embedded Agent-specific copies, explicit upgrades, PATH checks, on-demand JSON use, reserved name, and automated acceptance without a model gate.
* `SKL-CACHE-001` through `SKL-CACHE-010` and `SKL-OPS-001` through `SKL-OPS-010` in `docs/product-specs/cache-and-operations.md`: immutable cache identity, prune protection, quota behavior, clear semantics, corruption recovery, read-only cache information, read-only and fixing doctor modes, crash journals, concurrency locks, cache-miss recovery, XDG placement, durable database contents, migration/corruption handling, lazy creation, configuration, credential handling, network boundaries, logging/telemetry, privilege boundaries, and the local threat model.
* `SKL-CLI-001` through `SKL-CLI-012` in `docs/product-specs/cli-contract.md`: the canonical command tree, no-argument help, no aliases or placeholders, versioned JSON envelope, structured errors and exit codes, noninteractive JSON confirmation, idempotent outcomes, missing-target outcomes, English human output, explicit metadata commands, configuration schema handling, and network-free read operations.

Revision 1 acceptance for this documentation delivery is that every ID appears exactly once as a definition heading in its stated product specification; each definition contains a normative behavior statement, explicit acceptance examples or conditions, and status/scope metadata; all confirmed decisions in `Context and Orientation` map to at least one ID; and no architecture or design document changes the user-visible semantics defined by those IDs. Later implementation Plans must select the exact IDs and revisions they implement and provide runtime or test evidence. This Plan's own acceptance is documentation integrity, not executable product behavior.

## Design and Architecture Inputs

`ARCHITECTURE.md` must become the short, authoritative map of repository structure, dependency direction, ownership boundaries, and cross-cutting invariants. It must identify a Rust Cargo workspace with a core/application library crate and a CLI binary crate, even though this delivery does not create those directories. Product specifications remain authoritative for behavior; the architecture must link to them rather than restating acceptance in altered language.

The design documentation must explain the chosen implementation approach and rationale in five focused files: `docs/design-docs/application-and-persistence.md`, `docs/design-docs/github-resolution-and-integrity.md`, `docs/design-docs/deployment-transactions-and-recovery.md`, `docs/design-docs/agent-adapters-and-manager.md`, and `docs/design-docs/cli-json-and-release.md`. Reusable external facts about native Agent Skill discovery, GitHub repository identity/authentication, and the observed `npx skills` installation model must be captured separately under `docs/references/` with primary sources and dates. Design documents may cite those references but cannot turn environment facts into product promises.

The dependency direction is inward: CLI parsing and Agent/Git/SQLite/filesystem adapters may call application services; application services coordinate domain operations and ports; domain rules do not import CLI, database, Git process, Agent-specific, or presentation code. All mutating commands use a single application layer, so future TUI and Web adapters can be added without bypassing policy or persistence ownership. No future UI code or stub implementation is part of this delivery.

## Purpose / Big Picture

skilload is a local, single-binary manager for GitHub-hosted Agent Skills. Its first useful release lets one developer maintain a searchable local Library, approve exact sources, pin reproducible workspace and global deployments, and make selected Skills discoverable by Claude Code and Codex through their native directories. It also installs a built-in manager Skill so either Agent can manage skilload by invoking the stable JSON CLI. GitHub remains the source of Skill content; durable local state contains references, trust, metadata, desired state, and integrity records, while removable content lives only in cache.

The repository currently has governance documents but no product specification, technical design, implementation, or nonempty architecture. This P0 delivery turns the fully settled product design into authoritative repository documents. It deliberately stops before code so later ExecPlans can select stable behavior revisions rather than reconstructing product intent.

## Progress

- [x] (2026-08-18 03:25Z) Confirmed a clean, up-to-date `main`, installed the repository mise environment, verified GitHub authentication, inspected the empty documentation baseline, and created `codex/p0-product-architecture-baseline`.
- [x] (2026-08-18 03:29Z) Committed and pushed this `plan`-status ExecPlan, opened Draft PR https://github.com/bootids/skilload/pull/1, and recorded its canonical URL; awaiting a later explicit execution trigger.
- [x] (2026-08-18 03:32Z) Received explicit execution authorization, passed branch/Plan/dependency/Draft-PR preflight, and moved the Plan to `active`; next is external-fact verification and document authoring.
- [x] (2026-08-18 03:50Z) Verified current Claude/Codex discovery, GitHub identity/authentication, and `skills` 1.5.22 implementation facts against primary sources; captured three dated reference documents.
- [x] (2026-08-18 03:50Z) Authored the initial 119 revision-1 behaviors across seven product specifications plus the index, populated `ARCHITECTURE.md`, and authored all five planned technical designs; the traceability pass then identified license continuity and three unspecified read-command projections, expanding the final inventory to 123.
- [x] (2026-08-18 04:15Z) Completed the traceability and validation pass: 123 unique revision-1 behaviors each have one behavior and acceptance block; all expected ranges, relative links, command trees, ASCII checks, removed-command checks, and whitespace checks pass; the complete authored document set was manually reviewed.
- [ ] After implementation of this documentation delivery, acceptance, synchronization, and retrospective are committed and pushed, run `gh pr ready`, verify `isDraft: false` and the expected head SHA, then automatically enter `review` and push the status commit.
- [ ] After a later explicit human merge prompt, pass preflight, complete and push the Plan, merge the PR, return to updated `main`, and delete the local delivery branch.

## Surprises & Discoveries

- Observation: the original handoff described wrapper-launched Agents, temporary runtime mounts, Collections, TUI, and Web as initial scope, but the completed product interview explicitly removed all of those from the 0.1 CLI MVP.
  Evidence: the settled command surface has no `claude`, `codex`, `tui`, `web`, `collection`, or `init` command and instead uses persistent workspace/global native Skill links plus a separately installed manager Skill.
- Observation: the checked-in repository contains only governance scaffolding; `ARCHITECTURE.md` is empty and both product and design directories contain only `.gitkeep` files.
  Evidence: `git ls-files` on 2026-08-18 listed no product specification, design document, reference, source manifest, or implementation file.
- Observation: current Codex documentation names `$HOME/.agents/skills` as the personal Skill root, while current Codex source still reads `$CODEX_HOME/skills` as a deprecated compatibility location.
  Evidence: `docs/references/claude-and-codex-skill-discovery.md` cites the official Skill page and Codex commit `5ee6baee2fcc0b6ffd413d9611f5538dad40d0f2`, whose `host_roots.rs` loads both roots and labels the config-root location deprecated.
- Observation: current Claude and Codex releases can detect many Skill changes without restart, but both retain restart qualifications and Codex symlink reload behavior has varied.
  Evidence: the official Agent Skill pages referenced on 2026-08-18 describe live detection and recommend restart in qualifying cases; the settled next-launch guarantee remains conservative and valid.
- Observation: npm `skills` 1.5.22 recursively copies full Skill directories and uses a canonical `.agents/skills` copy plus Agent-specific links, but its lock and extraction choices do not meet skilload's identity/integrity contract.
  Evidence: inspected commit `c6f69c631292444cc541ac6d91e2226b0ff247da` stores a GitHub folder tree SHA rather than skilload's repository-ID/commit/canonical-digest tuple, dereferences symlinks, leaves LFS pointers, and does not initialize submodules.
- Observation: the traceability pass found the confirmed Apache-2.0 continuity promise only in Plan context and release-design prose, with no authoritative product behavior ID.
  Evidence: before `SKL-PROD-007` was added, the product-specification inventory ended at `SKL-PROD-006` even though the Context and release archive design required the repository license to continue and ship with artifacts.

## Decision Log

- Decision: treat this request as a complex documentation delivery requiring an ExecPlan and Draft PR before authoring the final documents.
  Rationale: the scope establishes the product's entire behavioral and architectural baseline across multiple authoritative documentation layers, so a reviewable and restartable Plan is required by `AGENTS.md` and `docs/PLANS.md`.
  Date/Author: 2026-08-18 / Codex
- Decision: make the delivery documentation-only and defer all Rust/toolchain scaffolding to later Plans.
  Rationale: P0 must provide stable behavior revisions and boundaries that later implementation Plans can select; mixing executable scaffolding into this PR would weaken that acceptance boundary.
  Date/Author: 2026-08-18 / Codex
- Decision: split product specifications by user-facing domain and design documents by technical concern while keeping `ARCHITECTURE.md` concise.
  Rationale: this matches the repository documentation precedence rules, avoids one unreviewable monolith, and lets future Plans update the smallest authoritative surface without duplicating behavior into design prose.
  Date/Author: 2026-08-18 / Codex
- Decision: preserve external integration facts in `docs/references/` rather than embedding mutable vendor details as architecture invariants.
  Rationale: Agent discovery paths and GitHub authentication behavior are version-sensitive facts; the repository rules require reusable research to be dated, sourced, and separated from product decisions.
  Date/Author: 2026-08-18 / Codex
- Decision: add a dedicated `SKL-TRUST-001` through `SKL-TRUST-008` behavior family instead of compressing Trust into the original source range.
  Rationale: Trust has independently observable approval, binding, import/clone, and revocation semantics. Giving those behaviors their own stable IDs satisfies the repository's atomic-behavior rule without changing any confirmed product decision.
  Date/Author: 2026-08-18 / Codex
- Decision: target `$HOME/.agents/skills` for new Codex global external/manager deployments and inspect deprecated `$CODEX_HOME/skills` only for compatibility and semantic conflicts.
  Rationale: this follows the current official Codex location while preserving the confirmed environment-sensitive profile model and avoiding new writes to a source-labelled deprecated root.
  Date/Author: 2026-08-18 / Codex
- Decision: design source acquisition around a temporary bare Git object database and `ls-tree`/`cat-file`, not a normal checkout.
  Rationale: object-level extraction preserves Git modes and symlink blobs while preventing repository-controlled checkout filters, hooks, LFS smudge, or submodule materialization from executing. It directly supports the hostile-source threat model.
  Date/Author: 2026-08-18 / Codex
- Decision: include explicit source rename/transfer migration in the network-capable operation set.
  Rationale: proving that a proposed GitHub name identifies the immutable numeric repository ID stored with an existing source requires fresh GitHub metadata. Migration is an explicit mutation, so this exception preserves both the confirmed migration semantics and the no-hidden-network rule for reads.
  Date/Author: 2026-08-18 / Codex
- Decision: give Apache-2.0 license continuity its own `SKL-PROD-007` behavior instead of leaving it only in release-design prose.
  Rationale: the confirmed license is an independently observable release promise. A dedicated stable ID makes later packaging acceptance traceable and follows the repository rule that product behavior cannot be defined only by a design document.
  Date/Author: 2026-08-18 / Codex
- Decision: require explicit one-or-more Agent selection when a global command resolves current-environment profiles.
  Rationale: a profile identity includes its Agent, so "current profile" alone cannot select between Claude and Codex. Explicit `--agent` values preserve the confirmed current-environment default roots and multi-Agent behavior without heuristic inference; stored `--profile` and `--all-profiles` remain alternative selectors.
  Date/Author: 2026-08-18 / Codex
- Decision: add domain behaviors for `workspace list`, global list/status reads, and `cache info`.
  Rationale: all three commands were present in the confirmed canonical command tree but lacked an explicit observable projection. `SKL-WSP-027`, `SKL-GLB-013`, and `SKL-CACHE-010` close those specification gaps without adding any command or product surface.
  Date/Author: 2026-08-18 / Codex
- Decision: keep corruption observation read-only and reserve quarantine/refetch for mutating operations that would use the object.
  Rationale: doctor, status, and cache info have an explicit no-mutation/no-network contract. They can verify or report local inconsistency, while a later trusted mutating operation performs the journaled quarantine and single exact refetch required before content is exposed.
  Date/Author: 2026-08-18 / Codex
- Decision: name external global install explicitly in the network-capable operation set.
  Rationale: installing a Library source globally may need to resolve its first shared pin or restore cache content. Treating it as an implicit form of "add" would make the authoritative boundary disagree with the CLI design, while manager install remains explicitly offline.
  Date/Author: 2026-08-18 / Codex
- Decision: keep cache LRU observations in rebuildable operational state and advance recency only on meaningful successful cache/deployment mutations.
  Rationale: immutable cache objects cannot carry mutable access timestamps, and read-only or already-satisfied commands must not rewrite state. A separate monotonic operational index supports deterministic prune order without becoming product truth or violating idempotence.
  Date/Author: 2026-08-18 / Codex
- Decision: accept a GitHub blob URL only when it resolves directly to `SKILL.md`, using that file's parent as the candidate Skill path.
  Rationale: a tree URL naturally names a directory, but an arbitrary blob does not identify which ancestor should be treated as a Skill. Requiring `SKILL.md` provides a deterministic normalization for the confirmed blob-URL input form without heuristic upward search.
  Date/Author: 2026-08-18 / Codex
- Decision: require active exact Trust when doctor recreates an external deployment link from verified cache.
  Rationale: offline availability is not authorization. Rebuilding FTS or ownership metadata does not expose source content to an Agent, but recreating a missing link is a deployment and must not become a bypass around Trust revocation.
  Date/Author: 2026-08-18 / Codex
- Decision: treat HTTPS/SSH as nonpersistent Git acquisition policy rather than part of canonical source identity.
  Rationale: transport choice can vary by machine and carries no repository identity proof. Explicit input supplies the first-attempt hint; otherwise deterministic noninteractive HTTPS-then-SSH attempts allow existing credential mechanisms while portable workspace state remains credential- and machine-independent.
  Date/Author: 2026-08-18 / Codex
- Decision: prove rename/transfer migration by comparing fresh proposed-name metadata with the repository ID stored on the old source.
  Rationale: GitHub may stop redirecting an old path after another repository reuses it. Comparing two current paths would strand a correctly bound source; comparing the proposed name to the immutable stored ID preserves both security and migration availability, while the old path's current result remains a warning.
  Date/Author: 2026-08-18 / Codex
- Decision: bind confirmation tokens to a canonical digest of the complete preview plan, not one representative source.
  Rationale: workspace/global batches and multi-Agent conflicts can contain several sources, targets, warnings, and overrides. Exact whole-plan binding prevents a token approved for a smaller or different operation from authorizing a broadened mutation.
  Date/Author: 2026-08-18 / Codex
- Decision: bind confirmation to a semantic `state_revision` that excludes token bookkeeping and derived-index maintenance.
  Rationale: creating a token must not invalidate its own database baseline. Only committed product-state mutations advance the revision; token consumption occurs atomically with such a mutation, preserving single use without self-invalidating previews.
  Date/Author: 2026-08-18 / Codex
- Decision: serialize GitHub numeric repository IDs as quoted decimal strings in portable JSON/YAML while retaining `u64` internally.
  Rationale: common JSON consumers represent numbers as IEEE-754 doubles and can lose precision above 53 bits. String encoding preserves the immutable identity across languages without weakening the internal validated type.
  Date/Author: 2026-08-18 / Codex
- Decision: store each cache object as a skilload manifest beside an exact `payload/` tree and link Agents only to `payload/`.
  Rationale: internal metadata must not alter source integrity, appear to the Agent, or collide with an upstream filename. A safely encoded physical key also prevents an untrusted Skill path from becoming filesystem path construction input.
  Date/Author: 2026-08-18 / Codex

## Outcomes & Retrospective

The documentation delivery is complete on the active branch. It establishes 123 planned revision-1 product behaviors across seven domain specifications plus their index, a populated `ARCHITECTURE.md`, five focused technical designs, and three dated external-fact references. The final behavior counts are `CACHE=10`, `CLI=12`, `GLB=13`, `LIB=11`, `MGR=9`, `OPS=10`, `PROD=7`, `SRC=16`, `TRUST=8`, and `WSP=27`. The baseline deliberately adds no Rust workspace or executable behavior and makes no implementation claim.

Validation on 2026-08-18 confirmed that every expected ID is defined exactly once at Revision 1 with one normative Behavior and one Acceptance block; product, design, and Plan copies of the 17-line command tree are identical; all relative Markdown link targets exist; the 18 authored/active-Plan documents are ASCII and contain no unresolved decision markers; removed commands appear only in explicit non-goal or rejection context; and `git diff --check` produces no output. A manual traceability review added explicit behavior maps to every technical design, promoted the previously design-only Apache-2.0 promise to `SKL-PROD-007`, specified the three previously unnamed read projections, and reconciled cross-domain network, profile, Trust, cache, source-migration, confirmation, serialization, and ownership details without adding a command surface.

No Cargo, Agent, model, or runtime test was run because this delivery is intentionally documentation-only and the repository has no implementation or test runner. The reusable integration claims are instead backed by the three dated primary-source references. The associated PR is https://github.com/bootids/skilload/pull/1; the remaining work in this execution is only the required commit/push and ready-to-review lifecycle transaction.

## Review Conversation Log

No review conversation has been processed.

## Context and Orientation

The repository root is `/Users/yangxuhui/Projects/Products/skilload` in the current environment. `AGENTS.md` defines documentation precedence and Plan lifecycle. `docs/PLANS.md` defines this Plan's required format and state transitions. `ARCHITECTURE.md` exists but is empty. `docs/product-specs/`, `docs/design-docs/`, and `docs/references/` contain only `.gitkeep` placeholders. There is no application code, Cargo manifest, test runner, or release workflow.

The authoritative product meaning to encode is the following.

skilload's primary wedge is reproducible, workspace-scoped Skill loading for a developer who uses both local Claude Code and local Codex CLI. A Skill is a directory rooted at a required `SKILL.md`. GitHub is the only external content source in the 0.1 MVP. skilload stores no durable copy of external Skill files; it stores them only in a removable immutable cache. The 0.1 release is CLI-only. It excludes a wrapper or Agent launcher, runtime leases, Collections, TUI, Web/HTTP service, daemon, MCP server, marketplace, compiler, semantic conversion, embeddings, cloud sync, remote Agent sessions, shortcuts, and `init`. Future TUI and Web releases remain possible in later 0.x minors, and 1.0 is reserved until both exist and have passed stability testing. Development artifacts use 0.0.x; the complete CLI MVP is 0.1.0; Homebrew stable starts at 0.1.0. The supported hosts are macOS and Linux on arm64 and x86_64.

A source is an exact `owner/repo/path@ref` on `github.com`, bound after validation to GitHub's immutable numeric repository ID. Different refs are different sources. Accepted input includes GitHub HTTPS tree/blob URLs, GitHub SSH repository URLs, and `owner/repo` shorthand. Missing refs resolve to the default branch and are then stored explicitly. A ref may be a branch, tag, or full commit SHA; SHA sources are already immutable and update reports `already_immutable`. A repository URL without an exact path is scanned for candidate `SKILL.md` roots. One candidate can be selected automatically; multiple candidates require explicit selection and JSON returns the candidate set. A root `SKILL.md` makes the whole repository the Skill directory, excluding `.git`.

The full Skill directory is materialized. Regular bytes and executable bits are preserved. Only relative symlinks whose resolved targets stay within that Skill directory are permitted. Submodules and unmaterialized Git LFS pointers are rejected. Default file-count and byte limits protect local resources, with an explicit override. The lock integrity is a canonical SHA-256 tree hash over relative paths, regular-file bytes, executable bits, and allowed symlink targets, and it is recorded with repository ID, commit, and verified Skill name. Reproducibility is conditional: a lock can restore exact content while the commit remains available from GitHub or a previously verified cache entry exists. A rename or transfer of the same numeric repository ID requires explicit source migration; only the owner/repository spelling changes in that operation, while path or ref changes create a new source and new Trust decision.

Fetching is data-only: skilload invokes system Git but never executes repository code, scripts, or hooks. `gh` is optional as a credential source. Credentials are neither prompted for nor stored. Public repository metadata may be queried without authentication subject to GitHub limits. Private source validation requires authenticated GitHub REST or GraphQL metadata through `GH_TOKEN`, `GITHUB_TOKEN`, or authenticated `gh`; SSH Git access alone cannot establish the numeric repository identity. Commit signatures are not required.

Trust is local durable authorization for one exact normalized source and immutable repository ID. The first direct GitHub add to either Library or workspace may fetch into a safe temporary area, validate content, and show source, repository ID, commit, verified name, description, file count, total bytes, and warnings before approval. Trust is separate from Library membership. A cloned `.skilload.yaml` never grants local Trust, and first Trust must be validated online or matched to a previously verified local record. In JSON mode there is no prompt; confirmation uses a short-lived single-use token bound to the action and complete preview plan, including all sources, repositories, commits, targets, overrides, conflicts, database revision, and workspace digest; any relevant state drift invalidates it. Revoking Trust preserves existing managed links but blocks future update, sync, cache-miss restoration, and recovery for that source; removal and uninstall remain allowed.

The global Library stores stable source identity plus optional unique alias, one optional free-text category, normalized deduplicated tags, a free-text note, and derived name/description/repository metadata. Adding an exact already-known source is idempotent `already_exists` and does not refresh metadata. First GitHub add can establish Trust after approval. List and search are offline and use embedded SQLite FTS5 across name, description, alias, tags, category, note, and repository. Refresh is explicit and does not change workspace locks. Removing Library metadata does not revoke Trust, delete cache, remove global deployment, or alter workspace files, and it reports known references. Export/import uses versioned JSON and carries Library metadata only, never Trust, global desired state, local paths, or cache content. Import is atomic and supports dry-run; existing sources are kept by default, alias conflicts fail, and explicit replace affects metadata only. The target is at least 10,000 entries.

There is no workspace `init`. A workspace-dependent command operates only when the current working directory itself contains `.skilload.yaml`; there is no upward search. Global Library, Trust, cache, manager, and global-deployment commands work from any directory. The first successful `workspace add` atomically creates `.skilload.yaml` and `.skilload.lock`, while failure leaves neither. Adding inside a child of an existing workspace is refused unless `--allow-nested` is explicit. A workspace need not be a Git repository.

`.skilload.yaml` contains normalized sources only, with no Library IDs, Agent targets, Trust, or machine paths. `.skilload.lock` contains deterministic resolved commits, repository identities, verified names, and integrity data. Both are committed and machine-managed; comments and hand formatting are not preserved. `workspace add` and `workspace remove` update config and lock atomically but do not write Agent Skill directories. A direct-source add may establish Trust but does not add Library membership. Removing the final Skill leaves empty config and lock files.

`workspace lock` reconciles configuration and lock: it preserves unchanged source commits, resolves new sources, and removes stale entries without advancing existing mutable refs. `workspace update` advances mutable refs and, with no selector, is an atomic all-source batch. `workspace pin` may choose a historical commit while retaining the mutable source so a future update can advance it. A SHA source never advances. Duplicate verified Skill names within a workspace are hard failures with no override. An upstream name change requires explicit acknowledgement of the rename and preflight for conflicts and the reserved manager name. Source rename/transfer migration and schema-format migration are explicit; read, status, and sync never silently rewrite schema.

`workspace sync --agent ...` requires one or more explicit Agents and deploys the entire workspace set to each selected Agent, all-or-nothing for normal command completion. Agent targets are not persisted in `.skilload.yaml`. Claude project links are `.claude/skills/<name>` and Codex project links are `.agents/skills/<name>`. Both point to the same immutable cache materialization. The required frontmatter `name` is the install directory name; Library alias never changes it. An existing exact user-owned target path is a hard failure and is never overwritten. A known semantic name conflict outside that target path may proceed only after confirmation and then yields `degraded_name_conflict`. Internal duplicate names, user-disabled Skills in Agent configuration, exact occupied target paths, and the reserved `skilload-manager` name remain hard failures.

The selected Agent executable must be installed and resolvable. There is no numeric Agent-version gate, but functional preflight checks directories, Agent disable settings, ownership, and conflicts. Deployment identity is tied to current `HOME`, `CLAUDE_CONFIG_DIR`, and `CODEX_HOME`; state records resolved roots and promises apply only to that environment. Changes apply on the next Agent launch, not through hot reload. A git workspace maintains exact owned entries in `.git/info/exclude` and never edits shared `.gitignore`. A git-excluded local manifest plus a global workspace index record exact links, cache targets, Agents, lock digest, and environment. A missing manifest can be reconstructed only when link name, target, commit, and integrity exactly match expected state. Status is read-only and reports registered deployments. Deleting a workspace record succeeds only when config is empty and no managed deployment remains; selected prior Agent deployments must first be synced to remove their links. The supported workspace size target is 200 Skills.

Global deployment is a separate local desired state. Only Library entries with Trust are eligible for a new global install. A deployment target is `(agent, resolved global skills root)` and is exposed through an opaque local profile ID plus its path. A source has one pinned commit and integrity shared across all target profiles; installing it to another profile reuses that pin, and update or pin switches every target profile atomically. Install, uninstall, sync, and status require one or more explicit Agents when they resolve current-environment profiles; stored `--profile` and `--all-profiles` are alternative selectors. Update and pin always cover all targets for the selected source. An inaccessible target makes the entire operation fail; no pending version divergence is introduced. Install/update/uninstall atomically change desired state and links, while sync restores the existing pin without advancing it. An unselected global update advances all mutable global sources as one batch. Exact user-owned targets are hard failures; confirmed semantic conflicts become degraded. Library removal leaves global desired state. Trust revoke leaves current links but blocks future restoration/update. Cache clear removes verified managed global links but preserves desired state for later sync. The scale target is 100 global deployment targets.

There is no wrapper process. `manager install`, `manager status`, and `manager uninstall` manage a built-in `skilload-manager` Skill independently of Library, Trust, cache, and GitHub. The repository contains Agent-specific manager assets that the build embeds into the binary. Installation atomically copies the relevant asset into each requested Agent global Skill directory and records an ownership/version marker. Multi-Agent install or uninstall is all-or-nothing. Upgrades are explicit; unrelated commands do not mutate the manager, and status/doctor reports drift. The selected Agent executable and `skilload` on the Agent-visible PATH are preflight requirements; the manager does not embed an absolute binary path. Agent variants may have different frontmatter but share a command and JSON contract. They inject no Library or workspace context; the Agent calls JSON commands on demand. `skilload-manager` is reserved for deployment, although an external source with that name may be collected in Library. Automated tests validate asset parsing, commands, JSON references, ownership, and install/uninstall. A real model conversation may be an optional smoke test but is not a release gate.

External cache entries are immutable and content-addressed by repository identity, commit, and Skill path. Active verified workspace/global links protect their targets from prune; a lockfile alone does not pin cache content. Before a state mutation exceeds the configured quota, skilload prunes unprotected least-recently-used entries; if capacity remains insufficient, it fails unless explicit override is present. Cache clear preflights and removes verified managed workspace/global links before deleting external content. Inaccessible or mismatched known workspaces block by default; force may leave an orphan or broken link. Library, Trust, workspace config/lock, global desired state, and built-in manager copies survive. On corruption, an entry is quarantined, the same commit is refetched once, and integrity is rechecked; persistent mismatch fails without rewriting a lock.

`doctor` is read-only by default. `doctor --fix` repairs only verifiably owned derived local state and performs no network operation. From any directory doctor checks durable database, Trust, cache, global deployments, manager ownership, and known workspace indexes; it performs deep current-workspace checks only when the current directory exactly contains `.skilload.yaml`. Mutations use a persistent journal so a later command can roll forward or roll back incomplete multi-resource work before making another mutation. True instantaneous atomicity across arbitrary directories is not promised, but normal command success is reported only after all resources commit. Workspace and database locks use a bounded wait followed by structured `busy`; network staging may overlap, but the final commit revalidates its baseline.

The canonical 0.1 command tree is exactly:

    skilload library add|remove|list|search|get|refresh|export|import
    skilload library alias set|clear
    skilload library category set|clear
    skilload library tag add|remove
    skilload library note set|clear
    skilload trust add|get|list|revoke
    skilload source migrate
    skilload workspace add|remove|list|status|delete
    skilload workspace lock|update|pin|sync
    skilload workspace migrate-source|migrate-format
    skilload global install|uninstall|list|status|sync|update|pin
    skilload manager install|uninstall|status
    skilload cache info|prune|clear
    skilload config get|set|unset|list
    skilload doctor [--fix]
    skilload --help
    skilload --version

There are no aliases or stub commands. Invoking `skilload` without arguments displays help. Operations that expose structured output use one versioned JSON stdout envelope with `api_version: 1`; errors are structured and use nonzero exit status; JSON mode never prompts. API version 1 may gain optional fields only. Breaking envelopes require a new API version. Human output is English. Already-satisfied mutations succeed with `unchanged` or `already_exists`; genuinely absent mutation targets return `not_found`. Library metadata changes have only the explicit subcommands in the tree. `source migrate` updates Library, Trust, and global database references for a same-repository rename/transfer, while workspace migration is separate.

Persistent configuration uses an explicitly versioned `config.toml`; unknown fields are errors and no command silently rewrites its schema. A future config migration command is added only when needed. Help, version, and empty read-only queries create no files; the first persistent mutation creates the necessary roots. No operation performs network access except explicit GitHub source add/Trust establishment, refresh, lock, external global install, update, pin, source rename/transfer migration identity verification, and cache-miss restoration. There is no telemetry or update check. Diagnostics go to stderr and no persistent log is written by default; explicit debug logs are redacted. skilload never escalates privilege, changes ownership, or applies broad permissions.

XDG paths separate configuration (`XDG_CONFIG_HOME`), durable data (`XDG_DATA_HOME`), operational state and journals (`XDG_STATE_HOME`), and removable content (`XDG_CACHE_HOME`). One durable embedded SQLite database contains Library metadata, Trust, global desired state, manager ownership, known workspace index, and FTS indexes, but never Skill file content. Database migrations take a backup and then run a transactional forward migration. A newer unknown schema or downgrade refuses writes. Database corruption stops writes and is never silently replaced; doctor may rebuild only derived FTS data and otherwise guides a documented out-of-band backup/export/restore-or-reset procedure rather than exposing a hidden reset command.

The threat model treats remote repositories and project configuration as potentially malicious. It trusts the current operating-system account, local user-controlled files, and an Agent process explicitly started by that user. It does not claim protection from another process running under the same account. All external content is inert data and paths are validated before filesystem writes.

The planned implementation is one Rust binary from a Cargo workspace with a core/application library crate and CLI binary crate. Internal domain/application areas are Library, Trust, workspace, global deployment, cache, source/GitHub, Agent adapters, persistence, and recovery. The repository will later add root `mise.toml`, `rust-toolchain.toml`, and committed `Cargo.lock`. Runtime dependencies are system Git and selected Agent CLIs; `gh` is optional. SQLite and FTS5 are embedded and Node is not a runtime requirement. Distribution is GitHub Releases plus Homebrew beginning at 0.1.0, with `cargo install` as an optional developer path. Release artifacts cover macOS/Linux arm64/x86_64 and include SHA-256 checksums, GitHub artifact attestations, and Homebrew checksums; signing/notarization can come later.

Default tests are offline and use temporary Git repositories, HTTP fixtures, isolated HOME/XDG roots, and fake Agent executables. Real GitHub and real Agent smoke tests are explicit or scheduled. Tests must include failure injection for transaction recovery and conflict ownership. Performance targets cover a 10,000-entry Library, 200-Skill workspace, and 100 global deployment targets. Apache-2.0 remains the repository license.

## Plan of Work

First, create the product specification set. Each file starts with its scope and status, explains normative language, and defines behaviors as `## <ID> - <title> (Revision 1)` headings. Each behavior states the promise, boundaries and error cases, and observable acceptance. Use cross-references instead of copying normative rules between files. Mark every behavior as planned for 0.1 unless it is explicitly a release-policy rule. Add a compact index in `docs/product-specs/README.md` that maps domains and ID ranges to files without becoming a second source of behavioral truth. Remove `docs/product-specs/.gitkeep` once real files exist.

Second, write `ARCHITECTURE.md` as the short map a contributor reads before deeper designs. Describe the planned Cargo workspace and module responsibilities; show an inward dependency diagram; identify authoritative state owners; list invariants for content immutability, Trust separation, no user-file overwrite, explicit migrations, application-service reuse, JSON contract ownership, crash recovery, and network boundaries; describe which later changes require product, architecture, or design updates. Avoid acceptance examples that belong in product specs and avoid detailed library choices that belong in design docs.

Third, create the five focused design documents. `application-and-persistence.md` covers the core/application split, ports and adapters, SQLite/FTS ownership, XDG roots, schemas at a conceptual level, explicit migrations, configuration, locking, and read-only laziness. `github-resolution-and-integrity.md` covers normalization, repository-ID lookup, credential sources, safe Git execution, candidate discovery, filesystem validation, canonical tree hashing, immutable cache layout, Trust preview/token mechanics, and source migration. `deployment-transactions-and-recovery.md` covers workspace/global desired versus derived state, Agent-target plans, ownership manifests, `.git/info/exclude`, journals, staging and commit ordering, rollback/roll-forward, sync/update/pin distinctions, cache clear/prune, and doctor. `agent-adapters-and-manager.md` covers Claude/Codex paths and environment roots, preflight, conflict taxonomy, native symlink deployment, global profiles, manager asset variants, markers, explicit upgrades, and the no-model-gate boundary. `cli-json-and-release.md` covers the exact command tree, application command/query dispatch, JSON envelopes and errors, confirmation-token round trips, human output, network policy, offline and fault-injection testing, cross-platform packaging, attestations, and release/version compatibility.

Fourth, capture three concise reusable references before relying on mutable vendor facts. `docs/references/claude-and-codex-skill-discovery.md` records the verified native project/global Skill directories, environment overrides, symlink support, precedence/collision cautions, official primary sources, versions or access dates, and why skilload uses links. `docs/references/github-repository-identity-and-auth.md` records numeric repository-ID stability, rename/transfer behavior, public/private metadata authentication requirements, SSH limitations, API/CLI credential routes, sources, and date. `docs/references/npx-skills-installation-model.md` records the inspected version, full-directory copy behavior, multi-Agent canonical-copy/symlink pattern, lock limitations, and LFS/submodule cautions, with source-code permalinks. Remove `.gitkeep` files in the design and reference directories after real documents exist.

Finally, perform a traceability pass. Every decision in `Context and Orientation` must resolve to exactly one normative product behavior, and every architecture invariant or design mechanism must cite the behavior IDs it supports. Resolve duplication in favor of product specs. Check that the exact CLI tree is identical everywhere, the wrapper and removed features appear only as non-goals, terminology is consistent (`Skill`, `source`, `Trust`, `Library`, `workspace`, `global deployment`, `manager Skill`, `cache entry`, and `profile`), and no document implies code already exists.

## Concrete Steps

Work from `/Users/yangxuhui/Projects/Products/skilload`. After explicit authorization, first use the `execute-exec-plan` workflow to verify prerequisites, keep the PR Draft, move this file to `docs/exec-plans/active/`, set `status: active`, update `Progress`, commit, and push.

Create and edit all documentation with reviewable patches. The intended resulting files are:

    ARCHITECTURE.md
    docs/product-specs/README.md
    docs/product-specs/product-and-release-scope.md
    docs/product-specs/source-and-trust.md
    docs/product-specs/library.md
    docs/product-specs/workspace.md
    docs/product-specs/global-and-manager.md
    docs/product-specs/cache-and-operations.md
    docs/product-specs/cli-contract.md
    docs/design-docs/application-and-persistence.md
    docs/design-docs/github-resolution-and-integrity.md
    docs/design-docs/deployment-transactions-and-recovery.md
    docs/design-docs/agent-adapters-and-manager.md
    docs/design-docs/cli-json-and-release.md
    docs/references/claude-and-codex-skill-discovery.md
    docs/references/github-repository-identity-and-auth.md
    docs/references/npx-skills-installation-model.md

Use `rg` during authoring to find copied terms and omissions. Once the files exist, run these commands:

    git diff --check
    rg '^## SKL-[A-Z]+-[0-9]{3} .*\(Revision 1\)$' docs/product-specs
    rg -o '^## (SKL-[A-Z]+-[0-9]{3}) ' docs/product-specs \
      | sed -E 's/.*(SKL-[A-Z]+-[0-9]{3}).*/\1/' \
      | sort | uniq -d
    rg 'skilload (claude|codex|tui|web)|skilload (collection|init)( |$)' \
      ARCHITECTURE.md docs/product-specs docs/design-docs
    rg 'skilload (library|trust|source|workspace|global|manager|cache|config|doctor)' \
      docs/product-specs/cli-contract.md docs/design-docs/cli-json-and-release.md
    git status --short
    git diff --stat
    git diff -- ARCHITECTURE.md docs/product-specs docs/design-docs docs/references

The duplicate-ID command must print nothing. The removed-command search may match only clearly labelled historical context or non-goals; rewrite any ambiguous occurrence. Review every command-tree occurrence manually for exact equality. Record concise evidence in this Plan as work proceeds.

Before entering review, update `Outcomes & Retrospective`, all living sections, and the final validation evidence. Commit and push all delivery content while the Plan is `active`. Run `gh pr ready <pull_request>`, then `gh pr view <pull_request> --json isDraft,headRefOid`; require `isDraft: false` and the expected pushed implementation SHA. Only then move this Plan to `docs/exec-plans/review/`, set `status: review`, record the evidence, commit, and push.

## Validation and Acceptance

Acceptance is documentation-based because the repository has no implementation and this Plan must not create one. A reviewer can begin with `docs/product-specs/README.md`, follow each domain link, and find every planned revision-1 behavior defined once with an observable acceptance clause. The reviewer can then open `ARCHITECTURE.md` and determine planned crates, dependency direction, state ownership, and non-negotiable invariants without encountering product semantics that disagree with the specifications. Each design document explains how its mechanisms satisfy cited behavior IDs and clearly distinguishes planned structures from existing files.

The exact accepted CLI surface must be present in `docs/product-specs/cli-contract.md` and `docs/design-docs/cli-json-and-release.md` without wrapper, Collection, TUI, Web, or `init` commands. All references to those removed concepts must label them as non-goals or superseded history. The source/Trust documents must distinguish source identity from repository identity, Trust from Library, persistent state from removable cache, and normal atomic completion from crash recovery. The workspace/global documents must make user-owned path protection and confirmed degraded semantic conflicts unambiguous.

Run `git diff --check`; expect no output. Run the ID extraction and duplicate check from `Concrete Steps`; expect all stated ID ranges and no duplicate definition. Manually compare the extracted inventory to `Product Baseline` and record counts by prefix. Check every relative Markdown link by resolving its target from the referring document; expect no missing targets. Search for temporary decision markers and deferred-decision phrases; expect no unresolved product decision. The only deliberate future work should be explicitly scoped non-goals or later releases.

No Cargo, Node, Agent, GitHub network, or model-level test is part of this documentation-only acceptance. External integration statements must be supported by dated primary-source references. The final diff must contain only this Plan lifecycle record, the requested specifications/architecture/design/reference documents, and removal of obsolete `.gitkeep` placeholders.

## Idempotence and Recovery

Documentation authoring and validation commands are repeatable. Preserve user changes and never delete an unfamiliar file. If research changes a mutable vendor fact without changing product intent, update the corresponding reference and technical design, not the product behavior. If research exposes an actual product decision conflict, stop execution and return the decision to the user rather than silently changing the confirmed baseline.

The initial Draft PR and Plan branch should be reused on rerun. During execution, update this living Plan before every pause. If the work proves too large for one coherent review, return the PR to planning rather than creating a second Plan on the same PR; split only into independently acceptable branches and preserve the overall documentation map.

If `gh pr ready` fails, keep the Plan in `active` and the PR Draft. If the PR becomes ready but the review-status move, commit, or push fails, run `gh pr ready <pull_request> --undo`, verify `isDraft: true`, and restore/keep the Plan in `active` before retrying. If material scope or acceptance rework is found during review, first return the PR to Draft and verify it, then move the Plan from `review` back to `active`, record the reason, and push; if publishing that transition fails, restore the Plan to `review`, mark the PR ready again, and re-verify consistency. After a human merge authorization, if a required check, repeated gate, queue attempt, or merge fails before GitHub reports `MERGED`, restore the Plan to `review`, record the failure, and push so the default branch never presents an unmerged delivery as completed.

## Artifacts and Notes

Planning baseline observed on 2026-08-18:

    $ git status --short --branch
    ## main...origin/main

    $ git rev-list --left-right --count main...origin/main
    0       0

    $ git ls-files
    .gitignore
    AGENTS.md
    ARCHITECTURE.md
    LICENSE
    docs/PLANS.md
    docs/design-docs/.gitkeep
    docs/exec-plans/{active,completed,plan,review}/.gitkeep
    docs/product-specs/.gitkeep
    docs/references/.gitkeep

The completed interview superseded the pasted handoff wherever they disagree. The largest superseded area is Agent launch: the final baseline has no wrapper, temporary runtime session, or injected context. Workspace and global sync use native Agent Skill directories, while the built-in manager is installed separately and calls the CLI on demand.

## Interfaces and Dependencies

This delivery adds no runtime interface or dependency. It specifies planned interfaces so later code Plans can implement them without changing product meaning.

The future repository is a Rust Cargo workspace with a reusable core/application crate and a thin CLI binary. The core/application crate owns domain values and use cases for normalized source identity, repository identity, Trust, Library entries, workspace configuration/locks, global desired state, content integrity, deployment plans, ownership records, and recovery journals. Infrastructure adapters own SQLite/FTS5, filesystem operations, system Git, GitHub metadata access, clocks, process discovery, and Claude/Codex environment inspection. The CLI owns argument parsing and rendering only; it calls the same application commands/queries that future interfaces must call.

The stable external interface planned for 0.1 is the canonical command tree and `api_version: 1` JSON envelope defined in `docs/product-specs/cli-contract.md`. The architecture and designs must not invent hidden database-writing routes for Agent-specific manager assets, future UIs, or tests. Agent integrations are adapters around native filesystem discovery, not semantic converters. External Skill content remains outside the durable database and manager assets remain outside the external cache.

Research uses only primary vendor documentation and inspected source code. No new package or service is required to complete this documentation delivery.

Plan revision note: initial planning baseline created on 2026-08-18 to translate the completed product interview into authoritative repository documentation without beginning implementation. The same day, the Draft PR URL and publication evidence were recorded after the initial push, then explicit human authorization moved the Plan to `active` after a successful delivery preflight. During authoring, Trust received its own behavior-ID family so each authorization rule can be specified atomically. The final traceability pass added stable behaviors for the confirmed Apache-2.0 promise and three previously unspecified read-command projections, then recorded the low-risk technical decisions needed to keep network, profile, Trust, cache, source, confirmation, serialization, and ownership mechanisms internally consistent.
