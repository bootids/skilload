# CLI, JSON, Testing, and Release Design

Status: planned design for the 0.1 CLI MVP. It implements `SKL-CLI-*`, `SKL-PROD-002`, and `SKL-PROD-004` through `SKL-PROD-007`.

## Behavior Traceability

* Command parsing, common arguments, JSON, confirmation, errors, idempotency, streams, and compatibility implement `SKL-CLI-001` through `SKL-CLI-012`.
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

## Common Arguments

Common presentation arguments are accepted at a documented consistent position:

* `--json` selects JSON mode and disables all prompts, spinners, color, and progress on stdout.
* `--confirm-token <opaque>` supplies the token returned by a prior JSON confirmation requirement.
* `--no-color` disables ANSI styling in human mode; non-TTY output defaults to no styling.
* Explicit resource-limit or cache-quota overrides belong only to operations that need them and appear in preview/results.

Workspace sync and manager operations accept repeated or multi-value `--agent` and require at least one value. Global install/uninstall/sync/status require one or more `--agent` values when resolving current-environment profiles, or use stored `--profile <id>`/`--all-profiles` selection where specified. Avoid global flags whose presence silently changes product-domain scope.

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

`result.outcome` is one of `changed`, `unchanged`, `already_exists`, or `already_immutable` where appropriate. Domain data uses explicit typed fields, canonical source strings, lowercase full commit SHA, `sha256:` integrity, opaque profile IDs, and paths encoded as display-safe strings plus a lossless representation when required. Although `RepositoryId` is a `u64` internally, portable JSON/YAML serializes it as a decimal string to avoid IEEE-754 precision loss in common consumers. Lists have documented stable ordering.

Progress and diagnostics go to stderr. In JSON mode, stderr remains optional operational diagnostics and never carries data required to complete the workflow. Secrets and confirmation tokens are redacted from debug output; the token appears only in its JSON response and is stored hashed.

API version 1 only gains optional fields. Required-field removal/rename or semantic reinterpretation requires a new top-level API version and compatibility/migration design.

## Errors and Exit Status

Define stable string error codes in `skilload-core`; rendering never derives them from prose. Initial families include:

* syntax/usage and unsupported command/argument;
* `not_found`, `already_immutable` (success), and invalid state;
* `confirmation_required`, invalid/expired/stale confirmation;
* `trust_required`, authentication, permission, or source unavailable;
* duplicate/reserved/exact/semantic conflict and `agent_disabled`;
* `busy`, stale baseline, inaccessible profile/workspace;
* validation, limit, integrity, cache corruption, and unsupported entry;
* schema newer/migration required/database corrupt;
* recovery blocked and internal invariant failure.

Exit categories are stable but callers use JSON `error.code` for detail:

    0  successful, including idempotent outcomes
    2  CLI syntax or usage
    3  confirmation required/invalid confirmation
    4  domain precondition, not found, Trust, or conflict
    5  external unavailability, busy, permission, or network/auth
    6  integrity, schema, recovery, or internal invariant failure

Human errors use the same code internally, an English message, relevant paths/sources, and actionable next command where one exists. Never suggest destructive ownership bypass.

## Command/Query Network Policy

Application request types declare one of `Offline`, `MayResolve`, or `RequiresResolve`. The production network/Git port refuses access for an `Offline` request even if a handler accidentally calls it. Offline includes help/version, Library reads and metadata-only changes, Trust reads/revoke, workspace list/status/delete/remove where no restoration occurs, global list/status/uninstall, manager operations, cache info/prune/clear, config, and doctor/fix. Cleanup remains possible without source access.

Network-capable requests are explicit source add/Trust add, Library refresh, source or workspace source migration, workspace lock/update/pin and a sync with a cache miss, global install/update/pin and a sync with a cache miss. Migration resolution may only prove that fresh metadata for the proposed name matches the repository ID stored with the old source; it cannot change path or ref under the migration operation. The application result records whether network and which credential class were used, without returning secret material.

## Human Rendering

Human output is English and optimized for terminal scanning rather than mirroring JSON. Mutation previews show complete selected sources/targets and warnings before confirmation. Success summaries distinguish changed versus unchanged and include degraded states. List/search output has deterministic sort and an explicit machine-independent fallback when width is narrow or stdout is not a TTY.

Do not write persistent logs by default. `--debug` (or a documented environment equivalent) writes redacted diagnostics to stderr. An explicit debug-log destination, if P1 adds it, belongs under XDG state and must be opt-in.

## Test Architecture

Default tests are offline and deterministic:

* unit tests for domain validation, source normalization, integrity encoding, conflict policy, command schema, JSON rendering, and exit mapping;
* repository/adapter contract tests against fakes and SQLite/filesystem implementations;
* temporary bare Git repositories with branches, tags, SHA pins, submodules, LFS pointers, symlinks, executable bits, hostile names, and deleted/unavailable commits;
* local HTTP fixtures for GitHub metadata, redirects, auth/rate errors, default branches, repository ID changes, and candidate trees;
* isolated HOME/XDG/Claude/Codex roots and fake Agent executables/configuration;
* transaction failpoint tests at every journal/filesystem/database phase;
* golden JSON/help/human snapshots with secret-redaction assertions;
* scale fixtures for 10,000 Library entries, 200 workspace Skills, and 100 global targets.

Performance budgets are recorded by the implementation Plan before acceptance, then enforced in nonflaky benchmark/integration thresholds. Tests must measure representative search, status, lock planning, and deployment planning rather than only database insertion.

Real GitHub and real Claude/Codex smoke tests are explicit, credential-aware jobs or scheduled workflows. They are not part of the default suite. A real model conversation with the manager Skill is optional and nonblocking.

## Toolchain and Build

The later P1 foundation adds a root Cargo workspace, committed `Cargo.lock`, `rust-toolchain.toml`, and `mise.toml`. mise pins Rust and any Node/npm/pnpm used only for repository tooling; the released product has no Node runtime dependency.

The binary links SQLite with FTS5 and an HTTPS client suitable for GitHub. Its required external runtime executables are system `git` and only the selected Agent CLI. `gh` remains optional. Build metadata exposes product version, Git commit, target triple, and manager asset version without embedding build-machine paths or timestamps that prevent reproducibility.

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

Maintain versioned fixtures for JSON API 1, workspace config/lock 1, Library export 1, config 1, database migrations, and manager markers. Every 0.1.x build reads prior patch fixtures and preserves required command/field semantics. A breaking format or command change requires a new product behavior revision, explicit migration, and later minor version.
