# skilload Architecture

Status: 0.1 CLI MVP 架构处于部分实现状态。`PLAN-0002` 建立 Rust workspace 与配置垂直切片；`PLAN-0003` 增加可移植 Library 导入/导出、受限 SQLite Library 元数据与本地 Unicode 15.1.0 规范化；`PLAN-0004` 在不改变 schema 或 ownership 的前提下增加八个显式、离线的 Library 元数据 mutation。来源解析、缓存内容、部署、manager 资产及其他产品域仍为 planned。

Product behavior is authoritative in [`docs/product-specs/`](docs/product-specs/README.md). This file defines boundaries, dependency direction, state ownership, and invariants. Technical mechanisms and rationale live in [`docs/design-docs/`](docs/design-docs/).

## System Shape

目标 0.1 系统是一个本地 Rust 二进制：它解析并验证 GitHub 托管的 Skill 目录，存储 durable metadata 与 desired state，在可移除 cache 中维护不可变外部内容，并将受管链接收敛到原生 Claude Code/Codex Skill roots；它绝不包装或启动 Agent。当前二进制实现配置切片、可移植 Library 导入/导出与显式 alias/category/tag/note 变更；它不解析网络来源、不创建 Trust、不缓存、部署或执行外部内容。

The current Cargo workspace is:

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
      skilload-cli/
        src/

已实现模块包括 `skilload-core` 的 configuration 与 Library domain/application/port/adapter 文件、`error.rs`，以及 `skilload-cli` 的参数、JSON、人类渲染和进程入口文件。P2/P3 Library adapter 只拥有 `data/skilload.db` 中的可移植来源元数据、alias/category/note、tags 与 semantic revision；它不拥有 Skill bytes、Trust、workspace、global 或 manager state。未实现的 ownership modules 与 manager assets 必须只在具有真实应用行为时加入。

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
* Adapters 在当前切片中实现 XDG/config 文件、受限可移植文件传输与 bundled SQLite Library repository；SQLite 的 FTS5 编译能力已被固定，但本切片不创建 FTS 表也不暴露 search。`skilload-core` 默认拒绝 unsafe code；唯一局部审计例外是在 first-import staging 与既有 database connection 的 SQLite main-file `HAS_MOVED` 检查中调用 bundled SQLite FFI，必须在任何 SQL 前返回 identity drift，不能成为一般 native I/O abstraction。immutable cache、system Git、GitHub metadata HTTP、time/randomness 和 Claude/Codex adapters 仍为 planned。
* The CLI parses arguments, invokes one application command/query, and renders human or JSON output. It does not issue SQL, edit workspace files, run Git, or manage links directly.
* Future TUI, Web, or other interfaces must call the same application layer. They may not bypass Trust, ownership, transaction, or network policies.

当前已实现 port 与 composition 见 [application and persistence design](docs/design-docs/application-and-persistence.md)。

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

**Source and Trust** normalize GitHub coordinates including fully qualified branch/tag intent and the exact repository-root name rule, obtain repository identity, receive inert Git objects through fixed byte/object/deadline budgets, validate bounded frontmatter plus the complete Skill tree and portable/target-filesystem path keys, compute integrity, and authorize one exact source. Git transport may use only a resolved Git launch including every interpreter, a separately validated fixed exec path, an allowlisted process environment, a supervised pre-index pack receiver, and, for SSH, an independently resolved and application-forced SSH launch. They do not decide Library metadata or deployment targets. See `SKL-SRC-*` and `SKL-TRUST-*`.

**Library** owns bounded searchable user metadata keyed by source and resource-bounded portable import/export. It does not own Skill content, Trust, pins, or deployment state. See `SKL-LIB-*`.

**Workspace** owns resource-bounded deterministic portable config/lock files and complete-set project deployment intent. Agent selection is invocation-local. A deployed workspace target is owned by canonical workspace, Agent, and canonical project Skill root; validated absolute HOME/configuration observations are not a second ownership identity for the same path. A random local workspace instance ID connects an untracked, git-excluded manifest to the durable index so an explicit, fully proved rebind can transfer that canonical-path component after a move without placing machine identity in portable files or adopting a lookalike target. Git-backed manifest checks bind to sanitized, independently resolved worktree/Git-directory/index resources. Once Git tracks that manifest, no mutation or recovery may rewrite it until the user removes it from the real per-worktree index. See `SKL-WSP-*`.

**Global deployment** owns local per-profile desired targets with one shared pin per source. It consumes Library and Trust but does not mutate either. See `SKL-GLB-*`.

**Manager** owns embedded Agent-specific assets and their copied global installations. It is independent of external sources, Library, Trust, and cache. See `SKL-MGR-*`.

**Cache and recovery** own immutable external materializations, stable allocated-byte quota accounting, serialized one-object integrity quarantine/headroom, mutation journals, process locks, and verifiable cleanup/repair. Base-database recovery follows the normative evidence/backup/restore-or-explicit-reset procedure and never silently adopts surviving files as ownership. They cannot rewrite a product source of truth merely to make observed state look healthy. See `SKL-CACHE-*`, `SKL-OPS-*`, and [`docs/product-specs/database-recovery.md`](docs/product-specs/database-recovery.md).

**Agent adapters** validate every environment path before resolving roots, pre-validate repository-controlled settings and conflict traversal under `agent-project-input-v1`, inspect disable/conflict state, and describe link/copy actions. A shared infrastructure resolver discovers every external executable and complete supported script-interpreter chain only from configured absolute Agent paths or resolver-accepted absolute PATH directories and rejects project/source/cache-contained members before any probe. Exact-owned removal plans may skip Agent launch/settings probes, but never root validation or ownership inspection. Agent adapters never parse business CLI arguments or make policy decisions about confirmation.

## Cross-Cutting Invariants

1. External repository content, cloned workspace documents, repository-controlled Agent settings/conflict trees, and Library import JSON from an untrusted project are always untrusted inert data. Pack receive, frontmatter, workspace YAML, Agent inputs/traversal, and import JSON are resource-bounded before unrestricted staging or a full model exists; source-controlled code, hooks, filters, submodules, LFS downloads, executable or interpreter candidates reached through relative/project-contained PATH locations, and caller-selected Git helpers or SSH commands are never executed (`SKL-SRC-007` through `SKL-SRC-016`, `SKL-LIB-008`, `SKL-LIB-010`, `SKL-WSP-004`, `SKL-WSP-005`, `SKL-WSP-022`, `SKL-OPS-010`).
2. Trust is exact, local, explicit, namespace-preserving for Git refs, and separate from Library/workspace/export state. Active Trust gates every external-content acquisition or valid use, but not an explicitly requested plan proved wholly subtractive against exact ownership evidence (`SKL-SRC-002` through `SKL-SRC-004`, `SKL-TRUST-001` through `SKL-TRUST-008`).
3. A successful pin names one repository ID, commit, Skill path, verified name, and canonical integrity. Sync never substitutes another commit (`SKL-SRC-012` through `SKL-SRC-015`).
4. External cache entries are managed as immutable: skilload never edits a promoted object in place and never promotes, links, or reuses an object after detecting failed integrity. Stable quota counts allocated promoted, quarantine, and staging bytes; one serialized repair may use only the bounded one-object transient headroom and must clean full quarantines on every completed/recovered outcome. Native Agents read deployed symlinks without passing through skilload, so a post-deployment same-account modification or disk fault can be consumed until the next skilload integrity observation; that observation reports or quarantines the object according to command mutability (`SKL-CACHE-001`, `SKL-CACHE-003`, `SKL-CACHE-005`).
5. skilload creates, replaces, removes, or rebinds only paths it can prove it owns. An exact foreign target is never overridden, including with force; a moved workspace requires the matching local instance, durable transaction/link evidence, absent old path, and explicit complete-Agent rebind. A Git-tracked local deployment manifest is preserved and blocks mutation rather than receiving further machine state (`SKL-WSP-019`, `SKL-WSP-023`, `SKL-WSP-025`, `SKL-GLB-010`).
6. Product mutations are planned and committed through application services. Normal success means every participating resource committed; crash recovery converges journaled work to a coherent old or new state. Corrupt base-database rows block writes until the operator completes `database-corruption-v1`; restore/reset never silently combines database generations or adopts surviving derived state (`SKL-CACHE-008`, `SKL-CACHE-009`, `SKL-OPS-004`).
7. Read-only commands do not create state or perform network access (`SKL-OPS-005`, `SKL-OPS-008`, `SKL-CLI-012`).
8. Schema and source migrations are explicit. Reads and sync never rewrite an unknown/older format. Database-wide `source migrate` owns only Library, Trust, and global records; workspace config/lock and ownership state change only through the separately journaled `workspace migrate-source` (`SKL-SRC-015`, `SKL-WSP-014`, `SKL-OPS-003`, `SKL-OPS-006`).
9. Human and JSON interfaces represent the same application outcome. JSON stdout is one versioned envelope whose authoritative catalog fixes the required result type/outcomes for all 50 non-meta operations, one complete confirmation preview, and every error-code detail variant; it never prompts. Full-range unsigned values use lossless decimal strings, batch network evidence remains attributable per source/attempt, and relocation status exposes the complete old/current/Agent evidence needed by the explicit command. Human rendering quotes and escapes every untrusted field before it can reach a terminal, valid logical strings use standards-compliant JSON escaping, and every filesystem path uses one display-plus-base64 `PathValue` so native bytes are lossless (`SKL-CLI-004` through `SKL-CLI-009`, [`docs/product-specs/api-v1.md`](docs/product-specs/api-v1.md)).
10. No external executable or indirect interpreter is discovered or probed through an empty, relative, current-project, source-staging, or external-cache search location; every supported launch-chain identity is checked before probe and revalidated before use. Every Git child drops inherited `GIT_*` state, fixes a separately validated exec path, and adds back only application-owned settings; repository/index inspection also binds explicit resolved resources and remote acquisition enters an object database only through the supervised fixed-budget receiver. Git SSH transport forces the independently resolved SSH launch. Agent roots never derive from relative environment values (`SKL-SRC-016`, `SKL-WSP-022`, `SKL-WSP-025`, `SKL-MGR-006`).
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

当前二进制不执行外部程序且不进行网络访问；它以 bundled SQLite（含 FTS5 编译能力）保存 P2 的可移植 Library 元数据，但不注册 FTS/search，也不链接 HTTP client。未来 source/deployment domains 需要 system `git` 用于 source object retrieval、仅在 SSH Git transport 时使用安全的 system `ssh`，以及 selected Agent CLI 用于 additive/repair/functional Agent 操作。exact-owned removal-only plan 不需要安装 Agent executable。`gh` 仍是可选 authenticated metadata-token source。未来 discovery（包括可接受的 direct 或 `/usr/bin/env` script interpreter）使用 `SKL-WSP-022` 的 shared trusted resolver；只有 `agents.claude.executable` 与 `agents.codex.executable` 可以覆盖 basename，且值必须绝对路径。

Network access is limited to the operations named by `SKL-OPS-008` and only GitHub.com is a content source. Agent directories and their version-sensitive behavior are isolated behind adapters and recorded in [the Agent discovery reference](docs/references/claude-and-codex-skill-discovery.md).

## Documentation Change Rules

Change product specifications first when user-visible behavior or acceptance changes. Change this file when module ownership, dependency direction, state authority, or a cross-cutting invariant changes. Change a design document when the implementation mechanism or rationale changes without altering product behavior. Every implementation ExecPlan must cite exact product behavior revisions and respect this architecture.
