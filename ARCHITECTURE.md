# skilload Architecture

Status: planned architecture for the 0.1 CLI MVP. The repository does not yet contain the Rust workspace described here.

Product behavior is authoritative in [`docs/product-specs/`](docs/product-specs/README.md). This file defines boundaries, dependency direction, state ownership, and invariants. Technical mechanisms and rationale live in [`docs/design-docs/`](docs/design-docs/).

## System Shape

skilload is one local Rust binary. It resolves and validates GitHub-hosted Skill directories, stores durable metadata and desired state, maintains immutable external content in a removable cache, and reconciles owned links into native Claude Code and Codex Skill roots. It never wraps or launches an Agent.

The planned Cargo workspace is:

    Cargo.toml
    Cargo.lock
    mise.toml
    rust-toolchain.toml
    crates/
      skilload-core/
        src/
          domain/
          application/
          ports/
          adapters/
          library/
          trust/
          source/
          workspace/
          global/
          cache/
          agents/
          persistence/
          recovery/
      skilload-cli/
        src/
    assets/
      manager/
        claude-code/
        codex/

`skilload-core` is the reusable domain/application library. `skilload-cli` is the only 0.1 presentation adapter and produces the single binary. The module names under `skilload-core` identify ownership areas; P1 may group files differently while preserving the boundaries below. Embedded manager assets are source files compiled into the binary, not external cache content.

## Dependency Direction

Dependencies point inward:

    CLI parsing and rendering
              |
              v
      application commands/queries
              |
              v
        domain rules and values
              ^
              |
      ports implemented by adapters
       /       |        |       \
    SQLite  filesystem  Git/API  Agents

* Domain values and rules do not import CLI, SQLite, HTTP, process, filesystem, clock, or Agent-specific code.
* Application services coordinate domain rules through explicit ports. They own use-case transaction boundaries and return presentation-neutral results.
* Adapters implement ports for SQLite/FTS5, XDG/config files, immutable cache, system Git, GitHub metadata HTTP, time/randomness, and Claude/Codex environments.
* The CLI parses arguments, invokes one application command/query, and renders human or JSON output. It does not issue SQL, edit workspace files, run Git, or manage links directly.
* Future TUI, Web, or other interfaces must call the same application layer. They may not bypass Trust, ownership, transaction, or network policies.

See [application and persistence design](docs/design-docs/application-and-persistence.md) for the planned ports and composition.

## Ownership Boundaries

The authoritative owner of each state category is singular:

* Product behavior: `docs/product-specs/`.
* Library, Trust, global desired state, manager ownership, profile identities, and known workspace index: the durable SQLite database.
* Workspace desired sources: `.skilload.yaml` in that exact workspace.
* Workspace resolved versions and integrity: `.skilload.lock` in that workspace.
* External Skill bytes: immutable entries under the XDG cache root; these bytes are removable and are never durable metadata. Canonically resolved config, data, state, and cache application roots are pairwise non-overlapping, so clearing removable content cannot traverse into durable or operational ownership state.
* Workspace derived deployment state: the git-excluded workspace manifest plus the durable workspace index.
* Global and manager derived deployment state: durable ownership records plus the observed Agent roots.
* Incomplete mutation intent: persistent transaction journals under the XDG state root.
* User configuration: versioned `config.toml` under the XDG config root.

No secondary store may silently become authoritative. In particular, a cache entry does not grant Trust, a lockfile does not grant Trust or pin cache retention, Library membership does not create deployment desire, and an observed symlink is not ownership evidence without a matching record and target.

## Domain Boundaries

**Source and Trust** normalize GitHub coordinates including fully qualified branch/tag intent, obtain repository identity, extract inert Git objects, validate a complete Skill tree, compute integrity, and authorize one exact source. They do not decide Library metadata or deployment targets. See `SKL-SRC-*` and `SKL-TRUST-*`.

**Library** owns searchable user metadata keyed by source. It does not own Skill content, Trust, pins, or deployment state. See `SKL-LIB-*`.

**Workspace** owns deterministic portable config/lock files and complete-set project deployment intent. Agent selection is invocation-local. A deployed workspace target is owned by canonical workspace, Agent, and canonical project Skill root; HOME/configuration fingerprints are observations, not a second ownership identity for the same path. A random local workspace instance ID connects the git-excluded manifest to the durable index so an explicit, fully proved rebind can transfer that canonical-path component after a move without placing machine identity in portable files or adopting a lookalike target. See `SKL-WSP-*`.

**Global deployment** owns local per-profile desired targets with one shared pin per source. It consumes Library and Trust but does not mutate either. See `SKL-GLB-*`.

**Manager** owns embedded Agent-specific assets and their copied global installations. It is independent of external sources, Library, Trust, and cache. See `SKL-MGR-*`.

**Cache and recovery** own immutable external materializations, eviction, integrity quarantine, mutation journals, process locks, and verifiable cleanup/repair. They cannot rewrite a product source of truth merely to make observed state look healthy. See `SKL-CACHE-*` and `SKL-OPS-*`.

**Agent adapters** resolve roots, inspect disable/conflict state, and describe link/copy actions. A shared infrastructure resolver discovers every external executable only from configured absolute Agent paths or absolute PATH directories and rejects project/source/cache-contained candidates before any probe. Agent adapters never parse business CLI arguments or make policy decisions about confirmation.

## Cross-Cutting Invariants

1. External repository content is always untrusted inert data. Source-controlled code, hooks, filters, submodules, LFS downloads, and executable candidates reached through relative/project-contained PATH locations are never executed (`SKL-SRC-008` through `SKL-SRC-016`, `SKL-WSP-022`, `SKL-OPS-010`).
2. Trust is exact, local, explicit, namespace-preserving for Git refs, and separate from Library/workspace/export state (`SKL-SRC-002` through `SKL-SRC-004`, `SKL-TRUST-001` through `SKL-TRUST-008`).
3. A successful pin names one repository ID, commit, Skill path, verified name, and canonical integrity. Sync never substitutes another commit (`SKL-SRC-012` through `SKL-SRC-015`).
4. External cache entries are managed as immutable: skilload never edits a promoted object in place and never promotes, links, or reuses an object after detecting failed integrity. Native Agents read deployed symlinks without passing through skilload, so a post-deployment same-account modification or disk fault can be consumed until the next skilload integrity observation; that observation reports or quarantines the object according to command mutability (`SKL-CACHE-001`, `SKL-CACHE-005`).
5. skilload creates, replaces, removes, or rebinds only paths it can prove it owns. An exact foreign target is never overridden, including with force; a moved workspace requires the matching local instance, durable transaction/link evidence, absent old path, and explicit complete-Agent rebind (`SKL-WSP-019`, `SKL-WSP-023`, `SKL-GLB-010`).
6. Product mutations are planned and committed through application services. Normal success means every participating resource committed; crash recovery converges journaled work to a coherent old or new state (`SKL-CACHE-008`, `SKL-CACHE-009`).
7. Read-only commands do not create state or perform network access (`SKL-OPS-005`, `SKL-OPS-008`, `SKL-CLI-012`).
8. Schema and source migrations are explicit. Reads and sync never rewrite an unknown/older format (`SKL-SRC-015`, `SKL-WSP-014`, `SKL-OPS-003`, `SKL-OPS-006`).
9. Human and JSON interfaces represent the same application outcome. JSON stdout is one versioned envelope and never prompts; human rendering quotes and escapes every untrusted field before it can reach a terminal, valid logical strings use standards-compliant JSON escaping, and every filesystem path uses one display-plus-base64 `PathValue` so native bytes are lossless (`SKL-CLI-004` through `SKL-CLI-009`).
10. No external executable is discovered or probed through an empty, relative, current-project, source-staging, or external-cache search location; candidate identity is checked before probe and revalidated before use (`SKL-SRC-016`, `SKL-WSP-022`, `SKL-MGR-006`).
11. No runtime subsystem may add a wrapper, daemon, server, TUI, or Web surface to 0.1 (`SKL-PROD-002`, `SKL-PROD-003`).

## Mutation Model

Every mutation follows the same architecture:

1. Recover or report any prior incomplete journal relevant to the target.
2. Acquire bounded domain/workspace locks and read a versioned baseline.
3. Resolve and validate all inputs. Network work may be staged outside the final critical section.
4. Reacquire/revalidate the baseline, build a complete action plan, and require confirmation when policy says so.
5. Persist a journal containing old/new ownership evidence and staged artifacts.
6. Apply reversible filesystem changes, commit the durable database anchor and/or workspace pair, and mark the journal complete.
7. Return success only after final verification; cleanup temporary backups after the commit is recoverable.

This is recoverable command atomicity, not a claim that unrelated filesystems and SQLite support one instantaneous transaction. Details are in [deployment transactions and recovery](docs/design-docs/deployment-transactions-and-recovery.md).

## External Boundaries

The only required external executables are system `git` for source object retrieval and whichever selected Agent CLI is being managed. `gh` is optional as an authenticated metadata-token source. Every discovery uses the shared trusted resolver from `SKL-WSP-022`; only `agents.claude.executable` and `agents.codex.executable` may override a basename, and those values are absolute paths. SQLite with FTS5 and the GitHub HTTP client are linked into the binary. Node.js is not a product runtime dependency.

Network access is limited to the operations named by `SKL-OPS-008` and only GitHub.com is a content source. Agent directories and their version-sensitive behavior are isolated behind adapters and recorded in [the Agent discovery reference](docs/references/claude-and-codex-skill-discovery.md).

## Documentation Change Rules

Change product specifications first when user-visible behavior or acceptance changes. Change this file when module ownership, dependency direction, state authority, or a cross-cutting invariant changes. Change a design document when the implementation mechanism or rationale changes without altering product behavior. Every implementation ExecPlan must cite exact product behavior revisions and respect this architecture.
