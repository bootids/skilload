# CLI, JSON, Testing, and Release Design

Status: 部分实现的 0.1 CLI MVP design。`PLAN-0002` 实现 `0.0.1` configuration slice 的 `SKL-CLI-002`、`SKL-CLI-003` 与 `SKL-CLI-011`；`PLAN-0003` 实现 `library import`/`library export` 的可移植传输表面与其适用的 API-v2 projection；`PLAN-0004` 实现八个显式 Library metadata leaves 与其离线 API-v2/human projection；`PLAN-0005` 实现 `library list [--limit] [--offset]`、`library search <QUERY> [--limit] [--offset]`、`library get <CANONICAL-SOURCE>` 与 `doctor [--fix]` 及其 API-v2/human projection。其他 CLI、release 与 compatibility design 仍为 planned。

## Behavior Traceability

* Command parsing, common arguments, the field-level API-v2 catalog, confirmation, errors, idempotency, streams, and compatibility implement `SKL-CLI-001` through `SKL-CLI-012`.
* Offline dispatch, credentials, diagnostics, and network enforcement implement the CLI-facing portions of `SKL-OPS-005` and `SKL-OPS-007` through `SKL-OPS-009`.
* The default fixture and fault suites cover the acceptance mechanisms and scale targets named throughout `SKL-SRC-*`, `SKL-TRUST-*`, `SKL-LIB-*`, `SKL-WSP-*`, `SKL-GLB-*`, `SKL-MGR-*`, and `SKL-CACHE-*`.
* Build, platform matrix, version policy, checksums, attestations, license inclusion, and compatibility fixtures implement `SKL-PROD-002` and `SKL-PROD-004` through `SKL-PROD-007`.

## CLI Composition

Use `clap` derive definitions in `crates/skilload-cli` as the single command schema. Help, parser tests, manager asset contract tests, and operation identifiers derive from or are checked against that schema so command lists cannot drift.

当前 `0.0.1` schema 只注册具有真实实现的 `config get|set|unset|list`、`library import --input <PATH> [--dry-run]`、`library export --output <PATH>`、`library list [--limit <COUNT>] [--offset <COUNT>]`、`library search <QUERY> [--limit <COUNT>] [--offset <COUNT>]`、`library get <CANONICAL-SOURCE>`、`library alias set|clear`、`library category set|clear`、`library tag add|remove`、`library note set|clear`、`doctor [--fix]`，以及文本 help/version。它以同一 `clap` schema 驱动 parsing 与 help，不注册 aliases 或 generated help subcommand；未知 future domain/Library names 必须为 usage error，未实现 canonical leaves 不得被 scaffold。

The canonical tree is:

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

No aliases are registered. With no subcommand, the parser renders top-level help and returns success. Unsupported names remain usage errors.

Each leaf converts validated syntax into one application request with a stable dotted operation identifier such as `library.add`, `workspace.sync`, or `manager.status`. The CLI does not sequence lower-level repository calls itself.

The version-1 configuration key registry is exactly `cache_limit_bytes`, `agents.claude.executable`, and `agents.codex.executable`. The two Agent setters consume one absolute path argument and unset restores basename lookup rather than storing a default string. Source-bearing operations accept a fully qualified `--ref refs/heads/...`, `--ref refs/tags/...`, or full SHA when URL/shorthand input needs disambiguation. `workspace sync` alone accepts `--rebind-from <OLD-WORKSPACE>` and still requires explicit Agents including every Agent recorded by the old local manifest. `library list` and `library search` alone accept `--limit` and `--offset` with the exact `SKL-LIB-005` ranges/defaults. These are options on existing leaves, not additional commands or aliases.

可移植 Library 传输叶子使用原生路径选项，而不是隐式标准输入协议：`library import --input <PATH> [--dry-run]` 读取一个受限的可移植 `LibraryExportData` 文档，`library export --output <PATH>` 以原子方式写入该文档。命令正常的人类结果或 API-v2 JSON 信封与输出文件保持分离，因此调用方无需剥离信封即可在随后导入前检查操作结果。这些选项属于既有叶子，不创建别名或另一命令族。

P3 metadata leaves 各自将完整 canonical source 和一个逻辑 UTF-8 value（clear 不带 value）交给唯一 application method。CLI 不访问 repository；application 负责构造文本或 Unicode 15.1.0 tag value，SQLite port 负责锁、snapshot、transaction、durability 与结果。每个 success 使用对应的 `library.alias.*`、`library.category.*`、`library.tag.*` 或 `library.note.*` operation，返回 `LibraryMutationData` 的 committed entry、changed fields、`network: { used: false, attempts: [] }` 和三个 null acquisition-policy fields；missing canonical source 以 `not_found`/`LookupDetails`/exit 4 返回。人类输出以 terminal-safe quoted encoding 显示 operation、outcome、source、changed fields、trust state 和最终元数据。

`PLAN-0005` 已注册 `library list [--limit <COUNT>] [--offset <COUNT>]`、`library search <QUERY> [--limit <COUNT>] [--offset <COUNT>]`、`library get <CANONICAL-SOURCE>` 与 `doctor [--fix]`。`library get` 在本切片只接受完整 canonical source；derived-name convenience selector 仍不启用。Search 把 `<QUERY>` 当作纯文本词项 AND：CLI 保留原始逻辑字符串用于 `LibrarySearchData.query`，domain 以本地 Unicode 15.1.0 数据构造完全 quoted 的 FTS expression，不能让 SQLite operator grammar 越过 application boundary。三项 Library read 均以 `observed` 返回；doctor 默认使用 `observed`，`--fix` 只有提交至少一个 migration/repair action 时使用 `changed`，没有 action 时使用 `unchanged`。当前 API-v2 `LibraryEntriesData`、`LibrarySearchData`、`LibraryEntry` 与 `DoctorData` 已是唯一输出 schema，不增加 operation alias 或新信封。

## Common Arguments

Common presentation arguments are accepted at a documented consistent position:

* `--json` selects JSON mode and disables all prompts, spinners, color, and progress on stdout.
* `--confirm-token <opaque>` supplies the token returned by a prior JSON confirmation requirement.
* `--no-color` disables ANSI styling in human mode; non-TTY output defaults to no styling.
* Source-validation operations accept `--max-source-files <COUNT>` and `--max-source-bytes <BYTES>` only as explicit unsigned-64-bit per-request ceilings. JSON previews/results carry both active ceilings as exact `DecimalU64` strings in `source_limits.max_files` and `source_limits.max_bytes`; they are confirmation-bound and never stored as durable policy.
* `--cache-limit-bytes <BYTES>` is a separate positive unsigned-64-bit per-invocation effective quota accepted only by the cache-promoting operations enumerated in `SKL-CACHE-003`. It must be no smaller than the configured limit; it covers the complete preview batch, is confirmation-bound, serializes byte quantities as `DecimalU64`, and is not written to configuration or domain state.

Workspace sync and manager operations accept repeated or multi-value `--agent` and require at least one value. Global install/uninstall/sync/status require one or more `--agent` values when resolving current-environment profiles, or use stored `--profile <id>`/`--all-profiles` selection where specified. `global uninstall --profile <id> --detach-inaccessible` is the only detach form; reject the flag with `--agent`, `--all-profiles`, an accessible profile, or any operation other than uninstall. Avoid global flags whose presence silently changes product-domain scope.

Human interactive confirmation reads the terminal only after the application returns a complete approval preview. A noninteractive human stream that requires confirmation fails with clear guidance to use JSON preview/token or an explicit documented confirmation mechanism; it never assumes yes.

## JSON Envelope

Serialize exactly one compact or pretty-but-single JSON object to stdout. Version-2 success shape:

    {
      "api_version": 2,
      "operation": "workspace.sync",
      "ok": true,
      "result": {
        "outcome": "changed",
        "data": { }
      }
    }

Version-2 error/confirmation shape:

    {
      "api_version": 2,
      "operation": "library.add",
      "ok": false,
      "error": {
        "code": "confirmation_required",
        "message": "Approval is required before trusting this source.",
        "details": {
          "preview": { },
          "confirmation_token": "opaque",
          "expires_at": "2026-08-18T04:00:00Z"
        }
      }
    }

The authoritative [`../product-specs/api-v2.md`](../product-specs/api-v2.md) catalog defines required versus optional notation, scalar encodings, closed producer objects, Version-2 consumer forward compatibility, deterministic ordering, reusable records, and the exact `result.data` type plus allowed outcome for each of the 50 non-meta operation leaves. Read-only operations use `observed`; mutations use only the narrowed `changed`, `unchanged`, `already_exists`, or `already_immutable` values listed for their operation. Domain data uses explicit typed fields, injectively percent-encoded canonical source strings from `SKL-SRC-002` with `refs/heads/`, `refs/tags/`, or lowercase full commit SHA intent, `sha256:` integrity, opaque profile/workspace-instance IDs, and `PathValue` for every native filesystem path.

Every field with domain type `NativePath` serializes as the same object in success, preview, confirmation, status, and error details:

    {
      "display": "/tmp/\\xFF",
      "bytes_base64": "L3RtcC//"
    }

`display` is the inner content produced by the human field encoder, with no surrounding quote characters. `bytes_base64` uses RFC 4648's standard `A-Z a-z 0-9 + /` alphabet, required `=` padding, and no whitespace over exact operating-system path bytes. Always use this object, including for valid UTF-8 paths, so a field never changes JSON type; for example `/tmp/foo` becomes `{ "display": "/tmp/foo", "bytes_base64": "L3RtcC9mb28=" }`. Workspace roots, Agent roots, link/cache targets, executable paths, configuration locations, profile paths, journal paths, and path-bearing diagnostics are `NativePath`. Git Skill paths and refs are separately validated source-domain UTF-8 strings and remain strings. Do not use native paths as object keys. The decoder treats `bytes_base64` as authoritative and may use `display` only for safe presentation; mismatching display is an invalid payload in any future API input that accepts this type.

A preview or result for an operation that can acquire/promote external content includes the catalog's applicable `source_limits`, fixed `fetch_budget`, and `cache_quota` records. `CacheQuota` always includes `configured_limit_bytes`, `effective_limit_bytes`, `projected_bytes`, `stable_quarantine_bytes`, `repair_headroom_bytes`, and `override_applied`; null rather than omission represents a record that does not apply. `NetworkUse.attempts` preserves each source's metadata/content attempts, actual transport/credential class, and success/failure fallback order without secret material, so one batch never fabricates one aggregate transport. A detached uninstall result carries the affected profile/source/path in a `DetachedOrphan` with `link_removed: false` and `orphan_recorded: true`; list/status keep that orphan separate from active target associations. Workspace status supplies the required nullable `WorkspaceRelocation` evidence, including old/current paths and complete Agents, whenever it reports a proved relocation candidate.

Progress and diagnostics go to stderr. In JSON mode, stderr remains optional operational diagnostics and never carries data required to complete the workflow. Secrets and confirmation tokens are redacted from debug output; the token appears only in its JSON response and is stored hashed.

API version 2 only gains optional fields. Required-field removal/rename, enum narrowing/reinterpretation, an operation's result-type change, or an error code's details-type change requires a new top-level API version and compatibility/migration design. `clap` operation metadata and Rust result/error types are the implementation source, but generated validator fixtures must prove exact agreement with the normative catalog rather than treating Rust serialization as self-authorizing.

## Errors and Exit Status

Define stable string error codes and typed detail structs in `skilload-core`; rendering never derives either from prose. The API-v2 catalog's error table is exhaustive and maps every code to exactly one details type and exit category. `library_input_limit_exceeded` distinguishes the six bounded Library-import dimensions from API-v1's archived `agent_input_limit_exceeded`; both use `LimitDetails` without collapsing resource/security failures into a validation string. `source_limit_exceeded` uses its dedicated two-dimension record, while repository/ref/path discovery errors use `SourceLocator` so null unknown path/ref state cannot masquerade as a resolved identity.

Exit categories are stable but callers use JSON `error.code` for detail:

    0  successful, including idempotent outcomes
    2  CLI syntax or usage
    3  confirmation required/invalid confirmation
    4  domain precondition, not found, Trust, or conflict
    5  external unavailability, busy, permission, or network/auth
    6  integrity, schema, recovery, or internal invariant failure

Human errors use the same code internally, an English message, relevant paths/sources, and actionable next command where one exists. Never suggest destructive ownership bypass.

## Command/Query Network Policy

Application request types declare one of `Offline`, `MayResolve`, or `RequiresResolve`. The production network/Git port refuses access for an `Offline` request even if a handler accidentally calls it. Offline includes help/version, Library reads and metadata-only changes, Trust reads/revoke, workspace list/status/delete/remove and a cache-complete relocation rebind where no restoration occurs, global list/status/uninstall, manager operations, cache info/prune/clear, config, and doctor/fix. Cleanup remains possible without source access.

Network-capable requests are explicit source add/Trust add, Library refresh, source or workspace source migration, workspace lock/update/pin and a sync with a cache miss, global install/update/pin and a sync with a cache miss. Migration resolution may only prove that fresh metadata for the proposed name matches the repository ID stored with the old source; it cannot change path or ref under the migration operation. A `source migrate` result separates mutated Library/Trust/global records from read-only workspace impacts; only `workspace migrate-source` may report workspace records as changed. Wherever the catalog includes `NetworkUse`, the result records a deterministic per-source sequence for every metadata and content attempt with its actual stage, transport, credential class, and outcome; a mixed batch can therefore report HTTPS for one source and HTTPS-failed/SSH-succeeded for another without returning secret material. Operation types that omit it are statically offline by contract.

## Human Rendering

Human output is English and optimized for terminal scanning rather than mirroring JSON. Mutation previews show complete selected sources/targets and warnings before confirmation. Success summaries distinguish changed versus unchanged and include degraded states. List/search output has deterministic sort and an explicit machine-independent fallback when width is narrow or stdout is not a TTY.

Treat every repository-controlled, path/filesystem-derived, environment-derived, and user-supplied value as untrusted terminal data. A shared renderer wraps each such field in ASCII double quotes, represents quote as backslash-double-quote and backslash as two backslashes, uses `\n`, `\r`, and `\t`, renders every other C0/DEL/C1 code point, U+2028/U+2029, and the bidirectional-format set U+061C, U+200E-U+200F, U+202A-U+202E, and U+2066-U+2069 as `\u{XXXX}` with uppercase hexadecimal zero-padded to four through six digits, and renders invalid UTF-8 bytes as `\xHH` with two uppercase digits. The encoding is injective because literal backslashes are escaped. No data field may contain raw ESC, BEL, carriage return, cursor movement, OSC hyperlink, or bidi-format control. Renderer-owned layout is the only source of newlines; renderer-owned ANSI styling is the only source of terminal escapes and is absent for `--no-color` or non-TTY output. Apply the encoder before width calculation so truncation never splits an escape, and prefer wrapping over dropping identifying bytes.

JSON uses a standards-compliant serializer over original valid string domain values, not their human-display form. JSON escapes its required string control characters. Every native path, whether valid UTF-8 or not, uses the `PathValue` object above; its display member uses the same encoder without outer quotes and its base64 member preserves exact bytes. Debug and error rendering use the terminal-safe field encoder, so a validation failure cannot reintroduce hostile bytes outside a preview.

Do not write persistent logs by default. `--debug` (or a documented environment equivalent) writes redacted diagnostics to stderr. An explicit debug-log destination, if a later delivery adds it, belongs under XDG state and must be opt-in.

## Test Architecture

Default tests are offline and deterministic:

* unit tests for domain validation, source normalization, integrity encoding, conflict policy, command schema, JSON rendering, and the catalog's exact error-to-details/exit mapping;
* repository/adapter contract tests against fakes and SQLite/filesystem implementations;
* temporary bare Git repositories with same-name/different-commit branches and tags, ambiguous slash-bearing refs/URL paths, SHA pins, submodules, LFS pointers, symlinks, executable bits, exact valid/invalid root and non-root Skill names, portable/target-filesystem path collisions, bounded-frontmatter fixtures, hostile names, bounded pack bytes/objects/deadlines, and deleted/unavailable commits;
* local HTTP fixtures for GitHub metadata, redirects, auth/rate errors, default branches, repository ID changes, and candidate trees;
* isolated HOME/XDG/Claude/Codex roots and fake Agent/Git/gh/ssh/skilload executables/configuration, including empty/relative/project/cache PATH entries, inherited Git exec-path/config/repository/index/SSH overrides, fixed-exec-path and bound-real-index assertions, relative Agent-root environments, outside symlinks back into a worktree, complete native/direct/env script-interpreter chains, and execution-marker assertions;
* transaction failpoint tests at every journal/filesystem/database phase;
* workspace relocation fixtures for proved rebind, duplicate/copy refusal, old-path accessibility, complete Agent selection, link/exclude transfer, crash recovery, and exact nullable API evidence containing old/current paths plus required Agents;
* bounded Library import JSON, workspace YAML, and Agent project-input fixtures covering every exact document/byte/entry/value/record/node/depth/scalar/traversal limit, duplicate-key/non-expanding feature rejection, directory symlinks, and no model/plan before pre-validation, plus tracked-manifest fixtures covering literal Git pathspecs, alternate-index poisoning, blocked recovery, and retry after untracking;
* golden JSON/help/human snapshots with secret-redaction assertions and hostile ANSI/OSC/CR/bidirectional/invalid-byte fields in previews, errors, lists, and diagnostics, plus `PathValue` round trips for valid UTF-8, invalid bytes, padding boundaries, and every path-bearing field; schema coverage extracts exactly 50 parser leaves, compares their dotted identifiers to the catalog, and validates every leaf/outcome/type pair, every confirmable preview, and every listed error details variant, including full-range decimal-u64 ceilings, both source-limit dimensions, repository/ref/source locators, and mixed per-source HTTPS/SSH fallback attempts;
* cache-quota fixtures for the 536,870,912-byte default, persistent set/unset, accepted and rejected per-invocation overrides, complete-batch allocated-byte accounting, retained staging/quarantine visibility, one-object repair headroom, success/failure/crash cleanup, confirmation drift, and non-persistence;
* profile fixtures proving that auxiliary Codex observations do not split one `(Agent, global root)` identity, plus inaccessible detach/orphan/cleanup fixtures that never claim an unobserved link deletion;
* removal-only fixtures proving absent Agent executables and revoked Trust do not block an explicitly requested empty exact-owned workspace sync, exact global uninstall, or exact manager uninstall while additive, content-using, inaccessible, drifted, foreign, and mixed plans remain failures;
* Library pagination fixtures for default and explicit pages, adjacent-page stability, offset-at/beyond-total emptiness, full unsigned-64-bit offset encoding, and parser rejection of zero/over-1,000 limits or misplaced flags;
* post-deployment cache-modification fixtures proving the direct-read limitation, read-only detection, and quarantine/refetch behavior on the next mutating use;
* database-corruption fixtures covering backup manifest selection, isolated restore validation, stale WAL exclusion, atomic rollback, explicit reset, and non-adoption; source-migration fixtures proving workspace impacts remain read-only until the separate journaled migration; and scale fixtures for 10,000 Library entries, 200 workspace Skills, and 100 global targets.

Performance budgets are recorded by the implementation Plan before acceptance, then enforced in nonflaky benchmark/integration thresholds. Tests must measure representative search, status, lock planning, and deployment planning rather than only database insertion.

Real GitHub and real Claude/Codex smoke tests are explicit, credential-aware jobs or scheduled workflows. They are not part of the default suite. A real model conversation with the manager Skill is optional and nonblocking.

## Toolchain and Build

The P1 foundation provides a root Cargo workspace, committed `Cargo.lock`, `rust-toolchain.toml`, and `mise.toml`. mise pins Rust and any Node/npm/pnpm used only for repository tooling; the released product has no Node runtime dependency.

当前二进制以 bundled SQLite（含 FTS5 编译能力）实现 P2 可移植 Library 元数据传输，但不创建 FTS schema、不链接 HTTPS client，也不执行外部程序。后续 full 0.1 binary 将在具有真实 source/deployment 行为时加入 HTTPS client、system `git`、仅限 SSH Git transport 的 system `ssh` 与 selected Agent CLI；exact-owned removal-only operations 不需要 Agent executable。`gh` 仍可选。Build metadata 暴露 product version、Git commit、target triple 与 manager asset version，且不嵌入 build-machine path 或阻碍 reproducibility 的 timestamp。

## Release Matrix and Provenance

GitHub Actions builds and tests these release triples:

    aarch64-apple-darwin
    x86_64-apple-darwin
    aarch64-unknown-linux-gnu
    x86_64-unknown-linux-gnu

Each archive contains the binary, license, concise install/readme material, and no platform-irrelevant cache/state. The release job:

1. checks that the version/tag follows the 0.0.x/0.1.x policy and the worktree lock is committed;
2. runs the full offline test/fault/format/lint matrix;
3. builds each target from the same source commit;
4. smoke-runs `--version`, no-argument help, and empty JSON reads on the produced artifact where executable;
5. creates deterministic archives and one SHA-256 checksum manifest;
6. publishes GitHub artifact attestations bound to artifacts and source commit;
7. creates/updates the GitHub Release only after every target and provenance step succeeds.

Homebrew stable starts at 0.1.0. The formula downloads release archives and pins their published checksums. A prerelease 0.0.x may use a separate tap/formula only when clearly labelled and must not replace stable. `cargo install` is optional and uses the same locked source version but is not the primary reproducible binary channel.

Code signing and macOS notarization are future hardening, not hidden 0.1 acceptance. Adding them later changes release design and evidence, not product command semantics.

## Compatibility Checks

Archive Version-1 fixtures as historical evidence and maintain current Version-2 fixtures for every JSON operation/outcome/error document, workspace config/lock 1, Library export 1, config 1, database migrations, and manager markers. Every 0.1.x Version-2 build preserves all required Version-2 fields, encodings, discriminators, ordering, and enum meanings while its consumer ignores newly added optional fields. A later breaking format or command change requires a new product behavior revision, explicit migration, and later minor version.
