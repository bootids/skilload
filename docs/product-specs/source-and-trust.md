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

**Behavior.** A valid `SKILL.md` at repository root defines the entire repository working tree as the Skill directory except `.git`. Its YAML frontmatter MUST contain a name of 1 through 64 ASCII bytes matching `^[a-z0-9]+(?:-[a-z0-9]+)*$` and a nonempty description of at most 1,024 Unicode scalar values. The name comparison is exact ASCII byte equality: skilload MUST NOT trim, case-fold, or Unicode-normalize it. The name MUST equal the selected Skill directory's final Git path segment; for a repository-root Skill, the normalized repository name is that logical segment. Before constructing a frontmatter model, skilload MUST stop if the frontmatter has no closing delimiter within 65,536 bytes, is not UTF-8, exceeds 512 scalar/mapping/sequence nodes, exceeds eight nested mapping/sequence containers, contains a scalar longer than 4,096 UTF-8 bytes, or contains an anchor, alias, explicit tag, duplicate mapping key, or non-string mapping key. For every Skill, skilload MUST materialize the complete directory rooted beside `SKILL.md`, including referenced scripts, assets, and supporting documentation, rather than extracting only `SKILL.md`.

**Acceptance.** Names `review-skill` and `a1` validate and compare only to the identical bytes. Empty, over-64-byte, uppercase, underscore, leading/trailing-hyphen, consecutive-hyphen, normalized-lookalike, or source-directory-mismatched names fail before Trust or cache promotion; `Review_Skill` is invalid rather than normalized. Boundary fixtures reject the 65,537th frontmatter byte, 513th node, ninth container level, 4,097th scalar byte, aliases/anchors/tags, duplicate/non-string keys, and a 1,025-scalar description without building an expanded model. A valid Skill with nested assets and scripts restores those files with their relative paths, while a root Skill includes ordinary repository files below the root but excludes `.git` metadata.

## SKL-SRC-008 - Regular file fidelity (Revision 1)

**Behavior.** Materialization MUST preserve regular-file bytes and the executable bit. It MUST use normalized relative paths and MUST reject path traversal, absolute destination paths, device files, sockets, and other unsupported filesystem entry types.

**Acceptance.** The canonical integrity and restored cache retain exact file bytes and whether a file is executable. A malicious entry that would escape the Skill root fails before cache promotion.

## SKL-SRC-009 - Symlink safety (Revision 1)

**Behavior.** A source Skill MAY contain only relative symlinks whose fully resolved target remains within the same Skill directory. skilload MUST preserve the relative link target as a link and MUST reject absolute, escaping, cyclic, or unresolved links.

**Acceptance.** A link from `scripts/current` to `v1/run.sh` within the Skill is preserved and hashed. A link to `../../secret`, `/etc/passwd`, a cycle, or a missing target rejects the candidate without persistent state.

## SKL-SRC-010 - Submodule and Git LFS rejection (Revision 1)

**Behavior.** skilload MUST reject Git submodule entries and unmaterialized Git LFS pointer files inside a Skill. It MUST NOT initialize submodules or invoke Git LFS to retrieve additional content as part of source resolution.

**Acceptance.** A candidate containing a gitlink or an LFS pointer produces a structured validation error naming the offending path. No lock or Trust record is created.

## SKL-SRC-011 - Resource limits (Revision 1)

**Behavior.** Source validation MUST default to at most 2,000 materialized entries and 52,428,800 total bytes (50 MiB) per selected Skill before cache promotion. The entry count includes regular files and symlinks but not implicit directories. Total bytes are the exact Git blob sizes of regular files plus preserved symlink-target text. The independent defaults remain in force unless the request explicitly supplies a finite unsigned ceiling through `--max-source-files <COUNT>` and/or `--max-source-bytes <BYTES>`; an omitted dimension keeps its default. JSON previews and results MUST expose the active values as `source_limits.max_files` and `source_limits.max_bytes`. An override MUST be at least the corresponding default, applies only to that request and every selected source in its preview plan, is bound into any preview/confirmation token, and MUST NOT persist in Trust, Library, workspace, lock, global, or configuration state.

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

**Acceptance.** A proposed name whose fresh ID matches the stored ID produces a preview and, after approval, updates every in-scope record without changing path, ref, commit, or integrity, including when the old path now resolves to a different repository and is flagged as reused. A different new-name ID, path, or ref is rejected as migration and must be added as a new source.

## SKL-SRC-016 - Data-only retrieval and authentication (Revision 1)

**Behavior.** skilload MUST treat repositories as untrusted data. It MAY invoke system Git with fixed safe arguments, but MUST resolve Git and optional `gh` only through the trusted external-executable rules in `SKL-WSP-022` and MUST NOT execute a candidate from an empty, relative, current-workspace, or enclosing-worktree PATH location. Before permitting an SSH Git transport, it MUST also resolve the fixed basename `ssh` through those rules, record and revalidate the canonical executable identity, remove caller-provided `GIT_SSH`, `GIT_SSH_COMMAND`, and `GIT_SSH_VARIANT`, and set an application-owned `GIT_SSH_COMMAND` that shell-quotes only that canonical path plus fixed noninteractive options together with `GIT_SSH_VARIANT=ssh`; a caller or Git config MUST NOT choose a different local SSH program. If no safe SSH executable exists, the SSH attempt is unavailable rather than falling back to a basename lookup. It MUST NOT execute repository hooks, scripts, filters, submodules, or Skill content. Public repository metadata MAY be queried without authentication. Private repository identity validation MUST use authenticated GitHub REST or GraphQL metadata via `GH_TOKEN`, `GITHUB_TOKEN`, or an authenticated `gh`; SSH Git access alone is insufficient. skilload MUST NOT prompt for or persist credentials, and commit signatures are not required.

**Acceptance.** A private SSH clone without API metadata credentials cannot establish first Trust and explains the missing requirement. Supplying a valid supported API credential allows identity validation without writing that credential to skilload state. A repository-controlled hook is never run, and a fake `git`, `gh`, or `ssh` reachable only through `PATH=.` or an absolute worktree directory is rejected before execution. Fixtures prove that unsafe PATH `ssh` and inherited `GIT_SSH`/`GIT_SSH_COMMAND` markers never execute, while Git receives only the revalidated canonical safe SSH client and fixed variant/options for an allowed SSH attempt.

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
