# GitHub Sources and Trust

Status: planned baseline for the skilload CLI MVP.

A **source** identifies one Skill location and one intended Git ref on GitHub. **Trust** is a local authorization to use one exact source after skilload verifies its repository identity and content preview. Trust is not Library membership and is never transferred by a workspace or Library export.

## SKL-SRC-001 - Accepted GitHub source input (Revision 1)

**Behavior.** The 0.1 MVP MUST accept GitHub HTTPS repository/tree/blob URLs, GitHub SSH repository URLs, and `owner/repo` shorthand. A tree URL identifies a candidate Skill directory. A blob URL MUST identify a file named `SKILL.md`, whose parent becomes the candidate Skill directory; other blob targets are invalid source shapes. It MUST support only `github.com`; GitHub Enterprise and all other hosts are outside scope. Every accepted form MUST normalize to one canonical representation.

**Acceptance.** Equivalent repository, tree, and direct-`SKILL.md` blob inputs resolve to the same canonical source. A different blob file or syntactically valid non-`github.com` URL fails with a structured source error before persistent state changes.

## SKL-SRC-002 - Canonical source identity (Revision 1)

**Behavior.** Canonical source identity MUST include normalized `owner/repo`, normalized Skill path, and an explicit namespace-preserving ref intent in the textual form `github:<lowercase-owner>/<lowercase-repo>#<encoded-path>@<encoded-ref>`. A branch MUST serialize as its full `refs/heads/<name>` ref, a tag as its full `refs/tags/<name>` ref, and a commit as its lowercase 40-hex SHA. Path separators and ref slashes remain `/`; within each path segment and the ref, only RFC 3986 unreserved ASCII bytes (`A-Z`, `a-z`, `0-9`, `-`, `.`, `_`, and `~`) remain literal. Every other UTF-8 byte, including `%`, `#`, `@`, and every non-ASCII byte, MUST use uppercase percent encoding `%HH`. Percent decoding occurs exactly once before validation, and canonical serialization MUST contain exactly one literal `#` and one literal `@`; a repository-root Skill has an empty encoded path. Git path spelling and branch/tag spelling MUST NOT receive Unicode normalization because canonically equivalent spellings can identify different Git entries. Two otherwise equal sources with different ref namespaces, names, or SHAs are distinct. Source identity MUST NOT use a machine path or Library database ID.

**Acceptance.** A repository with `refs/heads/release` and `refs/tags/release` produces distinct identities ending in `@refs/heads/release` and `@refs/tags/release`, even when the refs currently resolve to the same commit. Path `skills/foo@bar` at branch `main` serializes as `github:owner/repo#skills/foo%40bar@refs/heads/main`, while path `skills/foo` at branch `bar@main` serializes as `github:owner/repo#skills/foo@refs/heads/bar%40main`; parsing, export/import, database keys, and exact Trust keep all four tuples distinct.

## SKL-SRC-003 - Default ref normalization (Revision 1)

**Behavior.** When input omits a ref, skilload MUST query the repository's current default branch, resolve it, and persist the branch as `refs/heads/<default-name>`. It MUST NOT retain an implicit "default" sentinel or an unqualified name whose meaning can later change.

**Acceptance.** Adding a repository whose default branch is `trunk` writes `@refs/heads/trunk` to resulting source state. A later default-branch rename does not silently change that stored ref.

## SKL-SRC-004 - Mutable ref resolution (Revision 1)

**Behavior.** A ref intent MAY be a branch, tag, or full commit SHA. Branches and tags are mutable source intents whose current commit is resolved into a lock or global pin. A fully qualified `refs/heads/...` or `refs/tags/...` input MUST retain that namespace. A short mutable ref supplied by an option or embedded in a tree/blob URL MAY normalize only when exactly one matching branch or tag intent can be proved; if both namespaces match, skilload MUST return structured `ambiguous_ref` with both fully qualified candidates and require explicit selection rather than apply Git's precedence rules. An update operation MUST resolve only the stored namespace and compare and deliberately advance the resolved commit; ordinary sync and read operations MUST NOT advance it.

**Acceptance.** After a branch moves, sync continues using the previous pinned commit and update selects the new commit from `refs/heads/...`; a tag is treated by the same rule within `refs/tags/...` even if users expect tags to be stable. Given different commits at `refs/heads/release` and `refs/tags/release`, short `release` makes no persistent change and returns both candidates, while explicit fully qualified inputs resolve, update, export, and authorize independently.

## SKL-SRC-005 - Immutable SHA source (Revision 1)

**Behavior.** A source whose ref is a full commit SHA is already immutable. Update MUST leave its pin unchanged and return the successful outcome `already_immutable` rather than treating it as an error.

**Acceptance.** Repeated update of a SHA source performs no ref advancement, returns success with `already_immutable`, and leaves config, lock, database, cache, and links unchanged.

## SKL-SRC-006 - Repository candidate discovery (Revision 1)

**Behavior.** If input names a repository but not an exact Skill path, skilload MUST scan at most 100,000 Git tree entries and collect at most 256 valid `SKILL.md` candidates. Every visited tree, regular-file, symlink, gitlink, or unsupported entry counts toward the scan ceiling. Exactly one candidate MAY proceed automatically. Multiple candidates MUST require an explicit selection; JSON mode MUST return the candidate set and MUST NOT choose one. Exceeding either discovery ceiling MUST return structured `discovery_limit_exceeded` without a truncated candidate result or persistent change. Discovery limits have no override; supplying an exact Skill path bypasses repository-wide discovery but not selected-Skill validation.

**Acceptance.** A one-Skill repository resolves without an extra selector. A repository containing two valid candidates makes no persistent change until the caller selects one, and its JSON result includes both normalized paths. A scan that would visit entry 100,001 or collect candidate 257 fails with the measured ceiling and instructs the caller to provide an exact path; that exact path proceeds only if the selected Skill satisfies `SKL-SRC-011`.

## SKL-SRC-007 - Root and complete Skill directory (Revision 1)

**Behavior.** A valid `SKILL.md` at repository root defines the entire repository working tree except `.git` as the Skill directory. Its YAML frontmatter MUST contain a name of 1 through 64 ASCII bytes matching `^[a-z0-9]+(?:-[a-z0-9]+)*$` and a nonempty description of at most 1,024 Unicode scalar values. The name comparison is exact ASCII byte equality: skilload MUST NOT trim, case-fold, or Unicode-normalize the frontmatter value. For a non-root Skill, the comparison target is the selected directory's exact final Git path segment. For a root Skill, skilload MUST derive the logical segment only from the fresh GitHub REST metadata `name`, preserve that original value separately as repository display spelling, lowercase ASCII `A-Z`, replace each maximal run of `.`, `_`, and `-` with one `-`, and remove leading/trailing `-`; URL spelling and percent decoding MUST NOT participate. An empty result or one longer than 64 bytes is `invalid_root_skill_name`, and the frontmatter name MUST byte-equal the valid result. Before constructing a frontmatter model, skilload MUST stop if the frontmatter has no closing delimiter within 65,536 bytes, is not UTF-8, exceeds 512 scalar/mapping/sequence nodes, exceeds eight nested mapping/sequence containers, contains a scalar longer than 4,096 UTF-8 bytes, or contains an anchor, alias, explicit tag, duplicate mapping key, or non-string mapping key. For every Skill, skilload MUST materialize the complete directory rooted beside `SKILL.md`, including referenced scripts, assets, and supporting documentation, rather than extracting only `SKILL.md`.

**Acceptance.** Non-root names `review-skill` and `a1` compare only to identical directory bytes; uppercase, underscore, Unicode, or separator variants fail rather than normalize. Root repositories `Review_Skill`, `review.skill`, and `.REVIEW__skill.` all derive `review-skill`, preserve their different API display spelling, and require frontmatter `name: review-skill`; metadata names made only of separators or whose derived value exceeds 64 bytes fail before Trust. Boundary fixtures reject the 65,537th frontmatter byte, 513th node, ninth container level, 4,097th scalar byte, aliases/anchors/tags, duplicate/non-string keys, and a 1,025-scalar description without building an expanded model. A valid Skill with nested assets and scripts restores those files with their relative paths, while a root Skill includes ordinary repository files below the root but excludes `.git` metadata.

## SKL-SRC-008 - Regular file fidelity (Revision 1)

**Behavior.** Materialization MUST preserve regular-file bytes and the executable bit. Every Git path below the selected Skill MUST be valid UTF-8, relative, slash-separated, and free of empty, `.` or `..` segments; invalid UTF-8, path traversal, absolute destinations, device files, sockets, and other unsupported filesystem entries MUST fail before content allocation or promotion. In addition to exact raw-path uniqueness, skilload MUST compute a portable `skilload-path-key-v1` per path: for each segment apply Unicode 15.1 NFD, full default case folding from `CaseFolding.txt` statuses `C` and `F` but not `T`, then NFD again, encode UTF-8, and join with literal `/`. Two distinct paths with one key MUST fail as `portable_path_collision`. The original UTF-8 path bytes, not the key, remain the materialized path and `SKL-SRC-012` digest input.

Before cache promotion, skilload MUST also materialize names into an empty staging directory on the destination cache filesystem with exclusive no-follow operations and verify the enumerated name bytes and file identities after every insertion. A target filesystem that aliases two distinct source paths, changes a name's bytes through normalization, or cannot represent a name MUST return `filesystem_path_collision` and remove staging. This host-specific rejection MUST NOT rewrite a pre-existing portable lock or digest; a lock created on a permissive host can therefore fail explicitly, without substitution, on a more restrictive host.

**Acceptance.** The canonical integrity and restored cache retain exact file bytes and whether a file is executable. `A.txt`/`a.txt`, composed/decomposed `e-acute`, and Unicode full-fold equivalents collide under the pinned key on every host, while invalid UTF-8 and traversal fail before promotion. A case-sensitive fixture whose target filesystem accepts two raw names still rejects their portable collision; a filesystem-only alias fixture returns `filesystem_path_collision`, leaves a prior lock byte-identical, and promotes no partial object.

## SKL-SRC-009 - Symlink safety (Revision 1)

**Behavior.** A source Skill MAY contain only relative symlinks whose fully resolved target remains within the same Skill directory. skilload MUST preserve the relative link target as a link and MUST reject absolute, escaping, cyclic, or unresolved links.

**Acceptance.** A link from `scripts/current` to `v1/run.sh` within the Skill is preserved and hashed. A link to `../../secret`, `/etc/passwd`, a cycle, or a missing target rejects the candidate without persistent state.

## SKL-SRC-010 - Submodule and Git LFS rejection (Revision 1)

**Behavior.** skilload MUST reject Git submodule entries and unmaterialized Git LFS pointer files inside a Skill. It MUST NOT initialize submodules or invoke Git LFS to retrieve additional content as part of source resolution.

**Acceptance.** A candidate containing a gitlink or an LFS pointer produces a structured validation error naming the offending path. No lock or Trust record is created.

## SKL-SRC-011 - Resource limits (Revision 1)

**Behavior.** Source validation MUST default to at most 2,000 materialized entries and 52,428,800 total bytes (50 MiB) per selected Skill before cache promotion. The entry count includes regular files and symlinks but not implicit directories. Total bytes are the exact Git blob sizes of regular files plus preserved symlink-target text. The independent defaults remain in force unless the request explicitly supplies a finite unsigned ceiling through `--max-source-files <COUNT>` and/or `--max-source-bytes <BYTES>`; an omitted dimension keeps its default. JSON previews and results MUST expose the active values as `source_limits.max_files` and `source_limits.max_bytes`. An override MUST be at least the corresponding default, applies only to that request and every selected source in its preview plan, is bound into any preview/confirmation token, and MUST NOT persist in Trust, Library, workspace, lock, global, or configuration state. These selected-tree overrides MUST NOT raise or disable the independent fixed acquisition budgets in `SKL-SRC-016`; a source that exceeds either layer fails with that layer's typed error.

**Acceptance.** A 2,001-entry or 52,428,801-byte candidate fails without override and reports both measured values and both active ceilings. The same exact candidate may proceed only when every exceeded dimension has a sufficient explicit numeric override and any required confirmation is completed. A later refetch or update receives the Revision 1 defaults again unless that new request repeats the override.

## SKL-SRC-012 - Canonical integrity digest (Revision 1)

**Behavior.** The integrity value MUST be a canonical SHA-256 tree digest covering sorted normalized paths, entry type, regular-file bytes, executable bit, and allowed symlink target text. The version-1 encoding MUST represent every path, regular-content, and symlink-target byte length as exactly eight unsigned big-endian bytes; lengths count bytes, not Unicode scalar values. A resolved record MUST also carry numeric repository identity, commit, and verified frontmatter name. Host-dependent metadata such as timestamps, owners, and absolute paths MUST NOT affect integrity.

**Acceptance.** Two materializations of the same Git tree on supported hosts and independent implementations produce the same digest. Golden fixtures prove that each length is an eight-byte unsigned big-endian value and that changing bytes, executable state, path, or symlink target changes the digest.

## SKL-SRC-013 - Conditional reproducibility (Revision 1)

**Behavior.** A lock or global pin promises exact restoration only while its Git commit remains retrievable from GitHub or a verified local cache entry remains. skilload MUST report this condition honestly and MUST NOT claim that a ref and digest can reconstruct content after both sources disappear.

**Acceptance.** A cache miss restores and verifies the exact pinned commit when GitHub still serves it. If both remote commit and verified cache are unavailable, sync fails without substituting a newer commit or rewriting the pin.

## SKL-SRC-014 - Immutable repository binding (Revision 1)

**Behavior.** After online metadata validation, source, Trust, lock, and global pin records MUST bind the path-based repository name to GitHub's numeric repository ID. A path that later resolves to a different numeric ID MUST be treated as a different repository even if its spelling is identical.

**Acceptance.** Reuse of an old `owner/repo` path by a new repository fails identity verification against existing state. A matching repository ID may be considered for the explicit migration in `SKL-SRC-015`.

## SKL-SRC-015 - Explicit rename or transfer migration (Revision 1)

**Behavior.** A GitHub rename or ownership transfer MUST require an explicit migration that verifies fresh metadata for the proposed new name has the numeric repository ID already stored with the old source. The old path's current response MAY aid discovery or warn that the path was reused, but MUST NOT replace the stored identity. The database-wide `source migrate` operation updates Library, Trust, and global records atomically. Workspace source migration is a separate explicit operation. Only the `owner/repo` spelling may change; a path or ref change is a new source and new Trust decision.

**Acceptance.** A proposed name whose fresh ID matches the stored ID produces a preview and, after approval, updates every in-scope Library, Trust, and global record without changing path, ref, commit, or integrity, including when the old path now resolves to a different repository and is flagged as reused. The preview reports known workspace references as impacts, but `source migrate` leaves workspace config/lock files, local manifests, durable workspace ownership/index rows, workspace-owned link rows, and workspace transaction evidence byte-for-byte and row-for-row unchanged until a separate `workspace migrate-source`. A different new-name ID, path, or ref is rejected as migration and must be added as a new source.

## SKL-SRC-016 - Data-only retrieval and authentication (Revision 1)

**Behavior.** skilload MUST treat repositories as untrusted data. It MAY invoke system Git with fixed safe arguments, but MUST resolve Git and optional `gh`, including every supported script interpreter in their launch chain, only through the trusted external-executable rules in `SKL-WSP-022` and MUST NOT execute a candidate from an empty, relative, current-workspace, or enclosing-worktree PATH location. Before any Git command, it MUST discard every caller-provided environment variable whose name begins `GIT_`, discover the selected Git binary's exec path with that environment removed, require that path to resolve to a nonempty absolute directory outside every identified project/worktree and skilload source/cache root, record and revalidate its identity, and invoke Git with an explicit `--exec-path=<validated-path>`. It MAY then add only operation-specific application-owned `GIT_*` values. Before permitting an SSH Git transport, it MUST also resolve the fixed basename `ssh` and its interpreter chain through those rules, record and revalidate every canonical executable identity, and set an application-owned `GIT_SSH_COMMAND` that shell-quotes only that canonical launch plus fixed noninteractive options together with `GIT_SSH_VARIANT=ssh`; a caller or Git config MUST NOT choose a different local program. If no safe SSH launch exists, the SSH attempt is unavailable rather than falling back to a basename lookup.

The Revision 1 object-acquisition budget for one selected source, cumulative across transport fallback and targeted retry, MUST be 268,435,456 received pack bytes, 250,000 declared pack objects, and 120 seconds from the first remote request through indexed-pack completion. A multi-source invocation MUST additionally stop at 1,073,741,824 received bytes, 1,000,000 declared objects, or 600 seconds. These fixed ceilings have no override. The transport MUST expose the incoming pack to a supervised receiver that checks the cumulative byte count before forwarding each chunk, rejects a pack-header object count before object-table allocation, passes the same byte ceiling to the validated indexer, and terminates the complete process group on deadline or limit. A failed attempt MUST remove its private staging object database before trying another transport and MUST return `fetch_limit_exceeded` with dimension, measured value, limit, and source when the shared budget is exhausted. Plain unbounded `git fetch` followed by selected-tree validation is not conforming.

skilload MUST NOT execute repository hooks, scripts, filters, submodules, LFS helpers, remote helpers, or Skill content selected from an untrusted location. Public repository metadata MAY be queried without authentication. Private repository identity validation MUST use authenticated GitHub REST or GraphQL metadata via `GH_TOKEN`, `GITHUB_TOKEN`, or an authenticated `gh`; SSH Git access alone is insufficient. skilload MUST NOT prompt for or persist credentials, and commit signatures are not required.

**Acceptance.** A private SSH clone without API metadata credentials cannot establish first Trust and explains the missing requirement. Supplying a valid supported API credential allows identity validation without writing that credential to skilload state. A repository-controlled hook is never run, and a fake `git`, `gh`, `ssh`, or indirect interpreter reachable only through `PATH=.` or an absolute worktree directory is rejected before execution. Fixtures prove that an inherited `GIT_EXEC_PATH` containing a marker `git-remote-https`, inherited dynamic configuration/repository-selection variables, unsafe PATH `ssh`, inherited `GIT_SSH`/`GIT_SSH_COMMAND`, and `#!/usr/bin/env` interpreter markers never execute or redirect the operation, while Git receives only the revalidated exec/interpreter identities, application-owned settings, canonical safe SSH launch, and fixed variant/options. Pack fixtures abort before byte 268,435,457, object 250,001, or second 121, delete staging, and return measured `fetch_limit_exceeded`; a two-source batch also proves the cumulative invocation ceilings.

## SKL-TRUST-001 - Exact Trust binding (Revision 1)

**Behavior.** A Trust record MUST authorize exactly one canonical encoded source from `SKL-SRC-002` plus the verified numeric repository ID. Trust for one ref, path, or repository MUST NOT authorize another, even when delimiter characters in one tuple could resemble separators in another before canonical encoding.

**Acceptance.** Trusting `skills/review@refs/heads/main` does not authorize `skills/review@refs/tags/main`, `skills/review@refs/heads/v2`, `skills/test@refs/heads/main`, or a new repository occupying the same path spelling.

## SKL-TRUST-002 - Trust is separate from Library membership (Revision 1)

**Behavior.** Trust and Library membership MUST be independently stored and mutated. Removing a Library entry MUST NOT revoke Trust, and revoking Trust MUST NOT remove Library metadata. Adding a source to Library or workspace MAY establish Trust only through the approval flow.

**Acceptance.** After Library removal, `trust get` still reports the approved source. After Trust revocation, `library get` still returns its metadata while restricted operations fail as specified by `SKL-TRUST-007`.

## SKL-TRUST-003 - First-approval preview (Revision 1)

**Behavior.** Before first Trust is persisted, skilload MUST safely fetch and validate temporary content and present normalized source, numeric repository ID, resolved commit, verified name, description, file count, total bytes, and warnings. Every repository-controlled or otherwise untrusted field in a human preview MUST use the terminal-safe quoted encoding required by `SKL-CLI-009`; JSON MUST preserve valid logical string values through standard JSON escaping and every host filesystem path through `SKL-CLI-004`'s `PathValue`. Direct GitHub adds to Library or workspace use this same flow. Confirmation is a user-interface consent step, not cryptographic proof of a human.

**Acceptance.** Rejecting the preview leaves no Trust, Library/workspace mutation, or promoted cache entry. Approving an unchanged preview creates Trust and allows the requested mutation to continue atomically. A description containing ESC, OSC, BEL, carriage return, or bidirectional-format controls is displayed only as visible escaped text and cannot clear, rewrite, relabel, or reorder the approval screen.

## SKL-TRUST-004 - Noninteractive confirmation token (Revision 1)

**Behavior.** JSON mode MUST never prompt. A confirmation-required response MUST return a short-lived, single-use token bound to the action and complete preview plan: every canonical source, repository ID, commit, selected target/profile, active source-limit ceiling, configured/effective cache limit and any per-invocation override, warning/conflict, durable database revision, and applicable workspace digest. Relevant state drift, expiry, action or plan mismatch, or reuse MUST invalidate the token.

**Acceptance.** Replaying a consumed token, applying it after workspace/database/target drift, or reusing a one-source token for a larger batch fails with a structured stale-or-invalid-confirmation error. A fresh token for unchanged state completes only its exact bound action and plan.

## SKL-TRUST-005 - Trust is machine-local (Revision 1)

**Behavior.** A cloned `.skilload.yaml`, `.skilload.lock`, or imported Library export MUST NOT grant Trust. First Trust on a machine requires online identity validation or an existing locally verified record for the exact source and repository ID; a lockfile alone is not evidence.

**Acceptance.** On a new machine, workspace sync of an untrusted cloned source stops for approval even when the lock contains a repository ID and integrity. Library import creates no Trust rows.

## SKL-TRUST-006 - Trusted operation gate (Revision 1)

**Behavior.** New cache promotion, workspace or global deployment, update, pin, sync, and cache-miss restoration MUST require active Trust for every affected external source. Removal, uninstall, and cleanup of already managed state MUST remain possible without active Trust.

**Acceptance.** A batch containing one untrusted source makes no version or deployment changes. The user can still remove that source from workspace/global desired state and delete verified managed links.

## SKL-TRUST-007 - Revocation behavior (Revision 1)

**Behavior.** Revoking Trust MUST preserve current verified managed links and all unrelated state, but MUST block future update, pin, sync, cache-miss restoration, and recovery that needs external content for that source. Revocation MUST NOT execute or delete source content merely because approval changed.

**Acceptance.** Immediately after revoke, an existing linked Skill remains available. Its next restricted mutation returns a structured trust-required error, while uninstall/remove succeeds.

## SKL-TRUST-008 - Trust administration and migration (Revision 1)

**Behavior.** Users MUST be able to add, inspect, list, and revoke Trust explicitly. A same-repository rename/transfer migration covered by `SKL-SRC-015` MAY update the canonical repository spelling in an existing Trust record only after same-ID verification; all other identity changes require a new approval.

**Acceptance.** Trust list/get expose source, repository ID, state, and relevant warnings without network access. A successful same-ID migration preserves approval continuity, while a ref/path/ID change cannot mutate the existing Trust record.
