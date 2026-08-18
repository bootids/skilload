# CLI, JSON, Testing, and Release Design

Status: planned design for the 0.1 CLI MVP. It implements `SKL-CLI-*`, `SKL-PROD-002`, and `SKL-PROD-004` through `SKL-PROD-007`.

## Behavior Traceability

* Command parsing, common arguments, the field-level API-v1 catalog, confirmation, errors, idempotency, streams, and compatibility implement `SKL-CLI-001` through `SKL-CLI-012`.
* Offline dispatch, credentials, diagnostics, and network enforcement implement the CLI-facing portions of `SKL-OPS-005` and `SKL-OPS-007` through `SKL-OPS-009`.
* The default fixture and fault suites cover the acceptance mechanisms and scale targets named throughout `SKL-SRC-*`, `SKL-TRUST-*`, `SKL-LIB-*`, `SKL-WSP-*`, `SKL-GLB-*`, `SKL-MGR-*`, and `SKL-CACHE-*`.
* Build, platform matrix, version policy, checksums, attestations, license inclusion, and compatibility fixtures implement `SKL-PROD-002` and `SKL-PROD-004` through `SKL-PROD-007`.

## CLI Composition

Use `clap` derive definitions in `crates/skilload-cli` as the single command schema. Help, parser tests, manager asset contract tests, and operation identifiers derive from or are checked against that schema so command lists cannot drift.

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

The version-1 configuration key registry is exactly `cache_limit_bytes`, `agents.claude.executable`, and `agents.codex.executable`. The two Agent setters consume one absolute path argument and unset restores basename lookup rather than storing a default string. Source-bearing operations accept a fully qualified `--ref refs/heads/...`, `--ref refs/tags/...`, or full SHA when URL/shorthand input needs disambiguation. `workspace sync` alone accepts `--rebind-from <OLD-WORKSPACE>` and still requires explicit Agents including every Agent recorded by the old local manifest. These are options on existing leaves, not additional commands or aliases.

## Common Arguments

Common presentation arguments are accepted at a documented consistent position:

* `--json` selects JSON mode and disables all prompts, spinners, color, and progress on stdout.
* `--confirm-token <opaque>` supplies the token returned by a prior JSON confirmation requirement.
* `--no-color` disables ANSI styling in human mode; non-TTY output defaults to no styling.
* Source-validation operations accept `--max-source-files <COUNT>` and `--max-source-bytes <BYTES>` only as explicit per-request ceilings. JSON previews/results carry both active ceilings as `source_limits.max_files` and `source_limits.max_bytes`; they are confirmation-bound and never stored as durable policy.
* `--cache-limit-bytes <BYTES>` is a separate per-invocation effective quota accepted only by the cache-promoting operations enumerated in `SKL-CACHE-003`. It must be positive, finite, and no smaller than the configured limit; it covers the complete preview batch, is confirmation-bound, and is not written to configuration or domain state.

Workspace sync and manager operations accept repeated or multi-value `--agent` and require at least one value. Global install/uninstall/sync/status require one or more `--agent` values when resolving current-environment profiles, or use stored `--profile <id>`/`--all-profiles` selection where specified. `global uninstall --profile <id> --detach-inaccessible` is the only detach form; reject the flag with `--agent`, `--all-profiles`, an accessible profile, or any operation other than uninstall. Avoid global flags whose presence silently changes product-domain scope.

Human interactive confirmation reads the terminal only after the application returns a complete approval preview. A noninteractive human stream that requires confirmation fails with clear guidance to use JSON preview/token or an explicit documented confirmation mechanism; it never assumes yes.

## JSON Envelope

Serialize exactly one compact or pretty-but-single JSON object to stdout. Version-1 success shape:

    {
      "api_version": 1,
      "operation": "workspace.sync",
      "ok": true,
      "result": {
        "outcome": "changed",
        "data": { }
      }
    }

Version-1 error/confirmation shape:

    {
      "api_version": 1,
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

The authoritative [`../product-specs/api-v1.md`](../product-specs/api-v1.md) catalog defines required versus optional notation, scalar encodings, closed producer objects, consumer forward compatibility, deterministic ordering, reusable records, and the exact `result.data` type plus allowed outcome for each of the 50 non-meta operation leaves. Read-only operations use `observed`; mutations use only the narrowed `changed`, `unchanged`, `already_exists`, or `already_immutable` values listed for their operation. Domain data uses explicit typed fields, injectively percent-encoded canonical source strings from `SKL-SRC-002` with `refs/heads/`, `refs/tags/`, or lowercase full commit SHA intent, `sha256:` integrity, opaque profile/workspace-instance IDs, and the required path shape below. Although `RepositoryId` is a `u64` internally, portable JSON/YAML serializes it as a decimal string to avoid IEEE-754 precision loss in common consumers. Lists follow the catalog's stable ordering.

Every field with domain type `NativePath` serializes as the same object in success, preview, confirmation, status, and error details:

    {
      "display": "/tmp/\\xFF",
      "bytes_base64": "L3RtcC//"
    }

`display` is the inner content produced by the human field encoder, with no surrounding quote characters. `bytes_base64` uses RFC 4648's standard `A-Z a-z 0-9 + /` alphabet, required `=` padding, and no whitespace over exact operating-system path bytes. Always use this object, including for valid UTF-8 paths, so a field never changes JSON type; for example `/tmp/foo` becomes `{ "display": "/tmp/foo", "bytes_base64": "L3RtcC9mb28=" }`. Workspace roots, Agent roots, link/cache targets, executable paths, configuration locations, profile paths, journal paths, and path-bearing diagnostics are `NativePath`. Git Skill paths and refs are separately validated source-domain UTF-8 strings and remain strings. Do not use native paths as object keys. The decoder treats `bytes_base64` as authoritative and may use `display` only for safe presentation; mismatching display is an invalid payload in any future API input that accepts this type.

A preview or result for an operation that can acquire/promote external content includes the catalog's applicable `source_limits`, fixed `fetch_budget`, and `cache_quota` records. `CacheQuota` always includes `configured_limit_bytes`, `effective_limit_bytes`, `projected_bytes`, `stable_quarantine_bytes`, `repair_headroom_bytes`, and `override_applied`; null rather than omission represents a record that does not apply. A detached uninstall result carries the affected profile/source/path in a `DetachedOrphan` with `link_removed: false` and `orphan_recorded: true`; list/status keep that orphan separate from active target associations.

Progress and diagnostics go to stderr. In JSON mode, stderr remains optional operational diagnostics and never carries data required to complete the workflow. Secrets and confirmation tokens are redacted from debug output; the token appears only in its JSON response and is stored hashed.

API version 1 only gains optional fields. Required-field removal/rename, enum narrowing/reinterpretation, an operation's result-type change, or an error code's details-type change requires a new top-level API version and compatibility/migration design. `clap` operation metadata and Rust result/error types are the implementation source, but generated validator fixtures must prove exact agreement with the normative catalog rather than treating Rust serialization as self-authorizing.

## Errors and Exit Status

Define stable string error codes and typed detail structs in `skilload-core`; rendering never derives either from prose. The API-v1 catalog's error table is exhaustive and maps every code to exactly one details type and exit category. In particular, independent resource/security failures remain distinguishable as `fetch_limit_exceeded`, `agent_input_limit_exceeded`, `portable_path_collision`, `filesystem_path_collision`, `unsupported_interpreter`, `cache_quota_exceeded`, `cache_repair_space_insufficient`, and `database_corrupt`, rather than collapsing into a validation string. Confirmation has separate required-details variants for required, invalid, expired, and stale tokens. No API-v1 handler may emit an unlisted code, omit a required details field, or substitute a free-form object.

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

Network-capable requests are explicit source add/Trust add, Library refresh, source or workspace source migration, workspace lock/update/pin and a sync with a cache miss, global install/update/pin and a sync with a cache miss. Migration resolution may only prove that fresh metadata for the proposed name matches the repository ID stored with the old source; it cannot change path or ref under the migration operation. A `source migrate` result separates mutated Library/Trust/global records from read-only workspace impacts; only `workspace migrate-source` may report workspace records as changed. Wherever the catalog includes `NetworkUse`, the result records whether network was used plus separate metadata and content-transport credential classes without returning secret material; operation types that omit it are statically offline by contract.

## Human Rendering

Human output is English and optimized for terminal scanning rather than mirroring JSON. Mutation previews show complete selected sources/targets and warnings before confirmation. Success summaries distinguish changed versus unchanged and include degraded states. List/search output has deterministic sort and an explicit machine-independent fallback when width is narrow or stdout is not a TTY.

Treat every repository-controlled, path/filesystem-derived, environment-derived, and user-supplied value as untrusted terminal data. A shared renderer wraps each such field in ASCII double quotes, represents quote as backslash-double-quote and backslash as two backslashes, uses `\n`, `\r`, and `\t`, renders every other C0/DEL/C1 code point, U+2028/U+2029, and the bidirectional-format set U+061C, U+200E-U+200F, U+202A-U+202E, and U+2066-U+2069 as `\u{XXXX}` with uppercase hexadecimal zero-padded to four through six digits, and renders invalid UTF-8 bytes as `\xHH` with two uppercase digits. The encoding is injective because literal backslashes are escaped. No data field may contain raw ESC, BEL, carriage return, cursor movement, OSC hyperlink, or bidi-format control. Renderer-owned layout is the only source of newlines; renderer-owned ANSI styling is the only source of terminal escapes and is absent for `--no-color` or non-TTY output. Apply the encoder before width calculation so truncation never splits an escape, and prefer wrapping over dropping identifying bytes.

JSON uses a standards-compliant serializer over original valid string domain values, not their human-display form. JSON escapes its required string control characters. Every native path, whether valid UTF-8 or not, uses the `PathValue` object above; its display member uses the same encoder without outer quotes and its base64 member preserves exact bytes. Debug and error rendering use the terminal-safe field encoder, so a validation failure cannot reintroduce hostile bytes outside a preview.

Do not write persistent logs by default. `--debug` (or a documented environment equivalent) writes redacted diagnostics to stderr. An explicit debug-log destination, if P1 adds it, belongs under XDG state and must be opt-in.

## Test Architecture

Default tests are offline and deterministic:

* unit tests for domain validation, source normalization, integrity encoding, conflict policy, command schema, JSON rendering, and the catalog's exact error-to-details/exit mapping;
* repository/adapter contract tests against fakes and SQLite/filesystem implementations;
* temporary bare Git repositories with same-name/different-commit branches and tags, ambiguous slash-bearing refs/URL paths, SHA pins, submodules, LFS pointers, symlinks, executable bits, exact valid/invalid root and non-root Skill names, portable/target-filesystem path collisions, bounded-frontmatter fixtures, hostile names, bounded pack bytes/objects/deadlines, and deleted/unavailable commits;
* local HTTP fixtures for GitHub metadata, redirects, auth/rate errors, default branches, repository ID changes, and candidate trees;
* isolated HOME/XDG/Claude/Codex roots and fake Agent/Git/gh/ssh/skilload executables/configuration, including empty/relative/project/cache PATH entries, inherited Git exec-path/config/repository/index/SSH overrides, fixed-exec-path and bound-real-index assertions, relative Agent-root environments, outside symlinks back into a worktree, complete native/direct/env script-interpreter chains, and execution-marker assertions;
* transaction failpoint tests at every journal/filesystem/database phase;
* workspace relocation fixtures for proved rebind, duplicate/copy refusal, old-path accessibility, complete Agent selection, link/exclude transfer, and crash recovery;
* bounded Library import JSON, workspace YAML, and Agent project-input fixtures covering every exact document/byte/entry/value/record/node/depth/scalar/traversal limit, duplicate-key/non-expanding feature rejection, directory symlinks, and no model/plan before pre-validation, plus tracked-manifest fixtures covering literal Git pathspecs, alternate-index poisoning, blocked recovery, and retry after untracking;
* golden JSON/help/human snapshots with secret-redaction assertions and hostile ANSI/OSC/CR/bidirectional/invalid-byte fields in previews, errors, lists, and diagnostics, plus `PathValue` round trips for valid UTF-8, invalid bytes, padding boundaries, and every path-bearing field; schema coverage extracts exactly 50 parser leaves, compares their dotted identifiers to the catalog, and validates every leaf/outcome/type pair, every confirmable preview, and every listed error details variant;
* cache-quota fixtures for the 536,870,912-byte default, persistent set/unset, accepted and rejected per-invocation overrides, complete-batch allocated-byte accounting, retained staging/quarantine visibility, one-object repair headroom, success/failure/crash cleanup, confirmation drift, and non-persistence;
* profile fixtures proving that auxiliary Codex observations do not split one `(Agent, global root)` identity, plus inaccessible detach/orphan/cleanup fixtures that never claim an unobserved link deletion;
* removal-only fixtures proving absent Agent executables do not block empty workspace sync, exact global uninstall, or exact manager uninstall while additive, inaccessible, drifted, foreign, and mixed plans remain failures;
* post-deployment cache-modification fixtures proving the direct-read limitation, read-only detection, and quarantine/refetch behavior on the next mutating use;
* database-corruption fixtures covering backup manifest selection, isolated restore validation, stale WAL exclusion, atomic rollback, explicit reset, and non-adoption; source-migration fixtures proving workspace impacts remain read-only until the separate journaled migration; and scale fixtures for 10,000 Library entries, 200 workspace Skills, and 100 global targets.

Performance budgets are recorded by the implementation Plan before acceptance, then enforced in nonflaky benchmark/integration thresholds. Tests must measure representative search, status, lock planning, and deployment planning rather than only database insertion.

Real GitHub and real Claude/Codex smoke tests are explicit, credential-aware jobs or scheduled workflows. They are not part of the default suite. A real model conversation with the manager Skill is optional and nonblocking.

## Toolchain and Build

The later P1 foundation adds a root Cargo workspace, committed `Cargo.lock`, `rust-toolchain.toml`, and `mise.toml`. mise pins Rust and any Node/npm/pnpm used only for repository tooling; the released product has no Node runtime dependency.

The binary links SQLite with FTS5 and an HTTPS client suitable for GitHub. Its required external runtime executables are system `git`, system `ssh` only for SSH Git transport, and the selected Agent CLI only for additive/repair/functional Agent operations. Exact-owned removal-only operations need no Agent executable. `gh` remains optional. Build metadata exposes product version, Git commit, target triple, and manager asset version without embedding build-machine paths or timestamps that prevent reproducibility.

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

Maintain versioned fixtures for every JSON API 1 operation/outcome/error document, workspace config/lock 1, Library export 1, config 1, database migrations, and manager markers. The released 0.1.0 corpus becomes an immutable compatibility input. Every 0.1.x build reads prior patch fixtures, preserves all required fields, encodings, discriminators, ordering, and enum meanings, and proves its consumer ignores newly added optional fields. A breaking format or command change requires a new product behavior revision, explicit migration, and later minor version.
