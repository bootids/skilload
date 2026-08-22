# JSON API Version 2 Schema Catalog

Status: current normative field-level contract for `SKL-CLI-004`, `SKL-CLI-005`, `SKL-CLI-006`, and `SKL-CLI-012` in the 0.1 CLI MVP.

This catalog is part of the product specification. It fixes the API-v2 fields that 0.1.x patch releases must preserve. Domain behavior remains owned by the behavior IDs in the other product specifications; this file defines how each outcome is represented for machine clients and the built-in manager Skill.

## Schema Rules

The notation `field: Type` means the field is required. `field?: Type` means a producer may omit it. `T | null` means the field is required and may contain JSON `null`. Objects are closed for the initial 0.1.0 producer schema, but version-2 consumers MUST ignore unknown fields so later 0.1.x releases can add optional fields under `SKL-CLI-012`. A producer MUST NOT omit a required empty array, replace a required nullable field with omission, or emit an undocumented enum value under API version 2.

`String` is a valid Unicode JSON string. `Bool` is a JSON boolean. `UInt` is a JSON integer from 0 through 9,007,199,254,740,991 so common IEEE-754 consumers preserve it exactly. An unsigned 64-bit domain value uses `DecimalU64`, a string matching `0|[1-9][0-9]*` whose numeric value is at most 18,446,744,073,709,551,615. `Timestamp` is an RFC 3339 UTC string with a trailing `Z`. `Sha` matches `[0-9a-f]{40}`. `Integrity` matches `sha256:[0-9a-f]{64}`. `OpaqueId` and confirmation tokens are nonempty strings whose internal format clients MUST NOT parse.

All arrays are deterministically ordered. Sources sort by `source.canonical` bytes; native paths sort by decoded `bytes_base64` bytes; Agents sort `claude` before `codex`; profiles sort by Agent then root bytes then `profile_id`; Library tags sort by their `SKL-LIB-008` comparison key; findings sort by severity (`error`, `warning`, `info`), code, then stable identity; actions sort by scope, target path bytes, source canonical bytes, and kind. Network attempts sort by source canonical bytes and then their contiguous per-source `attempt_index`. Candidate locators sort by kind (`repository`, `ref`, `source`), then owner, repository, ref kind/value, and path bytes with null before a value.

## Envelopes and Outcomes

Every JSON command writes exactly one of these objects:

    SuccessEnvelope {
      api_version: 2,
      operation: Operation,
      ok: true,
      result: {
        outcome: Outcome,
        data: OperationData
      }
    }

    ErrorEnvelope {
      api_version: 2,
      operation: Operation,
      ok: false,
      error: {
        code: ErrorCode,
        message: String,
        details: ErrorDetails
      }
    }

`Outcome` is exactly `observed`, `changed`, `unchanged`, `already_exists`, or `already_immutable`. Read-only operations and an explicit non-mutating dry-run use `observed`. A mutation uses `changed` only when product state or owned derived state committed. Without a write, an add/install/Trust-add uses `already_exists` only when every requested member already exists, and workspace/global update uses `already_immutable` only when its nonempty selection consists entirely of SHA sources. Every other no-write mutation uses `unchanged`. For a mixed batch, any committed member makes the whole atomic result `changed`; otherwise the preceding all-members rules apply. The operation table below narrows the allowed values for each command.

`Operation` is exactly one dotted identifier in the operation table. `skilload --help`, no-argument help, and `skilload --version` are text-only meta invocations, have no operation identifier, and reject `--json` with the normal usage exit before any JSON command is dispatched.

## Common Records

    PathValue {
      display: String,
      bytes_base64: String
    }

`PathValue` follows `SKL-CLI-004`: `bytes_base64` is padded standard RFC 4648 base64 over exact native path bytes and is authoritative; `display` is the terminal-safe encoder output without outer quotes.

    SourceIdentity {
      canonical: String,
      owner: String,
      repository: String,
      repository_display: String,
      path: String,
      ref_kind: "branch" | "tag" | "commit",
      ref_value: String
    }

`owner` and `repository` are canonical lowercase identity components. `repository_display` is fresh GitHub metadata spelling. `path` is the normalized repository-relative Skill path and is `""` for a root Skill. `ref_value` is fully qualified for a branch/tag and a full lowercase SHA for a commit.

    SourceLocator {
      kind: "repository" | "ref" | "source",
      canonical: String | null,
      owner: String,
      repository: String,
      repository_display: String | null,
      path: String | null,
      ref_kind: "branch" | "tag" | "commit" | null,
      ref_value: String | null
    }

`SourceLocator` represents identity before every component needed by `SourceIdentity` is known. `path: null` means candidate discovery has not selected a Skill path; `path: ""` means the selected Skill is the actual repository root. The ref fields are both null or both non-null. A `repository` locator has null path/ref/canonical fields, a `ref` locator has a selected ref but a null path/canonical, and a `source` locator has a selected path and ref plus the exact non-null canonical string. `repository_display` remains null until metadata supplied it. A malformed input that cannot establish normalized owner/repository belongs in `UsageDetails`, not a fabricated locator.

    SourceLimits {
      max_files: DecimalU64,
      max_bytes: DecimalU64
    }

    FetchBudget {
      max_pack_bytes: UInt,
      max_objects: UInt,
      max_seconds: UInt,
      invocation_max_pack_bytes: UInt,
      invocation_max_objects: UInt,
      invocation_max_seconds: UInt
    }

`FetchBudget` reports the fixed `SKL-SRC-016` ceilings, not remaining counters: `268435456`, `250000`, `120`, `1073741824`, `1000000`, and `600` in field order.

    CacheQuota {
      configured_limit_bytes: DecimalU64,
      effective_limit_bytes: DecimalU64,
      projected_bytes: DecimalU64,
      stable_quarantine_bytes: DecimalU64,
      repair_headroom_bytes: DecimalU64,
      override_applied: Bool
    }

`stable_quarantine_bytes` is the retained quarantine allocation in the post-recovery stable projection. `repair_headroom_bytes` is zero when no serialized corrupt-object repair is planned; otherwise it is the planned temporary allowance, never more than that object's allocated bytes plus 16,777,216.

    NetworkAttempt {
      source: SourceIdentity,
      attempt_index: UInt,
      stage: "metadata" | "content",
      transport: "api" | "https" | "ssh",
      credential: "none" | "gh_token" | "github_token" | "gh_cli" | "git_helper" | "ssh_agent",
      outcome: "succeeded" | "failed"
    }

    NetworkUse {
      used: Bool,
      attempts: NetworkAttempt[]
    }

Offline results require `used: false` and `attempts: []`; `used` is true exactly when the array is nonempty. For each source, `attempt_index` starts at 1 and is contiguous in actual execution order, so a failed HTTPS attempt followed by successful SSH is preserved rather than collapsed. Metadata attempts require `transport: "api"` and a credential from `none`, `gh_token`, `github_token`, or `gh_cli`; content attempts require `https` or `ssh` and respectively use `none`/`git_helper` or `ssh_agent`. Public unauthenticated access uses `"none"`. No field contains credential material.

    ResolvedSkill {
      source: SourceIdentity,
      repository_id: DecimalU64,
      commit: Sha,
      integrity: Integrity,
      name: String,
      description: String,
      entry_count: DecimalU64,
      byte_count: DecimalU64
    }

`ResolvedSkill.entry_count` 与 `ResolvedSkill.byte_count` 都是正 `DecimalU64`。零仅适用于没有任何 resolved Skill 的聚合计数，不能表示单个已验证来源；每个有效来源至少包含其非空 regular `SKILL.md`。

    Warning {
      code: String,
      message: String,
      source: SourceIdentity | null,
      path: PathValue | null
    }

    Conflict {
      kind: "exact_target" | "semantic_name" | "internal_duplicate" | "reserved_name" | "agent_disabled" | "foreign_owned",
      name: String | null,
      agent: "claude" | "codex" | null,
      path: PathValue | null,
      source: SourceIdentity | null
    }

`internal_duplicate` 表示一个 durable domain 内已声明唯一值的冲突。对 Library import 的 alias 冲突，`name` MUST 为冲突 alias，`source` MUST 为被拒绝导入 entry 的 source，`agent` 与 `path` MUST 为 null；它同时覆盖与已有记录及同一 batch 中较早 entry 的冲突。对同一 batch 的 canonical source 重复，`name` MUST 为 null，`source` MUST 为后出现且被拒绝 entry 的 source，`agent` 与 `path` MUST 为 null。


    Profile {
      profile_id: OpaqueId,
      agent: "claude" | "codex",
      root: PathValue,
      accessible: Bool | null
    }

`Profile.accessible` is null when an operation, notably `global.list`, intentionally does not inspect the root; preflight/status results require a boolean observation.

    TargetRef {
      scope: "workspace" | "global" | "manager" | "cache" | "database",
      agent: "claude" | "codex" | null,
      profile_id: OpaqueId | null,
      workspace_instance_id: OpaqueId | null,
      path: PathValue | null
    }

    Action {
      kind: "create" | "replace" | "remove" | "keep" | "repair" | "migrate" | "detach" | "prune" | "restore",
      target: TargetRef,
      source: SourceIdentity | null,
      name: String | null,
      before: String | null,
      after: String | null
    }

`before` and `after` are stable non-path state labels or integrity/version values. Every native location belongs in `target.path` or another declared `PathValue`; these strings MUST NOT encode a filesystem path.

## Library, Trust, and Source Data

    LibraryEntry {
      skill: ResolvedSkill,
      alias: String | null,
      category: String | null,
      tags: String[],
      note: String | null,
      trust_state: "active" | "revoked" | "missing"
    }

    LibraryMutationData {
      source: SourceIdentity,
      entry: LibraryEntry | null,
      changed_fields: ("membership" | "resolved" | "alias" | "category" | "tags" | "note")[],
      network: NetworkUse,
      source_limits: SourceLimits | null,
      fetch_budget: FetchBudget | null,
      cache_quota: CacheQuota | null
    }

For removal, `entry` is the removed pre-mutation record; for every other Library mutation it is the committed record. `changed_fields` is empty only with an idempotent outcome. `library.add` always supplies all three acquisition-policy records, including with `already_exists`; offline removal/metadata mutations require the two limit fields and quota to be `null`. A no-network result uses `network.used: false` without omitting its applicable policy records.

    LibraryEntriesData {
      entries: LibraryEntry[],
      offset: DecimalU64,
      limit: UInt,
      returned: UInt,
      total: DecimalU64
    }

    LibrarySearchData {
      query: String,
      entries: LibraryEntry[],
      offset: DecimalU64,
      limit: UInt,
      returned: UInt,
      total: DecimalU64
    }

Only `library.list` and `library.search` accept pagination. `--limit <COUNT>` is an unsigned decimal integer from 1 through 1,000 and defaults to 100. `--offset <COUNT>` is an unsigned 64-bit decimal integer and defaults to 0. The operation first computes its complete deterministic matching order, then skips `offset` entries and returns at most `limit`; an offset at or beyond `total` succeeds with an empty array. `offset` and `limit` echo the active request, `returned` equals the array length, and `total` is the complete matching count before paging. Repeating the same query and page against unchanged data returns the same ordered entries and metadata.

    PortableLibraryEntry {
      skill: ResolvedSkill,
      alias: String | null,
      category: String | null,
      tags: String[],
      note: String | null
    }

    LibraryRefreshData {
      entries: LibraryEntry[],
      changed_sources: SourceIdentity[],
      network: NetworkUse,
      source_limits: SourceLimits,
      fetch_budget: FetchBudget,
      cache_quota: CacheQuota
    }

    LibraryExportData {
      format_version: 1,
      entries: PortableLibraryEntry[]
    }

    LibraryImportData {
      format_version: 1,
      dry_run: Bool,
      added: SourceIdentity[],
      updated: SourceIdentity[],
      kept: SourceIdentity[],
      conflicts: SourceIdentity[]
    }

`PortableLibraryEntry` deliberately omits `trust_state`; importing its resolved evidence never creates Trust or authorizes deployment. A dry-run requires `dry_run: true` and `observed`; a committing import requires `dry_run: false` and `changed` or `unchanged`.

    TrustRecord {
      source: SourceIdentity,
      repository_id: DecimalU64,
      state: "active" | "revoked",
      approved_skill: ResolvedSkill,
      approved_at: Timestamp
    }

    TrustMutationData {
      record: TrustRecord,
      network: NetworkUse,
      source_limits: SourceLimits | null,
      fetch_budget: FetchBudget | null,
      cache_quota: CacheQuota | null
    }

`trust.add` always supplies the acquisition-policy records, including for an already-active record; offline revoke requires them to be `null`. A no-network result uses `network.used: false`.

    TrustRecordsData {
      records: TrustRecord[],
      total: DecimalU64
    }

    SourceMigrationData {
      old_source: SourceIdentity,
      new_source: SourceIdentity,
      repository_id: DecimalU64,
      library_records_changed: DecimalU64,
      trust_records_changed: DecimalU64,
      global_records_changed: DecimalU64,
      workspace_impacts: PathValue[],
      network: NetworkUse
    }

## Workspace Data

    WorkspaceSource {
      source: SourceIdentity,
      locked: Bool,
      stale: Bool,
      commit: Sha | null,
      integrity: Integrity | null,
      name: String | null
    }

    WorkspaceSummary {
      root: PathValue,
      workspace_instance_id: OpaqueId | null,
      config_path: PathValue,
      lock_path: PathValue,
      config_format_version: UInt,
      lock_format_version: UInt,
      sources: WorkspaceSource[]
    }

    WorkspaceTarget {
      agent: "claude" | "codex",
      root: PathValue,
      status: "healthy" | "missing" | "stale" | "degraded_name_conflict" | "foreign_exact" | "drifted_owned" | "disabled" | "inaccessible" | "tracked_local_manifest" | "relocation_required" | "duplicate_workspace_instance" | "cache_missing" | "cache_corrupt" | "trust_blocked" | "recovery_pending",
      conflicts: Conflict[]
    }

    WorkspaceRelocation {
      old_workspace: PathValue,
      current_workspace: PathValue,
      workspace_instance_id: OpaqueId,
      required_agents: ("claude" | "codex")[]
    }

    WorkspaceData {
      workspace: WorkspaceSummary,
      targets: WorkspaceTarget[],
      relocation: WorkspaceRelocation | null,
      actions: Action[],
      network: NetworkUse,
      source_limits: SourceLimits | null,
      fetch_budget: FetchBudget | null,
      cache_quota: CacheQuota | null
    }

Read operations require `actions: []`, offline `network`, and null limit/quota fields. `relocation` is non-null only when local manifest plus durable evidence prove one `SKL-WSP-023` relocation candidate; it then reports the old canonical path, exact current path, matched instance, and complete recorded Agent set needed by `workspace sync --rebind-from`. A relocation-required target without that complete proof is an invalid producer state rather than a partial relocation object. Every non-relocation result requires null. A completed delete returns the pre-deletion `workspace` record as evidence. Mutation results return the complete selected action set, including `keep` actions, so atomic batches are inspectable.

`workspace.add`, `workspace.lock`, `workspace.update`, `workspace.pin`, and `workspace.sync` require non-null `source_limits`, `fetch_budget`, and `cache_quota` even when verified cache makes `network.used: false`; every other workspace operation requires those three fields to be null.

## Global and Manager Data

    GlobalTarget {
      profile: Profile,
      status: "healthy" | "missing" | "degraded_name_conflict" | "foreign_exact" | "drifted_owned" | "disabled" | "inaccessible" | "cache_missing" | "cache_corrupt" | "trust_blocked" | "recovery_pending",
      conflicts: Conflict[]
    }

    DetachedOrphan {
      profile: Profile,
      source: SourceIdentity,
      path: PathValue,
      link_removed: false,
      orphan_recorded: true,
      reason: String
    }

    GlobalSource {
      skill: ResolvedSkill,
      active_targets: GlobalTarget[],
      detached_orphans: DetachedOrphan[]
    }

    GlobalData {
      sources: GlobalSource[],
      actions: Action[],
      network: NetworkUse,
      source_limits: SourceLimits | null,
      fetch_budget: FetchBudget | null,
      cache_quota: CacheQuota | null
    }

`global.install`, `global.sync`, `global.update`, and `global.pin` require non-null `source_limits`, `fetch_budget`, and `cache_quota` even when no network is needed; `global.uninstall`, `global.list`, and `global.status` require those three fields to be null.

    ManagerTarget {
      profile: Profile,
      target: PathValue,
      status: "missing" | "current" | "older" | "newer_unknown" | "modified" | "foreign" | "inaccessible",
      embedded_version: String,
      installed_version: String | null,
      asset_digest: Integrity | null
    }

    ManagerData {
      targets: ManagerTarget[],
      actions: Action[]
    }

Manager status requires `actions: []`. Manager operations are offline and need no redundant `NetworkUse` field.

## Cache, Configuration, and Doctor Data

    CacheInfoData {
      quota: CacheQuota,
      verified_entry_count: DecimalU64,
      verified_bytes: DecimalU64,
      protected_bytes: DecimalU64,
      reclaimable_bytes: DecimalU64,
      quarantine_entry_count: DecimalU64,
      quarantine_bytes: DecimalU64,
      staging_bytes: DecimalU64,
      corruption_count: DecimalU64,
      index_consistent: Bool
    }

    CacheMutationData {
      before: CacheInfoData,
      after: CacheInfoData,
      actions: Action[],
      orphaned_targets: TargetRef[]
    }

    ConfigEntry {
      key: "cache_limit_bytes" | "agents.claude.executable" | "agents.codex.executable",
      configured: Bool,
      value: DecimalU64 | PathValue | null,
      default_value: DecimalU64 | null,
      default_command: "claude" | "codex" | null
    }

    ConfigEntryData {
      schema_version: 1,
      entry: ConfigEntry
    }

    ConfigEntriesData {
      schema_version: 1,
      entries: ConfigEntry[]
    }

Configuration entries sort in the exact order `cache_limit_bytes`, `agents.claude.executable`, `agents.codex.executable`. A cache entry uses `default_value` and null `default_command`; an Agent entry uses `default_command` and null `default_value`.

    DoctorFinding {
      severity: "error" | "warning" | "info",
      code: String,
      message: String,
      source: SourceIdentity | null,
      target: TargetRef | null,
      fixable_offline: Bool,
      fixed: Bool
    }

    DoctorData {
      fix_requested: Bool,
      findings: DoctorFinding[],
      actions: Action[],
      database_writable: Bool
    }

Default doctor requires `fix_requested: false`, every `fixed: false`, and `actions: []`. `doctor --fix` may use `changed` only when at least one listed action committed; otherwise it uses `unchanged`.

`database_writable` 表示当前二进制是否会因 observed database generation 的分类而拒绝 durable Library mutation；它不是操作系统权限探针，也不是每个 doctor finding 的反值。仅 default-doctor FTS diagnostic snapshot budget 超限时，字段仍为 true：该资源边界使 doctor 无法完成 offline FTS diagnostic，但不单独阻止 mutation 在其实际 write transaction 中完成完整 integrity gate。

## Operation Schema Map

Every non-meta command leaf appears exactly once below. `Confirm` means the operation may return `confirmation_required` with `ConfirmationRequiredDetails`; it does not mean every invocation requires approval.

| Operation | Required `result.data` type | Allowed outcomes | Confirm |
| --- | --- | --- | --- |
| `library.add` | `LibraryMutationData` | `changed`, `already_exists` | yes |
| `library.remove` | `LibraryMutationData` | `changed` | yes |
| `library.list` | `LibraryEntriesData` | `observed` | no |
| `library.search` | `LibrarySearchData` | `observed` | no |
| `library.get` | `LibraryEntry` | `observed` | no |
| `library.refresh` | `LibraryRefreshData` | `changed`, `unchanged` | no |
| `library.export` | `LibraryExportData` | `observed` | no |
| `library.import` | `LibraryImportData` | `observed`, `changed`, `unchanged` | no |
| `library.alias.set` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `library.alias.clear` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `library.category.set` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `library.category.clear` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `library.tag.add` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `library.tag.remove` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `library.note.set` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `library.note.clear` | `LibraryMutationData` | `changed`, `unchanged` | no |
| `trust.add` | `TrustMutationData` | `changed`, `already_exists` | yes |
| `trust.get` | `TrustRecord` | `observed` | no |
| `trust.list` | `TrustRecordsData` | `observed` | no |
| `trust.revoke` | `TrustMutationData` | `changed` | no |
| `source.migrate` | `SourceMigrationData` | `changed` | yes |
| `workspace.add` | `WorkspaceData` | `changed`, `already_exists` | yes |
| `workspace.remove` | `WorkspaceData` | `changed` | no |
| `workspace.list` | `WorkspaceData` | `observed` | no |
| `workspace.status` | `WorkspaceData` | `observed` | no |
| `workspace.delete` | `WorkspaceData` | `changed` | no |
| `workspace.lock` | `WorkspaceData` | `changed`, `unchanged` | yes |
| `workspace.update` | `WorkspaceData` | `changed`, `unchanged`, `already_immutable` | yes |
| `workspace.pin` | `WorkspaceData` | `changed`, `unchanged` | yes |
| `workspace.sync` | `WorkspaceData` | `changed`, `unchanged` | yes |
| `workspace.migrate-source` | `WorkspaceData` | `changed` | yes |
| `workspace.migrate-format` | `WorkspaceData` | `changed` | yes |
| `global.install` | `GlobalData` | `changed`, `already_exists` | yes |
| `global.uninstall` | `GlobalData` | `changed` | no |
| `global.list` | `GlobalData` | `observed` | no |
| `global.status` | `GlobalData` | `observed` | no |
| `global.sync` | `GlobalData` | `changed`, `unchanged` | yes |
| `global.update` | `GlobalData` | `changed`, `unchanged`, `already_immutable` | yes |
| `global.pin` | `GlobalData` | `changed`, `unchanged` | yes |
| `manager.install` | `ManagerData` | `changed`, `already_exists` | no |
| `manager.uninstall` | `ManagerData` | `changed` | no |
| `manager.status` | `ManagerData` | `observed` | no |
| `cache.info` | `CacheInfoData` | `observed` | no |
| `cache.prune` | `CacheMutationData` | `changed`, `unchanged` | no |
| `cache.clear` | `CacheMutationData` | `changed`, `unchanged` | no |
| `config.get` | `ConfigEntryData` | `observed` | no |
| `config.set` | `ConfigEntryData` | `changed`, `unchanged` | no |
| `config.unset` | `ConfigEntryData` | `changed`, `unchanged` | no |
| `config.list` | `ConfigEntriesData` | `observed` | no |
| `doctor` | `DoctorData` | `observed`, `changed`, `unchanged` | no |

## Confirmation Preview

Every confirmable operation uses one schema. It must describe the complete atomic plan, not a representative member:

    ApprovalPreview {
      preview_version: 1,
      operation: Operation,
      state_revision: DecimalU64,
      actions: Action[],
      sources: ResolvedSkill[],
      targets: TargetRef[],
      warnings: Warning[],
      source_limits: SourceLimits | null,
      fetch_budget: FetchBudget | null,
      cache_quota: CacheQuota | null
    }

    ConfirmationRequiredDetails {
      preview: ApprovalPreview,
      confirmation_token: String,
      expires_at: Timestamp
    }

All arrays and nullable fields are required. `actions`, `sources`, and `targets` contain every batch member, including unchanged targets that participate in atomic validation. The token binds the canonical encoding of every field. A follow-up invocation uses the same operation identifier; success returns that operation's mapped data type.

## Error Details and Exit Categories

Every error code maps to exactly one required details type and exit category. Codes are stable API values; `message` may improve without becoming machine semantics.

    UsageDetails { argument: String | null, value: String | null, path: PathValue | null, expected: String[] }
    LookupDetails { domain: String, selector: String | null, path: PathValue | null }
    ConfirmationTokenDetails { reason: "invalid" | "expired" | "stale" | "consumed" }
    TrustDetails { source: SourceIdentity, trust_state: "missing" | "revoked" }
    AuthenticationDetails { source: SourceIdentity | null, stage: "metadata" | "content", transport: "api" | "https" | "ssh", credential: "none" | "gh_token" | "github_token" | "gh_cli" | "git_helper" | "ssh_agent" }
    SourceAvailabilityDetails { location: SourceLocator, commit: Sha | null, transports_attempted: ("https" | "ssh")[] }
    CandidateDetails { input: String, candidates: SourceLocator[] }
    LimitDetails { limit_kind: String, measured: DecimalU64, allowed: DecimalU64, source: SourceIdentity | null, source_path: String | null, path: PathValue | null }
    SourceLimitDetails { source: SourceIdentity, measured_files: DecimalU64, allowed_files: DecimalU64, measured_bytes: DecimalU64, allowed_bytes: DecimalU64 }
    ValidationDetails { constraint: String, source: SourceIdentity | null, source_path: String | null, path: PathValue | null }
    PathCollisionDetails { source: SourceIdentity, first_path: String, second_path: String, collision_key: String | null, target_root: PathValue | null }
    EnvironmentDetails { variable: String, path: PathValue | null, reason: String }
    ExecutableDetails { program: String, path: PathValue | null, interpreter_depth: UInt | null, reason: String }
    ConflictDetails { conflicts: Conflict[] }
    WorkspaceIdentityDetails { workspace: PathValue, old_workspace: PathValue | null, workspace_instance_id: OpaqueId | null, reason: String }
    TrackedManifestDetails { workspace: PathValue, manifest: PathValue, index: PathValue, remedy: String }
    AccessDetails { domain: String, target: TargetRef, reason: String }
    BusyDetails { lock_domain: String, waited_ms: UInt }
    BaselineDetails { domain: String, expected_revision: DecimalU64, actual_revision: DecimalU64 }
    QuotaDetails { quota: CacheQuota, required_bytes: DecimalU64 }
    RepairSpaceDetails { object_path: PathValue, required_available_bytes: DecimalU64, observed_available_bytes: DecimalU64 }
    IntegrityDetails { source: SourceIdentity | null, path: PathValue | null, expected: Integrity | null, actual: Integrity | null }
    SchemaDetails { domain: String, found_version: UInt, supported_version: UInt }
    DatabaseCorruptDetails { database: PathValue, backups: PathValue[], recoverable_exports: String[], recovery_procedure: "database-corruption-v1" }
    RecoveryDetails { journal: PathValue, resource: TargetRef | null, reason: String }
    InvalidStateDetails { domain: String, state: String, path: PathValue | null, expected: String[] }

`InvalidStateDetails.expected` 仅包含稳定状态标签、版本或完整性值；若错误涉及 native filesystem location，`path` MUST 携带该位置的 `PathValue`，否则为 null。该可选字段遵守 `SKL-CLI-012` 的 API-v2 演进规则。
    InternalDetails { incident_id: OpaqueId }

`UsageDetails` uses `value` for a logical UTF-8 argument and `path` for a native path argument; at most one is non-null. `LookupDetails` requires exactly one of `selector` or `path` to be non-null: logical selectors remain strings and native filesystem targets use `PathValue`. In `LimitDetails` and `ValidationDetails`, a repository-relative Git path uses `source_path` while a host path uses `path`; a producer MUST NOT place either in the other field. `source_limit_exceeded` always uses `SourceLimitDetails` so both independently active dimensions are present even when only one was exceeded; generic fixed one-dimension limits use `LimitDetails`. `portable_path_collision` requires a non-null `collision_key` and null `target_root`; `filesystem_path_collision` requires the materialization `target_root` and may use null `collision_key` when the host alias rule has no portable textual key.

| Error code | Required details type | Exit |
| --- | --- | --- |
| `usage_error` | `UsageDetails` | 2 |
| `unsupported_argument` | `UsageDetails` | 2 |
| `not_found` | `LookupDetails` | 4 |
| `confirmation_required` | `ConfirmationRequiredDetails` | 3 |
| `confirmation_invalid` | `ConfirmationTokenDetails` | 3 |
| `confirmation_expired` | `ConfirmationTokenDetails` | 3 |
| `confirmation_stale` | `ConfirmationTokenDetails` | 3 |
| `trust_required` | `TrustDetails` | 4 |
| `authentication_required` | `AuthenticationDetails` | 5 |
| `authentication_failed` | `AuthenticationDetails` | 5 |
| `source_unavailable` | `SourceAvailabilityDetails` | 5 |
| `ambiguous_ref` | `CandidateDetails` | 4 |
| `ambiguous_source_url` | `CandidateDetails` | 4 |
| `source_selection_required` | `CandidateDetails` | 4 |
| `discovery_limit_exceeded` | `LimitDetails` | 4 |
| `source_limit_exceeded` | `SourceLimitDetails` | 4 |
| `fetch_limit_exceeded` | `LimitDetails` | 5 |
| `agent_input_limit_exceeded` | `LimitDetails` | 4 |
| `library_input_limit_exceeded` | `LimitDetails` | 4 |
| `invalid_root_skill_name` | `ValidationDetails` | 4 |
| `portable_path_collision` | `PathCollisionDetails` | 4 |
| `filesystem_path_collision` | `PathCollisionDetails` | 4 |
| `invalid_environment_path` | `EnvironmentDetails` | 4 |
| `overlapping_state_roots` | `EnvironmentDetails` | 4 |
| `unsafe_executable_path` | `ExecutableDetails` | 4 |
| `unsupported_interpreter` | `ExecutableDetails` | 4 |
| `executable_not_found` | `ExecutableDetails` | 5 |
| `conflict` | `ConflictDetails` | 4 |
| `agent_disabled` | `ConflictDetails` | 4 |
| `relocation_required` | `WorkspaceIdentityDetails` | 4 |
| `duplicate_workspace_instance` | `WorkspaceIdentityDetails` | 4 |
| `tracked_local_manifest` | `TrackedManifestDetails` | 4 |
| `inaccessible` | `AccessDetails` | 5 |
| `permission_denied` | `AccessDetails` | 5 |
| `busy` | `BusyDetails` | 5 |
| `stale_baseline` | `BaselineDetails` | 4 |
| `cache_quota_exceeded` | `QuotaDetails` | 4 |
| `cache_repair_space_insufficient` | `RepairSpaceDetails` | 5 |
| `integrity_mismatch` | `IntegrityDetails` | 6 |
| `cache_corrupt` | `IntegrityDetails` | 6 |
| `unsupported_entry` | `ValidationDetails` | 4 |
| `validation_failed` | `ValidationDetails` | 4 |
| `schema_newer` | `SchemaDetails` | 6 |
| `migration_required` | `SchemaDetails` | 6 |
| `database_corrupt` | `DatabaseCorruptDetails` | 6 |
| `recovery_blocked` | `RecoveryDetails` | 6 |
| `invalid_state` | `InvalidStateDetails` | 4 |
| `internal_invariant` | `InternalDetails` | 6 |

Exit 0 is reserved for success. Exit 2 is syntax/usage, 3 confirmation, 4 domain validation/precondition, 5 external availability/permission/contention, and 6 integrity/schema/recovery/internal failure. Adding or removing an error code, changing its details type, or reusing it for a different condition requires a new API version; version 2 evolves only through optional fields that existing consumers are already required to ignore.

## Contract Acceptance

The implementation must generate machine-readable schemas or equivalent validator fixtures from one typed source and compare them with this catalog. Coverage tests extract all 50 non-meta command leaves from the parser, require the exact 50 operation identifiers above, validate at least one success document for every allowed outcome/type pair used by a leaf, validate confirmation documents for every `Confirm: yes` leaf, and validate one document per error code. Focused fixtures cover `DecimalU64` at 9,007,199,254,740,992 and 18,446,744,073,709,551,615 plus overflow rejection; both `SourceLimitDetails` dimensions; repository/ref/source locators with null versus empty path; mixed per-source HTTPS/SSH attempt sequences; null and complete relocation evidence; and default, adjacent, and beyond-total Library pages. Manager-asset workflows must validate against the same fixtures. Compatibility tests retain the released 0.1.0 corpus and require every later 0.1.x producer to preserve all required fields, enum meanings, ordering, discriminators, and numeric/string encodings while consumers tolerate newly added optional fields.
