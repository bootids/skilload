# GitHub Resolution and Integrity Design

Status: planned design for the 0.1 CLI MVP. It implements `SKL-SRC-*` and `SKL-TRUST-*` while treating repositories as hostile data.

## Behavior Traceability

* Parsing, repository/ref resolution, candidate discovery, tree validation, and canonical hashing implement `SKL-SRC-001` through `SKL-SRC-014` and `SKL-SRC-016`.
* Same-ID source migration implements `SKL-SRC-015` and `SKL-TRUST-008`.
* Approval previews, local Trust records, and bound confirmation tokens implement `SKL-TRUST-001` through `SKL-TRUST-007`.
* Cache promotion, resource limits, exact refetch, credentials, network classification, and hostile-input defenses support `SKL-CACHE-001`, `SKL-CACHE-003`, `SKL-CACHE-005`, `SKL-CACHE-009`, and `SKL-OPS-007` through `SKL-OPS-010`.

## Resolution Pipeline

Use one application pipeline for Library add, direct workspace add, Trust add, lock, update, pin, refresh, and cache-miss restoration. Each caller supplies an operation policy describing whether it may resolve a mutable ref, establish Trust, promote cache, or mutate desired state.

The pipeline is:

1. Parse supported input and reject non-`github.com` hosts or ambiguous URL shapes.
2. Query GitHub repository metadata to obtain canonical current `owner/repo`, numeric repository ID, optional node ID, visibility, and default branch.
3. Normalize an omitted ref to `refs/heads/<default-branch>`; otherwise resolve input into a structured `Branch`, `Tag`, or `Commit` intent without dropping its namespace.
4. Resolve only that intent to a full commit SHA without mutating state. Reject a short name that matches both a branch and tag with both fully qualified candidates.
5. Receive the exact commit's Git pack through the Revision 1 byte/object/deadline gate, then index accepted objects into a temporary bare staging repository.
6. Inspect the Git tree to find/validate candidates and extract one complete Skill directory without checking out repository-controlled content.
7. Parse required `SKILL.md` frontmatter, validate every entry, compute canonical integrity, measure limits, and build a preview.
8. Verify Trust or return a confirmation requirement. On accepted unchanged state, promote the immutable materialization and let the caller commit its domain mutation.

All outputs carry a `ResolutionEvidence` value with canonical source, repository ID, commit, verified name/description, integrity, counts, warnings, separate metadata/content credential modes, and staged cache key.

## Source Parsing and Canonicalization

Parse URLs structurally rather than with ad hoc replacement. Supported examples normalize as follows:

    openai/skills
    https://github.com/openai/skills
    git@github.com:openai/skills.git
    https://github.com/openai/skills/tree/main/skills/example

The first three omit Skill path and ref; metadata supplies `refs/heads/<default-branch>` and candidate discovery supplies a selected path. A tree URL contributes its candidate directory. A blob URL is accepted only when the resolved blob basename is exactly `SKILL.md`, and contributes its parent directory. Parse either form only after verifying the URL belongs to the repository and separating ref from path through GitHub resolution, because both can contain slashes. Enumerate valid leading URL components against `refs/heads/`, `refs/tags/`, and a full SHA; accept only one `(ref intent, path)` tuple. If a short name exists in both mutable namespaces, or slash-bearing refs produce more than one valid tuple, return `ambiguous_ref`/`ambiguous_source_url` with fully qualified candidates and require an explicit `--ref refs/heads/...`, `--ref refs/tags/...`, or canonical source rather than choosing GitHub/Git precedence.

The canonical textual form is:

    github:<lowercase-owner>/<lowercase-repo>#<encoded-path>@<encoded-ref>

GitHub owner/repository matching is case-insensitive; preserve current display spelling separately. Normalize path separators to `/`, remove `.` segments, reject `..`, absolute paths, NUL/control bytes, empty selected paths except repository root, and `.git`. Do not Unicode-normalize Git path spelling. Represent ref intent as `Branch(name)`, `Tag(name)`, or `Commit(sha)`. Serialize the first two with their complete `refs/heads/` or `refs/tags/` prefix and the last as lowercase 40-hex. After structural normalization, percent-encode each path segment and complete serialized ref from their UTF-8 bytes: leave only RFC 3986 unreserved ASCII bytes literal, retain `/` as the path/ref separator, and encode every other byte with uppercase `%HH`. This includes literal `%`, `#`, `@`, and all non-ASCII bytes, so the serialized form has exactly one literal `#` and one literal `@`. Decode input URL escapes once, validate, and re-encode; never treat an already encoded canonical value as raw text and decode it twice. Branch/tag text remains exact after validation and is always passed to Git/GitHub as data, never shell syntax.

Use a structured `SourceIdentity { owner, repository, path, ref_intent: RefIntent }` domain value as the database, Trust, workspace, and export key; parse or render the textual form only at serialization boundaries. Golden tests include path `skills/foo@bar` with branch `main`, path `skills/foo` with branch `bar@main`, and same-name `refs/heads/release`/`refs/tags/release`; they require distinct text, keys, Trust records, fetch refspecs, update targets, and import round trips even when two intents resolve to one commit.

## Repository Metadata and Credentials

Use GitHub REST `GET /repos/{owner}/{repo}` through an embedded HTTPS client. Prefer credentials in this order: `GH_TOKEN`, `GITHUB_TOKEN`, then `gh auth token` when `gh` is installed and authenticated for `github.com`. Never prompt. Never persist or log a token.

Try public metadata unauthenticated when no credential exists. A private repository must return authenticated metadata before first Trust; a successful SSH clone alone has no numeric repository ID evidence. Treat 403/404 carefully because GitHub may hide private-resource existence. Follow repository redirects only to retrieve current metadata, then require same-ID migration for existing state.

Git content transport is acquisition policy, not source identity, and is not persisted as source state. During first resolution, an explicit SSH or HTTPS repository input makes that transport the first attempt. If it fails for authentication or availability, the resolver may try the other GitHub transport noninteractively using only existing credential helpers or SSH keys. A later resolution with no original transport hint uses deterministic HTTPS-then-SSH attempts. Report attempted transport classes without credential details; never convert a transport failure into a different repository identity.

Record the API-returned numeric repository ID as `RepositoryId(u64)`. Store node ID only as nonauthoritative supplementary evidence. Details and source cautions are in [`../references/github-repository-identity-and-auth.md`](../references/github-repository-identity-and-auth.md).

## Safe Git Object Retrieval

Use system `git` because it is an explicit runtime dependency, but resolve it and its complete supported interpreter chain before any probe through the shared external-executable adapter from the Agent design. Skip empty/relative PATH entries, reject every candidate/symlink/interpreter target in the current project/worktree or skilload source/cache roots, require regular executable files, and retain all canonical identities for revalidation. Build every Git child environment from an explicit allowlist of required non-Git operating-system, resolver-built PATH, proxy/TLS, locale, HOME, and authentication-agent values; never copy a caller variable whose name begins `GIT_`. With that clean environment and a private empty current directory, invoke the selected launch as `git --exec-path`, require one nonempty absolute result, canonicalize and reject a result inside any identified project/worktree or skilload source/cache root, and record the directory identity. Every later Git invocation revalidates the executable/interpreter/exec-directory identities and passes `--exec-path=<canonical-directory>` before the subcommand. This prevents inherited `GIT_EXEC_PATH` and PATH from selecting a remote helper or other dashed Git subprogram. Resolve optional `gh` and, before SSH, fixed basename `ssh` plus their interpreter chains by the shared rule; do not attempt a transport when its safe launch is unavailable. Avoid a normal working-tree checkout. For each acquisition:

1. Create a new private temporary receive directory and empty bare repository; no object or ref from a failed receive is reusable.
2. Revalidate the selected Git/interpreter/exec-directory identities and invoke the explicit canonical launch with the fixed `--exec-path` as an argv array, never through a shell.
3. Construct the GitHub remote URL from validated owner/repository, not arbitrary transport input.
4. Starting from the clean allowlist, add only application-owned Git settings: `GIT_TERMINAL_PROMPT=0`, a dedicated empty hooks directory, restricted protocols, no optional locks outside the staging repo, and fixed config that prevents user/repository filters from materializing content. Caller `GIT_CONFIG_*`, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR`, `GIT_NAMESPACE`, and every other inherited `GIT_*` value are absent. For SSH, add an application-owned `GIT_SSH_COMMAND` containing the injectively POSIX-shell-quoted explicit SSH interpreter/script launch plus only fixed `-oBatchMode=yes`, and set `GIT_SSH_VARIANT=ssh`. Git documents that `GIT_SSH_COMMAND` takes precedence over `GIT_SSH` and `core.sshCommand`, so this owned value prevents caller/global configuration from selecting a different program. Revalidate every SSH launch identity immediately before spawning Git and pass only the resolver-built PATH.
5. Request only the exact stored fully qualified ref or commit and required objects, with no implicit tags. A `BoundedPackReceiver` owns one cumulative `AcquisitionBudget` per source and one per invocation. It reads the 12-byte `PACK` header, requires a supported version, rejects declared object 250,001 before allocating an object table, and forwards each later chunk only when doing so keeps the source at or below 268,435,456 bytes and the invocation at or below 1,073,741,824 bytes. A supervisor enforces 120 source seconds and 600 invocation seconds from remote request through index completion.
6. Feed the accepted stream to the validated Git `index-pack --stdin --fix-thin --max-input-size=268435456` path as defense in depth, with fsck enabled and output confined to the private object database. The receiver owns both process groups; on byte/object/deadline failure it closes pipes, terminates and waits for transport/indexer children, reports `fetch_limit_exceeded`, and recursively removes staging before any fallback attempt. The fallback consumes the same source/invocation counters. A plain porcelain fetch that can populate the object database before these checks is forbidden.
7. Only after index success, connectivity/fsck, exact wanted-object presence, and budget reconciliation may the adapter install a private staging ref and use fixed `git rev-parse`, `git ls-tree -rz`, and `git cat-file --batch` arguments to read object metadata and blob bytes.

No checkout means `.gitattributes` smudge/clean filters and Git LFS do not execute. Tree mode `160000` identifies a submodule and is rejected. Mode `120000` identifies a symlink whose blob bytes are its target. Regular modes provide the executable bit. Tree and blob size accounting occurs before materialization.

Git command failures are captured as redacted structured diagnostics. Never pass a token in a command-line URL. HTTPS Git may use existing credential helpers; SSH may use the user's existing key for content only after API identity is independently known. Both attempts set terminal prompting off and remain bounded. Resolver tests place fake `git` and `gh` files in `.`, an empty PATH component, an absolute worktree directory, and an outside symlink back into the worktree and prove that none is executed.

## Candidate Discovery and Frontmatter

Stream the exact commit tree and locate regular blobs named `SKILL.md`, counting every visited entry and stopping before the Revision 1 ceiling of 100,000 entries or 257th valid candidate. Ignore `.git` by construction. Never return a truncated candidate set: report `discovery_limit_exceeded` and require an exact Skill path instead. A root `SKILL.md` yields the repository root candidate. An explicit Skill path bypasses repository-wide discovery, must contain its own `SKILL.md`, and does not search siblings.

For every candidate, locate the closing frontmatter delimiter without reading beyond 65,536 bytes, require UTF-8, and feed a non-expanding event API rather than a deserializer that resolves aliases. Abort before constructing the model on node 513, container level nine, scalar byte 4,097, any anchor/alias/explicit tag, duplicate key, or non-string key. Require a `name` of 1 through 64 ASCII bytes matching `^[a-z0-9]+(?:-[a-z0-9]+)*$` and a nonempty description of at most 1,024 Unicode scalar values. A non-root name must equal the exact final Git segment. For a root Skill, derive the comparison segment only from fresh REST metadata `name`: lowercase ASCII, collapse each maximal `[._-]+` run to `-`, trim edge hyphens, and reject empty/over-64-byte output. Preserve the API name as display evidence and never derive from a URL or apply this normalization to frontmatter. Store the validated `SkillName` as lowercase ASCII and compare byte-for-byte. The verified name becomes the deployment directory name. Do not execute dynamic content, referenced scripts, or frontmatter extensions. Candidate preview includes validation warnings for Agent-specific optional fields but skilload does not translate them. GitHub's repository-name basis is recorded in [`../references/github-repository-identity-and-auth.md`](../references/github-repository-identity-and-auth.md); the cross-Agent name basis is in [`../references/claude-and-codex-skill-discovery.md`](../references/claude-and-codex-skill-discovery.md).

If repository-only input yields more than one valid candidate, return all candidates sorted by path. A human caller selects explicitly; JSON returns typed candidates and no confirmation token until one source identity is exact.

## Tree Validation

Walk only entries under the selected root. The Revision 1 defaults allow 2,000 materialized regular-file/symlink entries and 52,428,800 bytes, where byte accounting sums regular blob sizes and symlink-target blob sizes. Inspect Git metadata and stop before allocating all content when either ceiling is exceeded. `--max-source-files` and `--max-source-bytes` map to an optional application `SourceLimits { max_files, max_bytes }`; validate each supplied unsigned value against its default, keep the default for an omitted dimension, and scope the resulting ceilings to the request's selected source set. JSON previews/results serialize the active values as `source_limits.max_files` and `source_limits.max_bytes`. Bind the complete override and every affected source into the preview and confirmation token but persist neither it nor an unlimited marker. It never raises the fixed pack-receive budgets above. A later update, refetch, or cache-miss restoration therefore requires a new explicit tree-limit override when the exact selected content exceeds a default and can still fail independently at acquisition.

Accepted entries are directories, regular files, and safe symlinks with valid UTF-8 relative paths. Reject gitlinks, special modes, invalid UTF-8, empty/dot/dot-dot segments, duplicate raw paths, and any path that cannot be represented safely. Before reading all blobs, compute `skilload-path-key-v1` independently for each segment using pinned Unicode 15.1 NFD, full default case fold (`C` plus `F`, excluding `T`), then NFD and UTF-8; join segment keys with `/` and reject a distinct raw path with the same key as `portable_path_collision`. Keep the original normalized UTF-8 bytes as the hash/materialization path. The pinned data and rationale are in [`../references/unicode-15-1-tag-normalization.md`](../references/unicode-15-1-tag-normalization.md).

After logical validation, use the cache filesystem adapter to build the empty payload tree with exclusive no-follow insertion. After each path, enumerate the parent and compare exact returned name bytes and file identity; reject normalization, case aliasing, or another host-only collision as `filesystem_path_collision`, remove the entire stage, and leave an existing lock unchanged. For a symlink, parse its blob as a relative path, normalize it relative to the link parent, follow the in-tree link graph, and reject absolute, escaping, missing, or cyclic resolution. Preserve the original relative target text after validation.

Detect a Git LFS pointer by its standard header and pointer structure; reject it rather than hashing the pointer as content. Do not run `git lfs` or initialize submodules. The comparative `npx skills` behavior is documented in [`../references/npx-skills-installation-model.md`](../references/npx-skills-installation-model.md).

## Canonical Integrity

Define a versioned byte encoding, `skilload-tree-v1`, independent of filesystem enumeration and metadata. Hash with SHA-256:

    magic bytes "skilload-tree-v1\0"
    for each entry sorted by raw normalized relative path bytes:
      entry type byte: 0x01 regular or 0x02 symlink
      path byte length as one unsigned 64-bit big-endian integer, then path bytes
      for regular:
        executable byte 0x00 or 0x01
        content byte length as one unsigned 64-bit big-endian integer, then exact blob bytes
      for symlink:
        target byte length as one unsigned 64-bit big-endian integer, then original relative target bytes

Every length is exactly eight bytes and counts the following byte sequence, not Unicode scalar values or filesystem characters. Reject a value that cannot fit in `u64` before hashing, although the smaller source resource ceilings normally make that unreachable. Directly concatenate the records above with no padding, terminator, entry count, BOM, or platform newline. Directories are represented implicitly by child paths; empty directories are not Git tree content relevant to a Skill. The resolved record stores `sha256:<lowercase-hex>` plus repository ID, commit, selected path, verified name, and format version. Tests use byte-level cross-platform golden fixtures covering zero/one/multibyte lengths, ordering, executable bits, symlinks, empty files, Unicode path bytes, and collision rejection.

## Immutable Cache Promotion

The logical cache key is repository ID, full commit, and normalized Skill path. Its physical directory uses safe components such as decimal repository ID, hexadecimal commit, and a hash/encoding of the Skill path; it never appends the untrusted path directly. One object has this conceptual layout:

    objects/<repository-id>/<commit>/<path-key>/
      manifest.json
      payload/                    # exact Agent-visible Skill directory

The manifest records the original normalized path, key/digest format versions, integrity, verified name, entry metadata, and size. It is outside `payload/`, is excluded from the Skill integrity tree, and cannot collide with or become visible as source content. Deployment links point to `payload/`.

Stage the whole object under a random directory, fsync files/directories needed for durability, verify the complete payload digest again, write the immutable manifest, then atomically rename into the cache object's parent.

If the destination already exists, verify its manifest and tree. Reuse only an exact match. Quarantine a mismatch and perform the one allowed exact refetch from `SKL-CACHE-005`. Cache objects are made read-only to discourage accidental edits, but integrity verification rather than permissions is the security boundary.

## Trust Preview and Token

An `ApprovalPreview` contains operation, canonical source, repository ID/current display name, commit, verified Skill name/description, file/byte counts, both active file/byte ceilings, integrity, ref mutability, warnings, and requested limit overrides. Human mode passes every repository-controlled name, description, path, ref, and warning through the CLI design's terminal-safe quoted encoder before asking for consent; source bytes never provide raw terminal control. JSON serializes the original logical values with ordinary JSON escaping rather than substituting the human-display form.

JSON mode returns a signed-or-random opaque token backed by a short-lived local database record. Store only a cryptographic token hash plus a canonical digest of the complete preview plan (action, all sources/repository IDs/commits, selected targets, overrides, and warnings), semantic `state_revision`, workspace digest when applicable, expiry, and consumed flag. Token bookkeeping does not advance `state_revision`. The second call hashes the presented token, reconstructs and compares the complete plan, acquires final locks, and atomically marks it consumed with the requested operation; any product-state change in that commit advances the revision. It fails on any bound-field drift. This token prevents accidental stale or broadened approval; it is not authentication against the same-account attacker excluded by the threat model.

## Source Migration

Migration resolves the proposed name through fresh GitHub metadata and requires its numeric repository ID to equal the immutable ID stored with the old source. It may resolve the old path for redirect discovery and warnings, but a reused old path is not the identity comparator. Build one impact plan with mutable Library, Trust, and global records plus a read-only list of known workspace references. `source migrate` commits only the first three domains atomically, including global-domain target/ownership rows, and must not update workspace config/lock files, local manifests, `known_workspaces`, `workspace_targets`, workspace-domain `owned_links`, or workspace transaction evidence. `workspace migrate-source` separately stages deterministic config/lock rewrites and their matching workspace ownership changes under the deployment journal.

Neither command changes path, ref, commit, integrity, verified name, Trust state, or deployment target. A path/ref change goes through normal new-source approval.

## Testing

Default tests use local bare Git repositories and an HTTP fixture server that models GitHub responses, redirects, authentication failures, mutable refs, deleted commits, and rate/error conditions. Fixtures cover the exact discovery, candidate, entry, byte, frontmatter-event, name, description, receive-byte/object/deadline, and invocation-aggregate boundaries; explicit-path discovery bypass; one-shot per-dimension source overrides; root metadata spellings `Review_Skill`, `review.skill`, separator-only, and overlong derived names; invalid-UTF-8 and Unicode portable-path collisions; target-filesystem aliases; hostile paths/modes/YAML; filters/hooks that must never execute; submodules; LFS pointers; symlink graphs; candidate ambiguity; canonical delimiter collisions and percent round trips; repository path reuse; confirmation replay/drift; hostile terminal fields; and byte-exact golden integrity digests with eight-byte lengths. Every pack-limit failure terminates/waits for children and removes staging before fallback. Process fixtures place marker helpers and interpreters under inherited `GIT_EXEC_PATH`/PATH, inject dynamic config/repository/object selectors, and prove the clean runner, validated interpreter chain, fixed exec path, and bounded receiver reach none of them. SSH fixtures put marker executables/interpreters in relative/project PATH entries and caller `GIT_SSH`/`GIT_SSH_COMMAND`, then prove only the revalidated safe launch receives Git's host/command arguments through the application-owned command and variant. Migration fixtures prove `source migrate` reports but does not mutate workspace ownership/config/lock evidence and that the later journaled workspace operation sees the original exact source. Real GitHub smoke tests are explicit or scheduled and never required for the default suite.
