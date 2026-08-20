# Cache and Local Operations

Status: partially implemented. `PLAN-0002` implements Revision 1 of `SKL-OPS-006` in the development `0.0.1` configuration slice. `PLAN-0005` implements Revision 1 of `SKL-OPS-003` (the backed-up v1→v2 forward migration), the durable-database portions of `SKL-OPS-004` (typed `database_corrupt` diagnostics with backup inventory and FTS-only rebuild), `SKL-OPS-005` for the new read/doctor leaves, and the offline read coverage of `SKL-OPS-008` for `library list`/`search`/`get` and `doctor`. Doctor's cross-domain inspection (`SKL-CACHE-006`) and its future repair surface (`SKL-CACHE-007` beyond database migration/FTS rebuild) remain planned for the 0.1 CLI MVP because Trust, cache, global, manager, and workspace state do not exist yet.

The **cache** contains removable external Skill bytes. Durable metadata and desired state live elsewhere. Operational state contains journals and ownership records needed to recover managed mutations safely.

## SKL-CACHE-001 - Immutable external-content cache (Revision 1)

**Behavior.** skilload MUST manage external Skill materializations as immutable, content-addressed objects keyed by numeric repository identity, commit, and normalized Skill path, with canonical integrity verification. It MUST never edit a promoted object in place and MUST apply non-writable object permissions where the host supports them, while treating those permissions as defense in depth rather than protection from the same operating-system account. The cache MUST contain no built-in manager copy and MUST NOT be the authoritative store for Library, Trust, workspace, or global desired state.

**Acceptance.** Two sources for the same repository/commit/path reuse one verified entry. A local byte, mode, path, or symlink-target change is detected at the next skilload integrity verification and is treated as corruption rather than a new valid form of the object.

## SKL-CACHE-002 - Prune protection follows active managed links (Revision 1)

**Behavior.** `cache prune` MUST protect every verified cache entry currently targeted by a managed workspace or global link. A lockfile, Library entry, or Trust record alone MUST NOT pin cache bytes. Unprotected entries MAY be evicted by least-recently-used order.

**Acceptance.** Prune leaves linked content intact, may remove an unlinked locked entry, and reports protected/freed totals without modifying durable source state.

## SKL-CACHE-003 - Quota enforcement before state commit (Revision 1)

**Behavior.** The Revision 1 cache limit MUST default to 536,870,912 bytes (512 MiB) when `config.toml` is absent or `cache_limit_bytes` is unset. `config set cache_limit_bytes <BYTES>` MUST persist a positive integer no greater than 9,223,372,036,854,775,807 so it remains one native TOML integer, and `config unset cache_limit_bytes` MUST restore the default. At every stable command boundary, quota usage MUST equal the allocated bytes of promoted objects, retained quarantine payloads, and retained cache staging files; manifests and filesystem allocation rounding count, while the rebuildable cache index does not. Before a mutation commits state requiring new cache content, skilload MUST enforce that configured limit after first planning removal of unprotected least-recently-used entries. `projected_bytes` MUST include every retained byte in the complete post-recovery plan, not only the new object's logical content. If capacity remains insufficient, the mutation MUST fail without persistent product state unless the same invocation supplies `--cache-limit-bytes <BYTES>`, a positive unsigned 64-bit effective limit no smaller than the configured value. The flag MUST be accepted only by `library add`, `library refresh`, `trust add`, `workspace add`, `workspace lock`, `workspace update`, `workspace pin`, `workspace sync`, `global install`, `global sync`, `global update`, and `global pin`, because these are the 0.1 operations that may promote external cache content. It applies to the complete invocation and all objects in its preview, is bound into confirmation, and MUST NOT persist in configuration, Trust, Library, workspace, lock, global desired state, or a later invocation. Human and JSON previews/results MUST expose configured, effective, projected post-operation, stable quarantine, temporary repair-headroom, and override-applied values; every API-v1 byte quantity uses lossless `DecimalU64`.

**Acceptance.** With no configuration file, a stable projected total of 536,870,913 bytes, including one retained quarantine or staging byte, fails without an override and leaves no Trust/Library/workspace/global state referring to an unpromoted entry. Repeating the operation with a sufficient `--cache-limit-bytes` value may succeed after confirmation and reports every required `cache_quota` field from the API-v1 catalog; the next invocation uses 536,870,912 again unless configuration was explicitly changed. Persistent value 9,223,372,036,854,775,807 round-trips as a TOML integer and decimal JSON string, its next integer is rejected, and an invocation-only 18,446,744,073,709,551,615 override round-trips without persistence while its next integer is rejected. Near-quota corruption fixtures use the separate bounded repair flow in `SKL-CACHE-005` rather than making the stable projection permanently hold both old and replacement payloads.

## SKL-CACHE-004 - Clear preserves durable intent (Revision 1)

**Behavior.** `cache clear` MUST preflight and remove verified managed workspace/global links before deleting all external cached content. It MUST preserve Library, Trust, workspace config/lock, global desired state, known workspace/profile records, and built-in manager copies. An inaccessible or mismatched known workspace blocks by default; explicit force MAY continue and report orphaned or broken links.

**Acceptance.** A normal clear leaves no managed link pointing into deleted cache. After clear, durable lists and files are unchanged and a later trusted sync can restore them. Force reports every link it could not safely remove.

## SKL-CACHE-005 - Corruption quarantine and one exact retry (Revision 1)

**Behavior.** When a mutating operation would use cached content and its integrity fails, skilload MUST hold the cache mutation lock, journal one repair, rename the exact corrupt object into `cache/quarantine/` without copying it, and attempt at most one refetch of the same repository ID, commit, and path. Only this serialized repair MAY use temporary physical headroom above the effective cache limit, and the headroom MUST equal at most the quarantined object's allocated bytes plus 16,777,216 bytes for pack/index/manifest overhead. Before receiving the replacement, skilload MUST require filesystem-reported available space for the planned replacement plus that overhead; otherwise it returns `cache_repair_space_insufficient` without changing a lock or pin. Temporary bytes and quarantined bytes remain visible in cache info but are not a second stable quota allowance.

After a correct replacement is verified and promoted, skilload MUST delete the full quarantined payload and all repair staging before committing dependent links/state. After a mismatch, unavailable commit, or capacity failure, it MUST preserve the pin, delete replacement staging, retain only a bounded 65,536-byte diagnostic quarantine manifest, and delete the full corrupt payload before returning. Crash recovery MUST complete those same cleanup rules before another cache mutation; it MUST never accumulate a second repair quarantine. Thus every completed/recovered command again satisfies `SKL-CACHE-003`, while the maximum transient cache allocation is the effective limit plus one corrupt object's allocated size plus 16,777,216 bytes. Read-only commands MUST report the mismatch without quarantining or refetching it. skilload MUST verify before promotion, link creation/replacement, or another mutating use, but it does not wrap an Agent or mediate reads through an already deployed native link; a post-deployment local modification can therefore be read by the Agent until the next skilload integrity observation.

**Acceptance.** Once skilload detects a locally modified entry, it never promotes, links, or reuses that entry as valid content. A 500 MiB corrupt object under the 512 MiB default can refetch an exact 500 MiB replacement when the filesystem has the reported temporary headroom, finishes with only the replacement counted, and leaves no full quarantine. Insufficient space or a persistent mismatch leaves the original pin unchanged, retains at most the bounded diagnostic manifest, and reports repair evidence; repeated failures and crash recovery never accumulate full corrupt copies. Doctor/status detect the same mismatch while leaving cache paths unchanged. A test that modifies content after link deployment demonstrates the explicit limitation: a direct Agent filesystem read can see the change before detection, and the next skilload check reports it.

## SKL-CACHE-006 - Read-only doctor coverage (Revision 1)

**Behavior.** `doctor` MUST be read-only by default. From any directory it MUST inspect durable database/schema, Trust consistency, cache integrity/indexes, global deployments, manager ownership, recovery journals, and known workspace/profile indexes. It performs deep current-workspace checks only when the current directory exactly contains `.skilload.yaml`.

**Acceptance.** Running doctor on healthy or broken state creates and modifies no file, database row, link, or network request, while reporting each detected inconsistency with a stable code.

## SKL-CACHE-007 - Verifiable offline doctor fixes (Revision 1)

**Behavior.** `doctor --fix` MUST perform no network access. It MAY repair derived state whose expected value and skilload ownership can be proved locally and MAY apply the supported transactional forward database migration from `SKL-OPS-003` after its required standalone backup; it MUST NOT perform any other product-state or schema rewrite. Recreating an external deployment link from verified cache MUST also require active exact Trust. It MUST NOT rewrite product source-of-truth files to hide corruption, adopt foreign paths, or repair external bytes without their expected verified content.

**Acceptance.** It can rebuild derived FTS data or an exactly verifiable manifest, can migrate an intact supported older database through the backed-up `SKL-OPS-003` path, and can recreate an exact cached link only while Trust is active. It refuses a corrupt/newer database, foreign target, or revoked source and reports that cache-miss restoration requires a separate network-capable sync.

## SKL-CACHE-008 - Recoverable multi-resource mutations (Revision 1)

**Behavior.** Every mutation spanning database/files/cache/link/workspace resources MUST use a persistent journal with enough before/after evidence for the next mutating command to roll forward or roll back safely. Normal success MUST be reported only after every resource commits. skilload does not promise an instantaneous filesystem transaction across directories.

**Acceptance.** Failure injection at each staged step followed by another mutation first recovers to one coherent old or new state, never silently accepts a mixed state, and preserves every unowned path.

## SKL-CACHE-009 - Concurrency and cache-miss restoration (Revision 1)

**Behavior.** Workspace and durable-database mutations MUST use process-safe locks with a bounded wait followed by structured `busy`. Network retrieval MAY stage concurrently, but final commit MUST reacquire/revalidate its baseline. Cache-miss restoration MUST retrieve the exact pin and require active Trust; it MUST not run during read-only operations.

**Acceptance.** Concurrent conflicting mutations yield one commit and one bounded `busy` or revalidation failure, not lost updates. A status/read command never fills a cache miss, while sync may do so at the exact digest.

## SKL-CACHE-010 - Offline cache information (Revision 1)

**Behavior.** `cache info` MUST be read-only and offline. It MUST report the complete API-v1 `CacheQuota`, verified entry count and bytes, protected and reclaimable totals, quarantine entry/byte totals, retained staging bytes, and locally known corruption or index inconsistencies. It MUST NOT verify by refetching, restore a missing object, update recency, prune, or create absent state.

**Acceptance.** In an isolated populated cache with networking denied, info reports totals consistent with manifests, allocated filesystem use, and ownership records, including a pending repair's quarantine, staging, and bounded headroom, while every file and database timestamp remains unchanged. On absent state it returns zero/default values without creating directories.

## SKL-OPS-001 - XDG state separation (Revision 1)

**Behavior.** skilload MUST place user configuration under `XDG_CONFIG_HOME`, durable data under `XDG_DATA_HOME`, operational state/journals under `XDG_STATE_HOME`, and removable content under `XDG_CACHE_HOME`, each with documented platform fallbacks. An XDG home value MUST be used only when it is nonempty and absolute; an unset, empty, or relative value MUST be ignored in favor of its `HOME`-based fallback. `HOME` MUST itself be nonempty and absolute when a fallback is needed, otherwise skilload MUST return a structured environment-path error before filesystem access. After appending the `skilload` application directory and resolving lexical components plus existing symlink aliases, the four effective application roots MUST be pairwise disjoint: no root may equal, contain, or be contained by another. An overlap or a root whose existing prefix cannot be resolved safely MUST return structured `overlapping_state_roots` or `invalid_environment_path` before reading or mutating skilload state. Workspace files remain in the workspace.

**Acceptance.** Isolated XDG integration tests show each file category only in its designated root, and clearing the cache root cannot remove durable data or operational ownership records. Setting `XDG_DATA_HOME=.data` from two different current directories uses the same absolute `HOME/.local/share` fallback and creates no current-directory `.data`; an invalid fallback `HOME` fails before reading or writing state. Setting data and cache homes to the same directory, nesting either effective application root beneath the other, or using different symlink spellings for one effective root fails before state access and leaves database, journals, configuration, and cache untouched.

## SKL-OPS-002 - Durable database ownership (Revision 1)

**Behavior.** One embedded SQLite database MUST own Library metadata, Trust, global desired state, manager ownership records, known workspace/profile indexes, and FTS5 indexes. It MUST NOT store external Skill file bytes, workspace config/lock contents as authoritative copies, credentials, or built-in manager asset bytes.

**Acceptance.** Database schema inspection finds every listed durable domain and no Skill-content blob or secret. Deleting removable cache leaves database rows intact.

## SKL-OPS-003 - Forward database migration (Revision 1)

**Behavior.** Before a database schema upgrade, skilload MUST create a recoverable backup and then apply a transactional forward migration. A database with an unknown newer schema or a requested downgrade MUST refuse writes rather than guessing or rewriting.

**Acceptance.** Fault injection leaves either the prior readable database plus backup or the complete new schema. A newer-schema fixture permits safe diagnostics/export where possible but rejects mutation.

## SKL-OPS-004 - Database corruption handling (Revision 1)

**Behavior.** Suspected database corruption MUST stop writes and MUST NOT silently recreate an empty database. Doctor MAY rebuild derived FTS indexes when base records are provably intact; otherwise `database_corrupt` diagnostics MUST identify the database and known backups as `PathValue`, name every still-readable export, and set `recovery_procedure: "database-corruption-v1"`. The operator MUST follow the normative [database corruption recovery procedure](database-recovery.md), which preserves evidence, validates a standalone migration backup in isolated XDG roots, replaces the database atomically with rollback, or explicitly moves the corrupt database/WAL/SHM set out of the live path before a destructive empty reset. The 0.1 CLI MUST NOT expose an unlisted reset command or adopt surviving links/manifests/cache as ownership after reset.

**Acceptance.** A corrupt fixture never turns into an empty successful Library. Repair of an FTS-only failure preserves base row identities and metadata. Restore fixtures reject a bad digest, newer schema, stale WAL, foreign-key failure, and live-file drift; a valid standalone backup restores atomically and can roll back as one generation. Reset fixtures require explicit removal of the database/WAL/SHM set, preserve portable workspace files, and leave every old deployment/cache artifact unowned until normal Trust and ownership are re-established.

## SKL-OPS-005 - Lazy state creation (Revision 1)

**Behavior.** `--help`, `--version`, doctor/read operations on absent state, and empty list/search/status queries MUST NOT create config, data, state, cache, workspace, or Agent directories. The first successful persistent mutation creates only the roots it needs.

**Acceptance.** Running every read-only command in an isolated HOME leaves the filesystem unchanged. The first Library add does not create workspace or Agent directories.

## SKL-OPS-006 - Explicit versioned configuration (Revision 1)

**Behavior.** `config.toml` MUST have the required schema key `version`; beyond it, the only allowed setting keys are `cache_limit_bytes` from `SKL-CACHE-003`, `agents.claude.executable`, and `agents.codex.executable`. `version` is format metadata, not a configurable setting: get/list MAY report it as `schema_version`, but set/unset MUST reject it. The cache value is a positive TOML integer through 9,223,372,036,854,775,807 and its JSON value/default use API-v1 `DecimalU64`. Each Agent key is optional and, when set through `config set <key> <ABSOLUTE-PATH>`, MUST be a nonempty valid-UTF-8 absolute filesystem path after lexical normalization; it is an override, not a command line, shell fragment, or command name. Setting it MUST NOT execute or require the target, while every later Agent operation MUST resolve and validate the current target under `SKL-WSP-022` before use. When an Agent key is absent or is removed by `config unset <key>`, the default is no override and safe PATH lookup of the fixed basename `claude` or `codex`; unset MUST NOT persist that basename as a configured value. `config get` and `config list` MUST distinguish `configured: false`, `value: null`, and the applicable `default_command` from a configured path, and JSON MUST represent a configured path with `SKL-CLI-004`'s `PathValue`. Unknown fields and unsupported schema versions MUST be errors. `config get|set|unset|list` are the only configuration mutation surface; no command silently migrates its schema.

**Acceptance.** An unknown field prevents dependent mutation without rewriting the file. Setting `cache_limit_bytes` within 1 through 9,223,372,036,854,775,807 changes the persistent effective quota and round-trips through decimal-string JSON; zero, a negative value, or the next integer fails without rewriting, and unset restores 536,870,912 bytes. Setting `agents.claude.executable` to `/opt/claude/bin/claude` round-trips that absolute path, while a relative path or multiword command fails without rewriting configuration; unsetting it returns `configured: false`, `value: null`, and `default_command: "claude"`, after which preflight uses safe PATH lookup. Repeated set/unset is idempotent, and no operation echoes or stores a credential.

## SKL-OPS-007 - Credential handling (Revision 1)

**Behavior.** skilload MUST use only caller-provided environment tokens, authenticated `gh`, and existing Git/SSH credential mechanisms described by source behavior. It MUST NOT prompt for a credential, persist one, include one in JSON/debug output, or copy one into subprocess arguments where it can be avoided.

**Acceptance.** State/export/log scans after private-source use contain no token. Missing credentials return actionable errors without opening an interactive prompt.

## SKL-OPS-008 - Network boundary (Revision 1)

**Behavior.** Network access MUST occur only for explicit GitHub source add/Trust establishment, refresh, lock resolution, external global install, update, pin, source rename/transfer migration identity verification, or cache-miss restoration. List, get, search, status, config, doctor, remove/uninstall, manager install, local cleanup, and help/version MUST be offline. skilload MUST perform no telemetry or automatic update check.

**Acceptance.** Network-deny integration tests pass for every read/cleanup command and observe no attempted connection. A network-capable command reports which source operation required access.

## SKL-OPS-009 - Diagnostics, logs, and privilege boundary (Revision 1)

**Behavior.** Normal diagnostics MUST go to stderr and no persistent log is written by default. Explicit debug logging MUST redact credentials and sensitive URL components. skilload MUST NOT escalate privileges, invoke `sudo`, change ownership, or apply broad permissions; created state uses restrictive current-user permissions appropriate to its content.

**Acceptance.** A normal run creates no log file. Redaction tests cover tokens and authenticated URLs, and filesystem tests find no privilege command, ownership change, or world-writable durable state.

## SKL-OPS-010 - Local threat model (Revision 1)

**Behavior.** skilload MUST treat remote repositories, cloned workspace configuration, repository-scoped Agent settings/conflict roots, and Library import documents from an untrusted project as potentially malicious data. It MAY trust the current operating-system account, user-controlled local files outside an identified untrusted project/source/cache root, and an Agent process explicitly launched by that user. It MUST NOT promote a PATH candidate, indirect script interpreter, Git helper path, Git repository/index selector, dynamic Git configuration, or Git child-process override to trusted local code or state merely because the same account can read or execute it; every Git process and external executable follows `SKL-WSP-022` and `SKL-SRC-016` before any probe, repository inspection, or fetch. It does not claim protection from another process running as the same account. All untrusted names, paths, file entries, metadata, import JSON/workspace YAML structure, Agent-root environment values, and executable locations MUST be bounded and validated before writes, full-model allocation, or execution.

The versioned `agent-project-input-v1` pre-validator MUST apply before Agent settings deserialization or conflict inventory returns a partial observation. Across one selected Agent it permits at most 64 project-controlled settings documents, 1,048,576 bytes per document and 8,388,608 aggregate bytes, 100,000 parsed scalar/container nodes, 16 container levels, and 65,536 UTF-8 bytes per scalar; it rejects duplicate keys, aliases/anchors, explicit tags, include/import expansion, multiple documents, and non-string keys in any format that can express them. Project-root conflict traversal permits at most 100,000 filesystem entries, 32 directory levels, 10,000 candidate Skill roots, 255 native bytes per path segment, and 4,096 native bytes per relative path; it uses `lstat`, never follows a directory symlink, and applies `SKL-SRC-007` frontmatter bounds to every inspected Skill. Exceeding any dimension MUST return `agent_input_limit_exceeded` with input class, measured, and allowed values and no settings/conflict model, target action, or filesystem write.

**Acceptance.** Security tests cover traversal, link escape, unsupported entry types, hostile metadata, exact Library-import/workspace/frontmatter/Agent-project parser and traversal ceilings, duplicate keys, expansion features, malicious refs/URLs, portable and host-only path collisions, relative Agent-root environments, foreign targets, repository/cache-contained executable or indirect-interpreter candidates, inherited `GIT_EXEC_PATH`/dynamic configuration/repository/index overrides, unsafe SSH child lookup, inherited Git SSH overrides, and pre-validation fetch budgets. Boundary fixtures stop on settings document 65, aggregate byte 8,388,609, node 100,001, level 17, scalar byte 65,537, traversal entry 100,001, depth 33, Skill root 10,001, segment byte 256, or relative-path byte 4,097 without a partial observation. Documentation states the same-account concurrent-attacker exclusion without treating project-controlled files as trusted or implying a stronger sandbox.
