# Agent Adapters and Manager Skill Design

Status: planned design for the 0.1 CLI MVP. It supports `SKL-WSP-015` through `SKL-WSP-025`, `SKL-GLB-*`, and `SKL-MGR-*`.

## Behavior Traceability

* Agent selection, native project paths, link naming, conflict/disable checks, environment identity, visibility, and ownership inspection implement `SKL-WSP-015` through `SKL-WSP-025`.
* Profile identity, shared-pin targets, accessibility, ownership, conflicts, and observed global status implement `SKL-GLB-002` and `SKL-GLB-004` through `SKL-GLB-013`.
* Embedded variants, copied installation, upgrades, PATH validation, on-demand JSON use, reserved naming, and test boundaries implement `SKL-MGR-001` through `SKL-MGR-009`.
* The adapter's refusal to overwrite or broaden permissions supports `SKL-OPS-009` and `SKL-OPS-010`.

## Adapter Contract

Agent-specific filesystem and configuration knowledge belongs behind an `AgentAdapter` port. A representative boundary is:

    pub trait AgentAdapter {
        fn kind(&self) -> AgentKind;
        fn resolve_profile(&self, env: &ProcessEnvironment) -> Result<AgentProfile, AgentError>;
        fn inspect(&self, profile: &AgentProfile, scope: Scope) -> Result<AgentObservation, AgentError>;
        fn validate_skill_name(&self, name: &SkillName) -> Result<(), AgentError>;
        fn manager_asset(&self) -> EmbeddedManagerAsset;
    }

The adapter resolves roots and reports facts. The application layer decides hard failure, confirmation, degraded status, and transaction membership. Filesystem writes go through the transaction/ownership adapter, not arbitrary Agent adapter code.

`AgentProfile` contains Agent kind, executable path, effective `HOME`, relevant config overrides, project root when applicable, canonical global Skill root, compatibility/conflict roots, and a deterministic environment fingerprint. For global and manager scope, the database maps only `(Agent kind, canonical global Skill root)` to the opaque `profile_id`. Executable, HOME, config, compatibility roots, and the fingerprint are replaceable observations used for current preflight and diagnostics; changing them without changing the identity tuple refreshes the same profile instead of creating a second owner for one target.

## Claude Code Adapter

For workspace scope, target:

    <workspace>/.claude/skills/<verified-name>

For global/manager scope, target:

    ${CLAUDE_CONFIG_DIR}/skills/<verified-name>  when CLAUDE_CONFIG_DIR is set
    $HOME/.claude/skills/<verified-name>         otherwise

Preflight resolves the configured `claude` executable, checks target parents, reads effective settings needed to detect disabled/overridden names, and inventories other personal/project/plugin/bundled names when the installed CLI exposes them safely. The product does not depend on Claude's current live reload; success guarantees the next fresh local launch.

Claude currently gives personal Skills precedence over project Skills and namespaces plugin Skills. These are observed facts used for diagnostics, not permission to overwrite. A free exact project target plus a same-name personal Skill is a semantic conflict requiring confirmation and degraded status.

## Codex Adapter

For workspace scope, target:

    <workspace>/.agents/skills/<verified-name>

For new global/manager scope, target the current canonical user root:

    $HOME/.agents/skills/<verified-name>

Current Codex also reads deprecated `$CODEX_HOME/skills`. Include that root, system/admin roots where readable, and other repository roots from current directory to repository root in semantic conflict inspection. Do not deploy new content to the deprecated root. The observation fingerprint includes effective `CODEX_HOME` because it changes compatibility/system/config observations, but a `CODEX_HOME`-only change under fixed `HOME` retains the same Codex profile identity and `$HOME/.agents/skills` deployment root.

Codex may expose two same-name Skills instead of deterministic shadowing. Internal skilload duplicates remain a hard failure; an external duplicate can proceed only with the explicit degraded-conflict contract. Symlinked Skill directories are natively supported, but next-launch remains the product guarantee.

The current vendor evidence is in [`../references/claude-and-codex-skill-discovery.md`](../references/claude-and-codex-skill-discovery.md).

## Executable and Root Preflight

Resolve an Agent executable from a configured nonsecret override or PATH. Record its canonical path for the plan but do not require a numeric version. Run only a bounded, documented noninteractive version/help probe if necessary to distinguish a compatible local CLI; never start an interactive Agent.

For every selected profile:

1. Resolve all target/conflict roots without creating them.
2. Inspect each ancestor with `lstat`; reject a foreign non-directory or unsafe symlinked parent.
3. Verify an existing root and target can be inspected; for a missing root, verify the nearest existing owned/user directory permits creation at commit time.
4. Load supported Agent settings needed to detect disabled names without modifying them.
5. Inventory exact and semantic conflicts, including reserved/bundled names where the Agent exposes them.
6. Return observations with file identities/digests so final commit can detect drift.

No preflight changes settings, creates a directory, enables a Skill, or accepts a target as owned.

## External Skill Links

Workspace and global external targets are directory symlinks to immutable cache-object `payload/` directories, never to the sibling skilload manifest. Use relative symlinks when target and link can be represented portably on the same host; otherwise use an absolute local symlink and record it exactly. Both forms are local derived state and are never committed to workspace configuration.

The transaction engine creates a temporary link, verifies `lstat` type and `readlink` target, then renames it. Ownership requires target path, exact link text/resolved cache object, source/pin/integrity, profile, and transaction ID to match. A link that merely points somewhere under the cache is not automatically owned.

Promotion applies non-writable permissions where supported, and link creation/replacement plus every later mutating use verifies the object manifest and canonical payload integrity. These checks cannot make native Agent reads pass through skilload: after a verified symlink is installed, the Agent dereferences it directly. A same-account edit or disk fault can therefore expose changed bytes until status, doctor, cache info, or a mutating operation next verifies the object. Read-only observers report that mismatch; a mutating use quarantines and performs the one exact refetch flow before it may reuse or relink the object.

Directory name comes from validated `SKILL.md` frontmatter. Alias is display/search metadata only. skilload never rewrites frontmatter for compatibility; Agent-specific unsupported fields may produce warnings.

## Semantic Conflict Inventory

Adapters normalize discovered names according to the Agent's current comparison behavior when known and retain raw display names. Findings identify scope and path/namespace. The application treats:

* exact target occupancy as hard regardless of name;
* internal desired duplicates and `skilload-manager` as hard;
* effective disabled state as hard `agent_disabled`;
* other discoverable same-name sources as confirmable degraded conflicts.

The confirmation token binds the entire conflict set and target baseline. Adding/removing a conflicting source before commit invalidates the approval.

## Global Profile Behavior

For current-environment global operations, each explicit `--agent` resolves that Agent's current profile; one invocation may select both. With no Agent/profile selection, the command fails rather than inferring an Agent from installed executables. `--profile` resolves a stored opaque profile and confirms its current roots still match; `--all-profiles` enumerates every selected stored target. An inaccessible profile blocks operations that include it except the explicit one-profile external-deployment detach, which records that no successful link inspection or deletion occurred. Manager operations have no detach exception.

One `GlobalSourcePin` is referenced by every active target association. Update/pin planning always inventories all active associated profiles even if invoked from one Agent environment; detached-orphan observations are reported but excluded. There is no pending per-profile version. Install to a new profile reuses the shared pin and verified cache object.

## Manager Asset Layout

Store source assets under:

    assets/manager/claude-code/skilload-manager/SKILL.md
    assets/manager/codex/skilload-manager/SKILL.md

Optionally include Agent-specific supporting files below each directory. A build step or Rust `include_dir`-style mechanism embeds exact bytes plus a generated manifest containing manager schema version, product version compatibility, file paths, executable bits if any, and SHA-256 asset digest. The binary does not extract assets until explicit `manager install`.

Both manager variants must:

* name the Skill `skilload-manager`;
* instruct the Agent to invoke `skilload ... --json` by command name;
* forbid direct database/config/lock/ownership edits;
* query Library/workspace/global status on demand rather than embedding context;
* handle `confirmation_required` by showing the preview to the user and sending the returned token only after approval;
* use only commands in `SKL-CLI-001` and respect exact-current-directory workspace behavior.

Agent-specific frontmatter may improve triggering, but instructions and JSON semantics remain contract-tested.

## Manager Installation and Marker

Manager content is copied, not linked. Install stages a complete embedded directory in the target parent and includes a marker such as `.skilload-manager.json`:

    {
      "schema_version": 1,
      "owner": "skilload",
      "manager_version": "<asset-version>",
      "asset_digest": "sha256:...",
      "agent": "claude|codex"
    }

The `asset_digest` covers the embedded payload tree and excludes the generated marker, avoiding a self-referential digest. The durable manager row stores the same digest/profile/target. Ownership requires both the exact marker and the installed payload tree digest (again excluding the marker) to match. A marker alone cannot authorize deletion of modified content. Explicit install over an exact older owned asset is an upgrade transaction; an equal version is unchanged; a newer/unknown or modified target is reported and not overwritten automatically.

Before install, verify the Agent executable and `skilload` resolve through the PATH environment that the local Agent process will inherit. Do not embed either absolute path. Multi-Agent install/uninstall uses the common journal and is all-or-nothing.

## Manager Status and Uninstall

Status compares embedded current digest, durable record, marker, and observed tree and reports current, missing, older, newer/unknown, modified, foreign, or inaccessible. It is offline and read-only.

Uninstall removes only an exact owned manager tree. If user modification changes the digest, refuse destructive cleanup and show the target/evidence. Manager operations never touch Library, Trust, external cache, workspace config/lock, or global external desired state.

## Tests

Use fake Agent executables and isolated HOME/config roots. Adapter contract tests cover default/override roots, deprecated Codex conflict discovery, missing executables, disabled settings, parent symlinks, exact foreign targets, semantic conflicts, environment profile changes, and next-launch fixture discovery. Manager tests parse both embedded assets, compare referenced commands to the CLI schema, validate PATH preflight, exercise multi-Agent failpoints, and prove cache clear cannot remove manager copies. Live model conversations remain optional nonblocking smoke tests.
