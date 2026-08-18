# Cache and Local Operations

Status: planned baseline for the skilload CLI MVP.

The **cache** contains removable external Skill bytes. Durable metadata and desired state live elsewhere. Operational state contains journals and ownership records needed to recover managed mutations safely.

## SKL-CACHE-001 - Immutable external-content cache (Revision 1)

**Behavior.** skilload MUST manage external Skill materializations as immutable, content-addressed objects keyed by numeric repository identity, commit, and normalized Skill path, with canonical integrity verification. It MUST never edit a promoted object in place and MUST apply non-writable object permissions where the host supports them, while treating those permissions as defense in depth rather than protection from the same operating-system account. The cache MUST contain no built-in manager copy and MUST NOT be the authoritative store for Library, Trust, workspace, or global desired state.

**Acceptance.** Two sources for the same repository/commit/path reuse one verified entry. A local byte, mode, path, or symlink-target change is detected at the next skilload integrity verification and is treated as corruption rather than a new valid form of the object.

## SKL-CACHE-002 - Prune protection follows active managed links (Revision 1)

**Behavior.** `cache prune` MUST protect every verified cache entry currently targeted by a managed workspace or global link. A lockfile, Library entry, or Trust record alone MUST NOT pin cache bytes. Unprotected entries MAY be evicted by least-recently-used order.

**Acceptance.** Prune leaves linked content intact, may remove an unlinked locked entry, and reports protected/freed totals without modifying durable source state.

## SKL-CACHE-003 - Quota enforcement before state commit (Revision 1)

**Behavior.** The Revision 1 cache limit MUST default to 536,870,912 bytes (512 MiB) when `config.toml` is absent or `cache_limit_bytes` is unset. `config set cache_limit_bytes <BYTES>` MUST persist a positive finite byte limit, and `config unset cache_limit_bytes` MUST restore the default. Before a mutation commits state requiring new cache content, skilload MUST enforce that configured limit after first planning removal of unprotected least-recently-used entries. If capacity remains insufficient, the mutation MUST fail without persistent product state unless the same invocation supplies `--cache-limit-bytes <BYTES>`, a positive finite effective limit no smaller than the configured value. The flag MUST be accepted only by `library add`, `library refresh`, `trust add`, `workspace add`, `workspace lock`, `workspace update`, `workspace pin`, `workspace sync`, `global install`, `global sync`, `global update`, and `global pin`, because these are the 0.1 operations that may promote external cache content. It applies to the complete invocation and all objects in its preview, is bound into confirmation, and MUST NOT persist in configuration, Trust, Library, workspace, lock, global desired state, or a later invocation. Human and JSON previews/results MUST expose configured, effective, projected post-operation, and override-applied values.

**Acceptance.** With no configuration file, a projected total of 536,870,913 bytes fails without an override and leaves no Trust/Library/workspace/global state referring to an unpromoted entry. Repeating the operation with a sufficient `--cache-limit-bytes` value may succeed after confirmation and reports `cache_quota.configured_limit_bytes`, `effective_limit_bytes`, `projected_bytes`, and `override_applied` in JSON; the next invocation uses 536,870,912 again unless configuration was explicitly changed.

## SKL-CACHE-004 - Clear preserves durable intent (Revision 1)

**Behavior.** `cache clear` MUST preflight and remove verified managed workspace/global links before deleting all external cached content. It MUST preserve Library, Trust, workspace config/lock, global desired state, known workspace/profile records, and built-in manager copies. An inaccessible or mismatched known workspace blocks by default; explicit force MAY continue and report orphaned or broken links.

**Acceptance.** A normal clear leaves no managed link pointing into deleted cache. After clear, durable lists and files are unchanged and a later trusted sync can restore them. Force reports every link it could not safely remove.

## SKL-CACHE-005 - Corruption quarantine and one exact retry (Revision 1)

**Behavior.** When a mutating operation would use cached content and its integrity fails, skilload MUST quarantine the entry, attempt at most one refetch of the same repository ID, commit, and path, and verify the expected digest. A second mismatch or unavailable commit MUST fail without rewriting any lock or pin and without substituting current ref content. Read-only commands MUST report the mismatch without quarantining or refetching it. skilload MUST verify before promotion, link creation/replacement, or another mutating use, but it does not wrap an Agent or mediate reads through an already deployed native link; a post-deployment local modification can therefore be read by the Agent until the next skilload integrity observation.

**Acceptance.** Once skilload detects a locally modified entry, it never promotes, links, or reuses that entry as valid content. A correct mutating-path refetch restores the expected digest; a persistent mismatch leaves the original pin unchanged and reports quarantine/refetch evidence. Doctor/status detect the same mismatch while leaving cache paths unchanged. A test that modifies content after link deployment demonstrates the explicit limitation: a direct Agent filesystem read can see the change before detection, and the next skilload check reports it.

## SKL-CACHE-006 - Read-only doctor coverage (Revision 1)

**Behavior.** `doctor` MUST be read-only by default. From any directory it MUST inspect durable database/schema, Trust consistency, cache integrity/indexes, global deployments, manager ownership, recovery journals, and known workspace/profile indexes. It performs deep current-workspace checks only when the current directory exactly contains `.skilload.yaml`.

**Acceptance.** Running doctor on healthy or broken state creates and modifies no file, database row, link, or network request, while reporting each detected inconsistency with a stable code.

## SKL-CACHE-007 - Verifiable offline doctor fixes (Revision 1)

**Behavior.** `doctor --fix` MUST perform no network access and MAY repair only derived state whose expected value and skilload ownership can be proved locally. Recreating an external deployment link from verified cache MUST also require active exact Trust. It MUST NOT rewrite product source-of-truth files to hide corruption, adopt foreign paths, or repair external bytes without their expected verified content.

**Acceptance.** It can rebuild derived FTS data or an exactly verifiable manifest, and can recreate an exact cached link only while Trust is active. It refuses a foreign target or revoked source and reports that cache-miss restoration requires a separate network-capable sync.

## SKL-CACHE-008 - Recoverable multi-resource mutations (Revision 1)

**Behavior.** Every mutation spanning database/files/cache/link/workspace resources MUST use a persistent journal with enough before/after evidence for the next mutating command to roll forward or roll back safely. Normal success MUST be reported only after every resource commits. skilload does not promise an instantaneous filesystem transaction across directories.

**Acceptance.** Failure injection at each staged step followed by another mutation first recovers to one coherent old or new state, never silently accepts a mixed state, and preserves every unowned path.

## SKL-CACHE-009 - Concurrency and cache-miss restoration (Revision 1)

**Behavior.** Workspace and durable-database mutations MUST use process-safe locks with a bounded wait followed by structured `busy`. Network retrieval MAY stage concurrently, but final commit MUST reacquire/revalidate its baseline. Cache-miss restoration MUST retrieve the exact pin and require active Trust; it MUST not run during read-only operations.

**Acceptance.** Concurrent conflicting mutations yield one commit and one bounded `busy` or revalidation failure, not lost updates. A status/read command never fills a cache miss, while sync may do so at the exact digest.

## SKL-CACHE-010 - Offline cache information (Revision 1)

**Behavior.** `cache info` MUST be read-only and offline. It MUST report the configured quota, verified entry count and bytes, protected and reclaimable totals, quarantine totals, and locally known corruption or index inconsistencies. It MUST NOT verify by refetching, restore a missing object, update recency, prune, or create absent state.

**Acceptance.** In an isolated populated cache with networking denied, info reports totals consistent with manifests and ownership records while every file and database timestamp remains unchanged. On absent state it returns zero/default values without creating directories.

## SKL-OPS-001 - XDG state separation (Revision 1)

**Behavior.** skilload MUST place user configuration under `XDG_CONFIG_HOME`, durable data under `XDG_DATA_HOME`, operational state/journals under `XDG_STATE_HOME`, and removable content under `XDG_CACHE_HOME`, each with documented platform fallbacks. An XDG home value MUST be used only when it is nonempty and absolute; an unset, empty, or relative value MUST be ignored in favor of its `HOME`-based fallback. `HOME` MUST itself be nonempty and absolute when a fallback is needed, otherwise skilload MUST return a structured environment-path error before filesystem access. Workspace files remain in the workspace.

**Acceptance.** Isolated XDG integration tests show each file category only in its designated root, and clearing the cache root cannot remove durable data or operational ownership records. Setting `XDG_DATA_HOME=.data` from two different current directories uses the same absolute `HOME/.local/share` fallback and creates no current-directory `.data`; an invalid fallback `HOME` fails before reading or writing state.

## SKL-OPS-002 - Durable database ownership (Revision 1)

**Behavior.** One embedded SQLite database MUST own Library metadata, Trust, global desired state, manager ownership records, known workspace/profile indexes, and FTS5 indexes. It MUST NOT store external Skill file bytes, workspace config/lock contents as authoritative copies, credentials, or built-in manager asset bytes.

**Acceptance.** Database schema inspection finds every listed durable domain and no Skill-content blob or secret. Deleting removable cache leaves database rows intact.

## SKL-OPS-003 - Forward database migration (Revision 1)

**Behavior.** Before a database schema upgrade, skilload MUST create a recoverable backup and then apply a transactional forward migration. A database with an unknown newer schema or a requested downgrade MUST refuse writes rather than guessing or rewriting.

**Acceptance.** Fault injection leaves either the prior readable database plus backup or the complete new schema. A newer-schema fixture permits safe diagnostics/export where possible but rejects mutation.

## SKL-OPS-004 - Database corruption handling (Revision 1)

**Behavior.** Suspected database corruption MUST stop writes and MUST NOT silently recreate an empty database. Doctor MAY rebuild derived FTS indexes when base records are provably intact; otherwise diagnostics MUST guide backup, export where possible, and a documented explicit out-of-band restore or reset procedure. The 0.1 CLI MUST NOT expose an unlisted reset command.

**Acceptance.** A corrupt fixture never turns into an empty successful Library. Repair of an FTS-only failure preserves base row identities and metadata.

## SKL-OPS-005 - Lazy state creation (Revision 1)

**Behavior.** `--help`, `--version`, doctor/read operations on absent state, and empty list/search/status queries MUST NOT create config, data, state, cache, workspace, or Agent directories. The first successful persistent mutation creates only the roots it needs.

**Acceptance.** Running every read-only command in an isolated HOME leaves the filesystem unchanged. The first Library add does not create workspace or Agent directories.

## SKL-OPS-006 - Explicit versioned configuration (Revision 1)

**Behavior.** `config.toml` MUST have an explicit schema version and MAY store only nonsecret operational settings such as Agent executable overrides and the `cache_limit_bytes` key defined by `SKL-CACHE-003`. Unknown fields and unsupported schema versions MUST be errors. `config get|set|unset|list` are the only configuration mutation surface; no command silently migrates its schema.

**Acceptance.** An unknown field prevents dependent mutation without rewriting the file. Setting `cache_limit_bytes` changes the persistent effective quota, unsetting it restores 536,870,912 bytes, and neither operation echoes or stores a credential.

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

**Behavior.** skilload MUST treat remote repositories and cloned workspace configuration as potentially malicious data. It MAY trust the current operating-system account, user-controlled local files, and an Agent process explicitly launched by that user. It does not claim protection from another process running as the same account. All untrusted names, paths, file entries, and metadata MUST be validated before writes.

**Acceptance.** Security tests cover traversal, link escape, unsupported entry types, hostile metadata, oversized input, malicious refs/URLs, and foreign targets. Documentation states the same-account attacker exclusion without implying a stronger sandbox.
