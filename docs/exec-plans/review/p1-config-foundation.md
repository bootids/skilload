---
plan_id: PLAN-0002
branch: codex/p1-config-foundation
pull_request: https://github.com/bootids/skilload/pull/2
status: review
depends_on: [PLAN-0001]
---

# Establish the Rust foundation and versioned configuration

This delivery creates the first executable skilload slice without pretending that the full 0.1 CLI exists. A user will be able to run a `0.0.1` development binary, inspect the three supported configuration keys, set or unset their values through human or JSON output, and observe strict schema, path, idempotence, XDG, and no-network behavior. The result is visible through real `skilload config get|set|unset|list` commands and an offline test matrix; every other product command remains absent rather than appearing as a stub.

This ExecPlan is a living document. As work proceeds, the `Progress`, `Surprises & Discoveries`, `Decision Log`, `Outcomes & Retrospective`, and `Review Conversation Log` sections must be kept current. Maintain this document in accordance with `docs/PLANS.md`.

## Delivery Metadata

This is the first implementation delivery after the completed documentation baseline `PLAN-0001`. That predecessor is present with `completed` status on `main` and defines the product and architecture inputs used here. This Plan owns one development vertical slice and one Draft Pull Request. It intentionally does not create SQLite state, source acquisition, Trust, Library, workspace/global deployment, cache content, doctor, manager assets, release archives, or placeholders for those operations.

The branch begins with `pull_request: pending`. After the initial planning commit is pushed and its Draft Pull Request is opened, replace that value with the canonical HTTPS URL, record the publication evidence in `Progress`, commit, and push again. No implementation may begin until a later human prompt explicitly authorizes the `execute-exec-plan` workflow.

## Product Baseline

This delivery fully implements and verifies four atomic Revision 1 behaviors.

* `SKL-CLI-002` Revision 1 in `docs/product-specs/cli-contract.md` requires no-argument invocation to print top-level help, exit zero, and create no file or network request.
* `SKL-CLI-003` Revision 1 in `docs/product-specs/cli-contract.md` forbids aliases, removed commands, hidden UI/server surfaces, and stub subcommands. The current development schema contains only real configuration leaves plus text meta invocations; representative `add`, `rm`, `use`, `init`, `claude`, `codex`, `tui`, `web`, `collection`, and not-yet-implemented canonical domain commands remain usage errors with no state change.
* `SKL-OPS-006` Revision 1 in `docs/product-specs/cache-and-operations.md` defines a required version-1 `config.toml` whose only configurable keys are `cache_limit_bytes`, `agents.claude.executable`, and `agents.codex.executable`. The cache value is a positive integer through 9,223,372,036,854,775,807; its absent default is 536,870,912. An Agent value is a nonempty valid-UTF-8 absolute path after current-directory-independent lexical normalization, is stored without probing or executing it, and when absent reports no override plus the fixed basename `claude` or `codex`. Unknown fields, wrong types, unsupported schema versions, relative paths, and invalid numbers fail without rewrite. Repeated set/unset operations are idempotent.
* `SKL-CLI-011` Revision 1 in `docs/product-specs/cli-contract.md` exposes exactly those keys through `config get|set|unset|list`. It requires human and JSON projections, API-v1 `DecimalU64` for the cache value, API-v1 `PathValue` for configured executable paths, strict type/schema validation, no secret handling, and the common successful `changed`/`unchanged` outcomes.

The implementation must demonstrate the current revisions exactly, then update product-spec status prose without changing the behavior text or revision numbers. `docs/product-specs/README.md` currently says every behavior is planned; execution must make that statement accurate for a partially implemented product. `docs/product-specs/cli-contract.md` and `docs/product-specs/cache-and-operations.md` must identify the four behaviors above as implemented and leave all others planned.

Several other Revision 1 specifications constrain this slice without becoming completed acceptance claims. The implementation uses the four-root validation rules from `SKL-OPS-001`, lazy and offline behavior from `SKL-OPS-005` and `SKL-OPS-008`, safe streams and permissions from `SKL-OPS-009`, and the applicable JSON, error, idempotence, human encoding, and evolution rules from `SKL-CLI-004`, `SKL-CLI-005`, `SKL-CLI-007`, `SKL-CLI-009`, and `SKL-CLI-012`. Those broader IDs remain planned because their acceptance spans commands and domains not present in this delivery. The cache-quota enforcement portions of `SKL-CACHE-003` also remain planned even though configuration stores its limit and default.

The development package version is `0.0.1`. This follows the incomplete-artifact constraint in `SKL-PROD-005` without claiming the complete `0.1.0` command surface, platform matrix, or distribution behavior. Cargo package metadata uses Apache-2.0 and the repository license, preserving but not completing the release-archive acceptance in `SKL-PROD-007`.

## Design and Architecture Inputs

`ARCHITECTURE.md` requires a root Cargo workspace with reusable `skilload-core` and thin `skilload-cli` crates, inward dependencies, presentation-neutral application results, native paths retained as bytes until the CLI boundary, and no CLI-owned filesystem policy. This delivery creates that shape and only the configuration modules needed now. It must not add a second direct filesystem path from command handlers or create dormant modules that return not-implemented errors.

`docs/design-docs/application-and-persistence.md` owns the configuration document, XDG roots, application facade, ports/adapters, and lazy creation approach. In particular, all four effective application roots are resolved before any skilload state access: an XDG home is used only when nonempty and absolute, otherwise the corresponding absolute `HOME` fallback is used; the appended `skilload` roots are lexically normalized, existing prefixes are resolved without creating missing components, and every pair must be non-equal and non-ancestral even through symlink aliases. A query over absent state is an in-memory default view and creates nothing. A successful configuration mutation creates only the configuration root and the operational lock path it needs.

`docs/design-docs/cli-json-and-release.md` owns command parsing and rendering. Use one `clap` schema, disable its automatic `help` subcommand, register no alias, and define only the four implemented configuration leaves. No-argument invocation renders current top-level help successfully. `--help` and `--version` are conventional text-only meta invocations and reject `--json`. `--json` and `--no-color` are global presentation flags for configuration leaves and may appear before or after the nested command.

`docs/product-specs/api-v1.md` fixes the machine shape. The four operation identifiers and data/outcome pairs are:

    config.get    -> ConfigEntryData,    observed
    config.set    -> ConfigEntryData,    changed | unchanged
    config.unset  -> ConfigEntryData,    changed | unchanged
    config.list   -> ConfigEntriesData,  observed

`ConfigEntry` has required `key`, `configured`, `value`, `default_value`, and `default_command` fields. Cache entries use nullable `DecimalU64` `value`, non-null default `"536870912"`, and null command. Agent entries use nullable `PathValue` `value`, null default value, and the non-null fixed command. List order is cache, Claude, then Codex. Every response has one API-v1 success or error envelope and JSON stdout contains no other byte.

Exact selected external versions and primary sources are recorded in `docs/references/rust-foundation-versions.md`. The implementation uses Rust 1.97.1 with edition 2024; direct crate requirements begin at `clap 4.6.6`, `serde 1.0.229`, `serde_json 1.0.151`, `toml 1.1.4`, `thiserror 2.0.20`, `base64 0.23.1`, and `tempfile 3.27.0`; CLI tests use `assert_cmd 2.2.2` and `predicates 3.1.4`. Commit the exact resolution in `Cargo.lock`. No Node, SQLite, HTTP, Git, Agent, async runtime, telemetry, or logging dependency belongs in this delivery.

## Purpose / Big Picture

The repository currently contains only specifications and design documents. This change gives later domain Plans a real, tested inward-facing Rust structure and gives users one complete product slice rather than a scaffold. In an isolated home, a user can inspect defaults without creating files, persist a cache limit or absolute Agent executable override, receive the same meaning in human and JSON modes, repeat the operation without rewriting state, and return to defaults by unsetting it.

The deliberately narrow development command surface is important. Building all 50 final parser leaves now would violate the no-placeholder rule because their application operations do not exist. This Plan proves the architecture with a real vertical slice and lets later Plans add a command only with its implementation and acceptance.

## Progress

- [x] (2026-08-18 12:22Z) Established a clean, up-to-date `main` baseline, verified mise and GitHub authentication, audited all Plan states and the documentation-only implementation, selected `PLAN-0002`, and created `codex/p1-config-foundation`.
- [x] (2026-08-18 12:22Z) Verified current Rust, crate, standard-library locking, and CI action inputs against primary sources; recorded them in `docs/references/rust-foundation-versions.md` and authored this self-contained `plan`-status delivery.
- [x] (2026-08-18 13:09Z) Committed and pushed the initial planning baseline as `c21211b0d1aa55e2c422d6d5929bf65457fb5a91`, opened Draft PR https://github.com/bootids/skilload/pull/2, wrote its canonical URL into frontmatter, and published this metadata evidence; awaiting a later explicit execution trigger.
- [x] (2026-08-18 13:26Z) Received explicit execution authorization, reran `mise install` and GitHub/Draft-PR preflight, confirmed `PLAN-0001` is completed on `origin/main`, and entered `active` before implementation.
- [x] (2026-08-18 13:49Z) Implemented the locked Rust workspace, root CI matrix, strict configuration domain/application/filesystem adapter, four real CLI leaves, API-v1 JSON and safe human projections, 15 core unit/adapter tests, five CLI unit tests, and six isolated CLI integration tests.
- [x] (2026-08-18 13:54Z) Pushed implementation commit `05ad0c9ae39f244d4287194249b09e29bb56ecff`; [CI run 32144984316](https://github.com/bootids/skilload/actions/runs/32144984316) passed its `ubuntu-24.04` and `macos-15` jobs, including locked format, Clippy, and test checks.
- [x] (2026-08-18 13:56Z) With the active implementation at `5faf8ff8a5f06087e572e0c8c20e63ebc0f85b36`, ran `gh pr ready https://github.com/bootids/skilload/pull/2` and observed `isDraft: false` plus the same `headRefOid`; CI run 32145189606 also passed both required jobs for that exact head.
- [x] (2026-08-18 13:56Z) Created the review-state change that moves this sole Plan copy to `docs/exec-plans/review/` and sets `status: review`.
- [ ] After a later explicit human merge prompt, use `merge-exec-plan` to pass preflight, complete and push the Plan, merge, update local `main`, and delete the local delivery branch.

## Surprises & Discoveries

- Observation: The default branch contains one completed documentation Plan and no Cargo manifest, source file, test runner, or open Pull Request; all 123 runtime behavior IDs remain planned.
  Evidence: `rg --files` listed only governance/product/design/reference documents and `gh pr list --state all` listed only merged PR #1.
- Observation: A partial parser containing all final command names would immediately conflict with `SKL-CLI-003` because every accepted leaf must dispatch to real behavior.
  Evidence: `SKL-CLI-001` requires each accepted path to reach an operation, while `SKL-CLI-003` rejects scaffold-only subcommands.
- Observation: Rust 1.97.1 already includes standard-library cross-process file locks, so the first mutation lock needs no extra crate.
  Evidence: the official `std::fs::File` documentation marks `lock`, `try_lock`, and `unlock` stable since Rust 1.89.
- Observation: The mise-installed Rust 1.97.1 toolchain initially lacked Clippy even though `rust-toolchain.toml` names it as a component.
  Evidence: `mise exec -- cargo clippy` reported that `cargo-clippy` was not installed; `mise exec -- rustup component add clippy` installed it under the same pinned toolchain, after which the warning-free workspace lint passed.
- Observation: macOS resolves temporary-directory paths through `/private/var/...`, so XDG root tests must compare canonical existing prefixes rather than the textual temporary-directory spelling.
  Evidence: the XDG resolver's filesystem-identity check returned the canonical `/private/var/...` path for a `/var/...` temporary fixture.
- Observation: GitHub forced the pinned `jdx/mise-action` SHA from deprecated Node.js 20 onto Node.js 24 and emitted a warning, but both CI jobs passed.
  Evidence: CI run 32144984316 emitted the same Node.js warning for `ubuntu-24.04` and `macos-15`; the exact action pin remains verified and its warning is captured in `docs/references/rust-foundation-versions.md`.

## Decision Log

- Decision: Make the first implementation delivery a complete configuration vertical slice plus the Rust workspace, not a full parser scaffold or a database-only layer.
  Rationale: Users can observe and accept the slice independently, while every exposed product leaf has real behavior and later domains can reuse the core/application/CLI boundaries.
  Date/Author: 2026-08-18 / Codex
- Decision: Treat `SKL-CLI-002`, `SKL-CLI-003`, `SKL-OPS-006`, and `SKL-CLI-011` as the only completed product IDs in this PR.
  Rationale: Broader XDG, cache, JSON, error, idempotence, and offline behaviors span absent operations; implementing their mechanisms for configuration does not prove their complete acceptance.
  Date/Author: 2026-08-18 / Codex
- Decision: Version the development crates and binary as `0.0.1` and expose only `config get|set|unset|list` plus text help/version.
  Rationale: The product specification reserves `0.1.0` for the complete CLI and explicitly forbids stubs. Unknown future commands are safer than misleading placeholders.
  Date/Author: 2026-08-18 / Codex
- Decision: Pin Rust 1.97.1 in mise and rustup metadata, use edition 2024, select the minimal direct crate set recorded in the Rust reference, and commit `Cargo.lock`.
  Rationale: This is the deferred foundation decision from the design documents, uses current supported inputs, and makes local/CI resolution repeatable without Node.
  Date/Author: 2026-08-18 / Codex
- Decision: Use a typed strict TOML model and canonical full-document replacement; when the final configured key is unset, retain a valid version-only file if a file already existed, but do not create a file for an absent-state unset.
  Rationale: The schema requires `version = 1`, comments are explicitly not preserved, and this distinguishes a real mutation from an absent no-op while keeping every existing document valid.
  Date/Author: 2026-08-18 / Codex
- Decision: Resolve and compare all four XDG application roots for every state-bearing config operation, even though only config and state-lock paths are used.
  Rationale: The architecture forbids reading any skilload state before proving root separation; doing this in the shared adapter prevents a later domain from weakening the invariant.
  Date/Author: 2026-08-18 / Codex
- Decision: Use standard-library advisory file locking with a two-second bounded acquisition, optimistic no-op reads, re-read/revalidation under the lock for actual writes, and same-directory atomic replacement.
  Rationale: This prevents lost updates and partial documents without creating operational state for an absent idempotent no-op or adding a locking dependency.
  Date/Author: 2026-08-18 / Codex
- Decision: Compare the raw configuration baseline beneath the lock and retry the application mutation on a stale baseline.
  Rationale: Two concurrent setters for different keys can then serialize and retain both updates, while an externally changed invalid document is never rewritten from a stale in-memory view.
  Date/Author: 2026-08-18 / Codex
- Decision: Reject `--json` without a configuration operation and with text-only help/version, while accepting `--no-color` as a no-op presentation choice.
  Rationale: API-v1 has no operation identifier or envelope for meta invocations, and a JSON flag must not create an undocumented response shape.
  Date/Author: 2026-08-18 / Codex
- Decision: Treat a whitespace-delimited trailing option token in an Agent executable setting as command-line intent, but preserve an absolute filesystem path whose name merely contains spaces.
  Rationale: This rejects inputs such as `/usr/bin/claude --version` without needlessly rejecting valid native path names.
  Date/Author: 2026-08-18 / Codex

## Outcomes & Retrospective

The implementation now provides a `0.0.1` Rust binary with only `config get|set|unset|list`, text help/version, strict version-1 TOML validation, all-four-root XDG validation, lock-protected atomic writes, and API-v1/human result projections. It deliberately leaves every non-configuration domain absent rather than exposing a placeholder command. The product-spec index, owning specifications, architecture, and two affected design documents now identify exactly the four implemented Revision 1 behaviors and continue to mark all remaining behavior planned.

Local acceptance on 2026-08-18 passed `mise exec -- cargo fmt --all --check`, `mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings`, `mise exec -- cargo test --workspace --all-features --locked` (26 tests: 15 core, five CLI-unit, six CLI-integration), `mise exec -- cargo build --workspace --all-features --locked`, and `git diff --check`. An isolated manual run printed `skilload 0.0.1`, returned the three ordered default configuration entries as one JSON document, changed the cache and Claude settings, returned the padded `PathValue`, and after mutation left only the config document plus the state lock hierarchy; data and cache roots remained absent. [CI run 32144984316](https://github.com/bootids/skilload/actions/runs/32144984316) passed the same locked format, lint, and test matrix on Ubuntu 24.04 and macOS 15.

Review outcome: PR [#2](https://github.com/bootids/skilload/pull/2) is ready for human review. The ready transaction used implementation head `5faf8ff8a5f06087e572e0c8c20e63ebc0f85b36`, which [CI run 32145189606](https://github.com/bootids/skilload/actions/runs/32145189606) passed on both required runners. The Product Baseline remains Revision 1 of `SKL-CLI-002`, `SKL-CLI-003`, `SKL-OPS-006`, and `SKL-CLI-011`. This review-state commit records lifecycle metadata only; a later explicit merge authorization must complete the review-conversation preflight and final archive transition.

## Review Conversation Log

No review conversation has been processed.

## Context and Orientation

The repository root is `/Users/yangxuhui/Projects/Products/skilload`. At Plan creation, it contains no `Cargo.toml`, `Cargo.lock`, `mise.toml`, `rust-toolchain.toml`, `crates/`, `tests/`, or CI workflow. `ARCHITECTURE.md` describes those paths as planned. `skilload-core` is the inward library that owns validation, use cases, ports, and filesystem adapters. `skilload-cli` is the outward adapter that owns arguments, human rendering, JSON serialization, and exit status.

A configuration document is the UTF-8 TOML file at the resolved config application root's `config.toml`. Its model is:

    version = 1
    cache_limit_bytes = 1073741824

    [agents.claude]
    executable = "/opt/claude/bin/claude"

    [agents.codex]
    executable = "/opt/codex/bin/codex"

Only `version` is required. Omit unset settings and omit an empty Agent table. The serializer writes stable LF text with `version` first, then cache, Claude, and Codex in that order, and one trailing newline. A document containing any other key/table, a duplicate, unsupported version, wrong type, zero/negative/overflow cache value, or invalid executable path is invalid and is never rewritten implicitly.

The four base environment variables are `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`, and `XDG_CACHE_HOME`. Their fallbacks are respectively `$HOME/.config`, `$HOME/.local/share`, `$HOME/.local/state`, and `$HOME/.cache`. Append `skilload` after choosing each absolute base. A relative or empty XDG value behaves as absent; a fallback requires an absolute nonempty `HOME`. Lexically remove `.` and process `..` without consulting the current directory or crossing the absolute root. Resolve the longest existing prefix through filesystem identity, append missing suffix components, and reject inaccessible, changing, equal, nested, or aliased effective roots before opening `config.toml`.

`DecimalU64` is a canonical unsigned decimal JSON string, not a JSON number. `PathValue` has `display` and `bytes_base64`. On supported Unix hosts, encode exact `OsStr` bytes with padded RFC 4648 standard base64. The display string uses the product's injective terminal-safe encoding without outer quotes: escape backslash and double quote, use visible `\n`/`\r`/`\t`, encode other C0 controls U+0000-U+001F, DEL U+007F, C1 controls U+0080-U+009F, U+2028, U+2029, U+061C, U+200E-U+200F, U+202A-U+202E, and U+2066-U+2069 as uppercase `\u{XXXX}` padded to four through six digits, and encode each invalid UTF-8 byte as uppercase `\xHH`. Human output adds the surrounding quotes for every path/data field. Never use lossy UTF-8 conversion.

The applicable API-v1 error details remain typed. `UsageDetails` carries nullable argument/value/path plus required expected strings; `ValidationDetails` carries a constraint and nullable source/source-path/native-path fields; `EnvironmentDetails` carries the environment variable, nullable native path, and reason; `BusyDetails` carries lock domain and waited milliseconds; `SchemaDetails` carries domain, found version, and supported version; `InvalidStateDetails` carries domain, observed state, and expected states; and `InternalDetails` carries a nonempty incident ID. Usage errors exit 2, environment/validation/schema preconditions use their cataloged exit 4 or 6, busy exits 5, and internal invariant failures exit 6. A known configuration leaf in JSON mode returns its typed error envelope. A parser failure before any operation identifier exists, including an unknown subcommand or JSON combined with help/version, uses conventional usage stderr and exit 2 rather than fabricating an operation.

## Plan of Work

### Milestone 1: Create the locked Rust workspace

Add root `Cargo.toml`, `Cargo.lock`, `mise.toml`, and `rust-toolchain.toml`. The workspace uses resolver 3, edition 2024, `rust-version = "1.97.1"`, shared version `0.0.1`, Apache-2.0, `publish = false`, and forbids unsafe code. Extend `.gitignore` with only the Rust `/target/` output while preserving the existing `.agents/` entry.

Create `crates/skilload-core` as a library and `crates/skilload-cli` as the binary package named `skilload`. Put dependency versions in `[workspace.dependencies]` and enable only needed features: `clap` derive/std/help/usage/error-context/suggestions without color or automatic extras; `serde` derive; `toml` parse/display/serde/std without order-preserving maps; the other selected crates use their standard feature set. Add no placeholder module or command.

Add `.github/workflows/ci.yml` with read-only contents permission and a matrix over `ubuntu-24.04` and `macos-15`. Pin checkout and mise actions to the immutable commits recorded in the Rust reference. Each job runs mise installation, format checking, warning-free Clippy over all targets/features, and locked workspace tests. At this milestone, `mise install` followed by `mise exec -- cargo test --workspace --locked` must compile both crates even before the later behavior tests are filled in.

### Milestone 2: Implement configuration domain, application, and storage

In `crates/skilload-core/src/domain/configuration.rs`, define exact keys, typed values, ordered entries, schema version, default cache limit, configured/default distinction, lexical Agent-path validation, and mutation outcomes. Keep native paths as `PathBuf`/`OsString` values; do not base64-encode or format them in core.

In `crates/skilload-core/src/application/configuration.rs` and `src/lib.rs`, expose one application facade for the four use cases. Queries return presentation-neutral typed data. Mutations validate input before persistent staging, compare the desired document with the loaded model, and return `Changed` or `Unchanged` without rendering.

In `crates/skilload-core/src/ports/configuration.rs`, define focused environment/root/configuration-store/lock boundaries rather than exposing `std::fs` to the application service. In `src/adapters/xdg.rs` and `src/adapters/configuration.rs`, implement the production Unix filesystem adapters. Reads over absent state return defaults and create nothing. Writes optimistically detect no-ops, create `state/locks/config.lock` only when a real mutation must be serialized, acquire a bounded exclusive standard-library lock, re-resolve roots and re-read state, then stage a mode-0600 file in the config directory, sync it, atomically rename it to `config.toml`, and sync the parent. Created application/lock directories use mode 0700. Never follow a final config-file symlink or replace a non-regular file.

Use failpoint-capable small internal filesystem operations so tests can prove that failure before rename preserves the old bytes, failure after rename returns only after the new file and directory are synced, stale root identity aborts, and leftover staging files are recognizable and removable without adopting them. This is a single-file atomic replacement, not the multi-resource journal that later deployment Plans implement.

In `crates/skilload-core/src/error.rs`, define typed errors and the relevant stable API-v1 details rather than a string map. This slice needs usage/unsupported argument, validation, invalid/overlapping environment roots, bounded busy, schema-newer/migration-required, invalid-state, and internal-invariant categories. Preserve native paths in details. Filesystem access failure while resolving or using an XDG-controlled prefix reports the applicable environment-path category and path; it must not fabricate a database, workspace, or Agent target.

### Milestone 3: Implement the real CLI and output contracts

In `crates/skilload-cli/src/args.rs`, define the four real leaves and global presentation flags. Disable aliases and the generated help subcommand. Parse cache numbers and executable values as raw inputs so the application can return the correct typed validation without losing non-UTF-8 path bytes. Reject `--json` with help/version before operation dispatch. A missing top-level subcommand prints current help to stdout and returns zero.

In `crates/skilload-cli/src/json.rs`, define closed initial producer structs for the API-v1 envelope, `ConfigEntryData`, `ConfigEntriesData`, applicable details variants, `DecimalU64`, and `PathValue`. Serialize exactly once to stdout with one trailing newline and never print progress there. The default cache get JSON must be structurally equal to:

    {
      "api_version": 1,
      "operation": "config.get",
      "ok": true,
      "result": {
        "outcome": "observed",
        "data": {
          "schema_version": 1,
          "entry": {
            "key": "cache_limit_bytes",
            "configured": false,
            "value": null,
            "default_value": "536870912",
            "default_command": null
          }
        }
      }
    }

In `crates/skilload-cli/src/human.rs`, implement one terminal-safe field encoder and concise English configuration rendering. Static layout may use raw spaces/newlines; values and paths use the safe quoted encoder. This first binary emits no ANSI styling, so `--no-color` is accepted and outputs the same bytes. Diagnostics go only to stderr and normal execution writes no persistent log.

In `crates/skilload-cli/src/main.rs`, compose the real environment and filesystem adapters, dispatch exactly one application use case, select human or JSON rendering, and map typed errors to the API-v1 exit categories. Do not inspect Git, run a child process, read stdin, contact a network, or initialize any absent root for help/version/query operations.

### Milestone 4: Prove behavior and synchronize documentation

Add core unit/adapter tests and CLI integration tests under the owning crates. Tests must cover the exact number/path/schema boundaries, all XDG fallback and overlap cases, symlink aliases, no-op inode/mtime stability, concurrent different-key setters, atomic-write failure points, restrictive permissions, invalid native bytes in error paths, and terminal-control fixtures. Integration tests run the compiled binary under fully isolated environment roots and assert stdout, stderr, exit, and complete filesystem snapshots.

Update `ARCHITECTURE.md` to describe the now-present workspace and implemented configuration path while continuing to label SQLite and all other domains planned. Update the status/current-state prose in `docs/design-docs/application-and-persistence.md` and `docs/design-docs/cli-json-and-release.md` without weakening their future design. Update the product-spec index and the two owning product specifications to mark only the four baseline IDs implemented. Do not change behavior text or revisions unless implementation uncovers a real semantic decision; if that happens, stop and obtain the required product decision.

Finish by recording exact test counts, CI URLs/results, dependency resolution, and observable transcripts in this Plan. Commit and push all active implementation and evidence. Mark the Draft PR ready, verify `isDraft: false` and `headRefOid` equal the pushed implementation head, then move this file to `docs/exec-plans/review/`, set `status: review`, record the ready evidence, commit, and push.

## Concrete Steps

Work from `/Users/yangxuhui/Projects/Products/skilload`. After explicit execution authorization, first use the repository's `execute-exec-plan` skill. It must verify `PLAN-0001` is completed on `main`, the PR is Draft, the branch matches frontmatter, and the worktree is clean; then it moves this file to `active/` and pushes that lifecycle commit before code changes.

Install and verify the pinned toolchain:

    mise install
    mise exec -- rustc --version
    mise exec -- cargo --version

Expect Rust `1.97.1`. Create the workspace and source files described above with reviewable patches, then generate and commit the lockfile through the mise-resolved Cargo:

    mise exec -- cargo generate-lockfile
    mise exec -- cargo metadata --locked --format-version 1

Run the local acceptance suite repeatedly:

    mise exec -- cargo fmt --all --check
    mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings
    mise exec -- cargo test --workspace --all-features --locked
    mise exec -- cargo build --workspace --all-features --locked
    git diff --check

The formatter, Clippy, tests, build, and whitespace check must all exit zero. `cargo test` must name and pass the unit/adapter/integration cases described in `Validation and Acceptance`; record the final counts here rather than predicting them.

Use a disposable directory to exercise the binary. Assign every XDG root separately so the overlap check is meaningful:

    tmp="$(mktemp -d)"
    export HOME="$tmp/home"
    export XDG_CONFIG_HOME="$tmp/config"
    export XDG_DATA_HOME="$tmp/data"
    export XDG_STATE_HOME="$tmp/state"
    export XDG_CACHE_HOME="$tmp/cache"
    mkdir -p "$HOME"
    bin="$PWD/target/debug/skilload"

First prove that meta and query invocations create nothing:

    "$bin"
    "$bin" --version
    "$bin" config list --json
    find "$tmp" -mindepth 1 -print

The first two commands exit zero with text. The JSON command emits one API-v1 object containing the three ordered unconfigured entries. `find` prints only the explicitly created `home` directory; no XDG application root exists.

Then exercise persistent values:

    "$bin" config set cache_limit_bytes 1073741824 --json
    "$bin" config set agents.claude.executable /opt/claude/bin/claude --json
    "$bin" config get agents.claude.executable --json
    "$bin" config list
    "$bin" config unset agents.claude.executable --json

The first two responses are `changed`, the get returns a padded-base64 `PathValue` for the exact absolute bytes, list shows the same state in English without control bytes, and unset returns `changed` with `configured: false`, null value, and default command `claude`. Only `$XDG_CONFIG_HOME/skilload/config.toml` and the required state lock hierarchy exist; data and cache roots remain absent.

Capture the config file identity and bytes, repeat an already-satisfied set and unset, and require `unchanged` with identical bytes, inode, and modification time. Exercise values `1` and `9223372036854775807` successfully; reject `0`, negative input, and `9223372036854775808` without a diff. Reject a relative or invalid-UTF-8 Agent path without creating or rewriting the document.

Before review, inspect every changed file:

    git status --short
    git diff --stat main...HEAD
    git diff main...HEAD
    git diff --check

Commit only scoped implementation, tests, synchronized documentation, this living Plan, the reference, toolchain manifests, lockfile, and CI. Push them, wait for the workflow, and record its URL and result. Then perform the ready/review transaction exactly as required by `docs/PLANS.md`.

## Validation and Acceptance

Acceptance for `SKL-CLI-002` runs the binary with no arguments under an isolated empty home. It exits zero, writes current help to stdout, writes no required diagnostic, opens no network capability, and leaves every XDG root absent. `--help` has the same no-state property; `--version` reports `skilload 0.0.1`.

Acceptance for `SKL-CLI-003` extracts every current `clap` leaf and obtains exactly four: `config.get`, `config.set`, `config.unset`, and `config.list`. The schema has no automatic `help` command, alias, hidden TUI/server command, or domain placeholder. Each representative removed/unknown invocation exits 2, makes no state change, and cannot route to another action.

Acceptance for `SKL-OPS-006` proves the absent/default, set, get, list, unset, schema, and validation behavior for all three keys. Boundary tests accept cache limits 1 and 9,223,372,036,854,775,807 and reject zero, negative, overflow, signs/whitespace or non-decimal forms not accepted by the argument grammar. Agent path tests accept an absolute valid-UTF-8 value after lexical normalization, never require the target to exist, and reject empty, relative, current-directory-dependent, multiword command intent, and invalid-UTF-8 values without rewrite. Unknown fields/tables/types, missing/older/newer versions, duplicate keys, a config symlink, and a non-regular file all preserve bytes and receive typed errors. Unsetting the final key in an existing file leaves canonical `version = 1`; unsetting from absent state creates nothing.

Acceptance for `SKL-CLI-011` validates golden human and JSON output for every command and both mutation outcomes. Every JSON document decodes once, has the correct operation/data type, required nullable fields, sorted entries, decimal string, and padded exact-byte path. `--json` with closed stdin never reads or hangs. Help/version with `--json` exit 2 before dispatch. Human fixtures containing quote, backslash, newline, carriage return, tab, ESC/CSI/OSC, BEL, DEL/C1, U+2028/U+2029, U+061C and the listed bidi controls, plus invalid path bytes, emit only the injective visible escapes from the product contract.

Supporting adapter acceptance covers unset, empty, relative, and absolute values for every XDG variable; invalid fallback `HOME`; CWD independence; all equal/nested pairs among four effective roots; two spellings through one symlink; inaccessible/changing prefixes; and final mutation revalidation. Every rejection happens before config/state access or creation. An absent list/get is byte-for-byte filesystem inert. A real mutation creates mode-0700 owned directories and a mode-0600 regular config file on macOS/Linux, never data/cache/Agent/workspace paths. Two concurrent setters for different keys serialize without lost updates; a bounded lock contender returns typed `busy`. Injected failure at each stage yields either the complete prior document or complete new document, never truncated TOML.

The normal dependency graph must contain no HTTP, Git, SQLite, Agent, telemetry, or Node runtime. CI must pass on its pinned Linux and macOS runners. This Plan does not claim arm64/x86_64 release artifacts, the final 50-leaf parser, full API-v1 error coverage, or any domain beyond configuration.

## Idempotence and Recovery

`mise install`, Cargo generation with an unchanged manifest, formatting, linting, tests, builds, and query scenarios are safe to repeat. Use disposable XDG/HOME roots for manual acceptance. Never reuse a developer's real configuration directory and never delete an unfamiliar path.

Configuration reads never create. A detected no-op returns before staging. A real write uses one persistent lock, re-reads after acquiring it, stages in the destination directory, and replaces only after validation and sync. A crash before rename leaves the old file authoritative; a recognizable same-directory temp may be removed only when it is not the active locked stage. A crash after rename leaves the complete new file. Root or baseline identity drift aborts and retries from a fresh read; it never follows the drift.

An invalid/unsupported document is preserved byte-for-byte. Revision 1 has no silent migration. The implementation may report an older schema as `migration_required` and a newer schema as `schema_newer`, but neither path rewrites. Do not invent a reset, repair, or force option.

On workflow rerun, reconcile and reuse this branch, Plan, reference, and Draft PR. If material scope exceeds the configuration vertical slice, return to planning and split an independently acceptable later Plan rather than adding a second Plan to this PR or exposing stubs.

If `gh pr ready` fails, keep the Plan `active` and the PR Draft. If ready conversion succeeds but the review move, commit, or push fails, run `gh pr ready <pull_request> --undo`, verify `isDraft: true`, and restore/keep the Plan in `active` before retrying. If review reveals materially incomplete scope, first return the PR to Draft and verify it, move the Plan back to `active`, record and push the reason, then resume only through `execute-exec-plan`. If publishing that reverse transaction fails, restore `review` and ready state.

After explicit merge authorization, if any required check, repeated gate, queue attempt, or merge fails before GitHub reports `MERGED`, restore this Plan to `review`, record the failure, and push. A `completed` declaration becomes the official archive only after the merge enters `main`.

## Artifacts and Notes

Planning baseline on 2026-08-18:

    $ git status --short --branch
    ## main...origin/main

    $ git rev-list --left-right --count main...origin/main
    0       0

    $ find docs/exec-plans -maxdepth 2 -type f -print
    docs/exec-plans/completed/p0-product-architecture-baseline.md
    ...status-directory .gitkeep files...

    $ gh pr list --state all
    #1 merged: docs: establish skilload product and architecture baseline

The only predecessor is `PLAN-0001`. No implementation or open delivery exists to reuse. Current external version evidence and action commit pins are retained in `docs/references/rust-foundation-versions.md` rather than only in this Plan or chat.

Implementation evidence on 2026-08-18:

    $ mise exec -- rustc --version
    rustc 1.97.1 (8bab26f4f 2026-07-14)

    $ mise exec -- cargo test --workspace --all-features --locked
    15 core tests passed; 5 CLI unit tests passed; 6 CLI integration tests passed.

    $ target/debug/skilload config list --json
    {"api_version":1,"operation":"config.list","ok":true,...}

    $ target/debug/skilload config set cache_limit_bytes 1073741824 --json
    {"api_version":1,"operation":"config.set","ok":true,"result":{"outcome":"changed",...}}

    $ gh run view 32144984316
    CI succeeded: ubuntu-24.04 and macos-15 passed format, Clippy, and locked tests.

    $ gh pr view https://github.com/bootids/skilload/pull/2 --json isDraft,headRefOid
    {"isDraft":false,"headRefOid":"5faf8ff8a5f06087e572e0c8c20e63ebc0f85b36"}

    $ gh run view 32145189606
    CI succeeded: ubuntu-24.04 and macos-15 passed for implementation head 5faf8ff.

    $ find "$tmp" -mindepth 1 -print
    .../home
    .../config/skilload/config.toml
    .../state/skilload/locks/config.lock

## Interfaces and Dependencies

In `crates/skilload-core/src/domain/configuration.rs`, define these semantic interfaces, refining field visibility without changing meaning:

    pub const CONFIG_SCHEMA_VERSION: u16 = 1;
    pub const DEFAULT_CACHE_LIMIT_BYTES: u64 = 536_870_912;

    pub enum ConfigKey {
        CacheLimitBytes,
        ClaudeExecutable,
        CodexExecutable,
    }

    pub enum ConfigValue {
        CacheLimitBytes(u64),
        Executable(NativePath),
    }

    pub struct ConfigEntry {
        pub key: ConfigKey,
        pub configured: bool,
        pub value: Option<ConfigValue>,
        pub default_value: Option<u64>,
        pub default_command: Option<&'static str>,
    }

    pub struct ConfigEntries {
        pub schema_version: u16,
        pub entries: [ConfigEntry; 3],
    }

    pub enum MutationOutcome {
        Changed,
        Unchanged,
    }

`NativePath` owns a native `PathBuf` and exposes no lossy string conversion. `ConfigKey` iteration is fixed in API order. Cache input validates in checked `u64` space but caps persistent TOML at `i64::MAX`. Executable input remains `OsString` until core has rejected invalid UTF-8, then validates and stores an absolute normalized path without filesystem probing.

In `crates/skilload-core/src/ports/configuration.rs`, define:

    pub trait Environment {
        fn var_os(&self, key: &str) -> Option<OsString>;
    }

    pub trait StateRootResolver {
        fn resolve(&self, environment: &dyn Environment) -> Result<ResolvedRoots, AppError>;
        fn revalidate(&self, roots: &ResolvedRoots) -> Result<(), AppError>;
    }

    pub trait ConfigurationStore {
        fn load(&self) -> Result<LoadedConfig, AppError>;
        fn replace(
            &self,
            expected: &ConfigBaseline,
            desired: &ConfigDocument,
        ) -> Result<StoreOutcome, AppError>;
    }

`LoadedConfig` contains the validated `ConfigDocument` plus an opaque `ConfigBaseline` carrying exactly the file/root identity evidence needed by `replace`; absent state has an explicit absent baseline. The store owns serialization, locking, revalidation, staging, permissions, sync, and atomic replacement. The application never receives a file handle or path to mutate directly.

The public `Application` facade provides:

    pub fn config_get(&self, key: ConfigKey) -> Result<ConfigEntry, AppError>;
    pub fn config_list(&self) -> Result<ConfigEntries, AppError>;
    pub fn config_set(
        &self,
        key: ConfigKey,
        raw_value: OsString,
    ) -> Result<ConfigMutation, AppError>;
    pub fn config_unset(&self, key: ConfigKey) -> Result<ConfigMutation, AppError>;

`ConfigMutation` carries `MutationOutcome` and the post-operation `ConfigEntry`. `AppError` carries a stable code and typed details with native paths; it contains no terminal prose assembled by core. The CLI adds English messages and maps exit categories.

At the workspace root, declare compatible direct requirements using the exact starting versions in the Rust reference, then treat committed `Cargo.lock` as the executable dependency snapshot. `skilload-core` depends on `serde`, `toml`, `thiserror`, and `tempfile`. `skilload-cli` depends on `skilload-core`, `clap`, `serde`, `serde_json`, and `base64`. CLI integration tests additionally use `assert_cmd`, `predicates`, and `tempfile`. No other direct dependency is authorized by this Plan without a recorded discovery and Decision Log entry.

Plan revision note: created on 2026-08-18 to turn the completed product/architecture baseline into the smallest real implementation slice. It selects the Rust foundation and exact current direct inputs, fully scopes four Revision 1 behaviors, forbids placeholder domain commands, and defines the application, storage, CLI, validation, documentation, and lifecycle evidence needed for an independently reviewable delivery. The same day, initial commit `c21211b0d1aa55e2c422d6d5929bf65457fb5a91` was pushed, Draft PR https://github.com/bootids/skilload/pull/2 was opened, and its canonical URL plus publication evidence were recorded before the required metadata push. Execution completed on the same day: implementation head `5faf8ff8a5f06087e572e0c8c20e63ebc0f85b36` passed CI, the PR was made ready, and this Plan moved to `review` pending human review and a later explicit merge authorization.
