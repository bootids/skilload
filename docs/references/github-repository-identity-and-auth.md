# GitHub Repository Identity and Metadata Authentication

Scope: GitHub.com REST/GraphQL, repository naming and rename/transfer behavior, and Git helper/SSH/repository-selection/acquisition behavior verified on 2026-08-18. This reference explains why skilload binds a path-based source to an API repository ID, derives root-Skill naming from fresh metadata, and must secure and bound Git's child-process and object-receive boundaries separately from Git itself.

## Why It Matters

`owner/repo` is a mutable location. A repository can be renamed or transferred, and the old location can later be reused. Trusting path spelling alone could therefore authorize a different repository. skilload records the numeric repository `id`, revalidates a proposed new path against that stored ID, and allows path spelling to change only through explicit same-ID migration.

## Key Conclusions

* The REST `Get a repository` response includes both numeric `id` and GraphQL-compatible `node_id` fields as well as mutable `full_name` and `default_branch` fields.
* GitHub recommends persisting global node IDs to reference objects across API versions. skilload's product decision uses the REST numeric repository ID as its immutable binding and may retain `node_id` as supplementary diagnostics; migration always proves equality through a fresh API response.
* GitHub redirects repository web and Git operations after rename or transfer, but redirects can disappear if the old path is reused. A redirect is therefore discovery help, not identity proof.
* `Get a repository` may return `301 Moved Permanently`, and following it yields the current repository metadata. skilload compares fresh proposed-name metadata with stored identity before proposing a migration; an old-path response is discovery/warning evidence only because that path may have been reused.
* GitHub repository names are at most 100 characters and contain only ASCII letters, digits, `.`, `-`, and `_`. The REST metadata name is therefore the authoritative unescaped display spelling for repository-root Skill-name derivation; URL percent-decoding or caller spelling is not a second naming input.
* GitHub portable owner identity 使用 login 的 canonical lowercase 形式：1–39 个 ASCII bytes，只能由字母/数字或单个 hyphen 组成，且不得 leading/trailing 或 consecutive hyphen。GitHub Enterprise Cloud documentation 独立确认 39-character maximum；下列 pinned validation expression 记录 P2 使用的完整 login grammar，使不可能的 owner 不会进入 durable source evidence。
* Public repository metadata can be requested without authentication. Private repository metadata requires an authenticated token with repository Metadata read access (or equivalent classic-token access).
* An unauthenticated or insufficiently authorized REST request can return `403` or deliberately indistinguishable `404` responses.
* Git uses `GIT_SSH` or `GIT_SSH_COMMAND` instead of the default `ssh`; `GIT_SSH_COMMAND` takes precedence over `GIT_SSH` and is interpreted by a shell.
* Git's `core.sshCommand` can also choose the SSH command, but the official config documentation says an environment `GIT_SSH_COMMAND` overrides it. `GIT_SSH_VARIANT` overrides command-variant detection and can select the fixed OpenSSH argument contract.
* Git's `GIT_EXEC_PATH` environment variable and global `--exec-path=<path>` option select the directory from which Git runs core subprograms such as protocol remote helpers. PATH remains a fallback for non-core `git-<command>` programs.
* `GIT_CONFIG_COUNT` plus numbered key/value environment variables inject runtime configuration. `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, and related variables redirect configuration sources.
* `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR`, object-directory variables, and namespace/discovery variables can redirect the repository resources used by core commands. Git documents that the index normally comes from the per-worktree Git directory and that linked worktrees have their own Git directory/index.
* `git index-pack --max-input-size=<bytes>` aborts when the received pack exceeds an explicit byte ceiling. The `git fetch` porcelain documents depth and object filters but no corresponding total received-pack or object-count ceiling, so selected-tree validation after an ordinary fetch is not a pre-receive resource bound.

## Authentication Consequence

Git over SSH can prove that GitHub allowed a clone, but the Git transport does not return REST repository `id`. It follows that SSH credentials alone cannot establish skilload's first Trust for a private source. Private validation needs `GH_TOKEN`, `GITHUB_TOKEN`, or a token obtained from an already authenticated `gh` CLI, in addition to whichever Git credentials retrieve content. This is an implementation inference from the separate Git and API interfaces, not a statement that GitHub rejects SSH cloning.

skilload does not persist these credentials. API error handling must not interpret private-resource `404` as proof that the repository does not exist.

## Git SSH Child-Process Consequence

Resolving and invoking a trusted absolute `git` binary does not constrain the program Git later starts for an SSH remote. Caller environment, Git configuration, and PATH can independently select that child. On Git 2.50.1, a local fixture with a trusted Git path and `PATH=<worktree>:/usr/bin` executed `<worktree>/ssh` during `git ls-remote` before any remote connection, which confirms that safe Git discovery alone is insufficient.

For an SSH attempt, skilload therefore resolves fixed basename `ssh` through the same absolute-directory, final-target, and file-identity checks as Git; removes caller-provided `GIT_SSH`, `GIT_SSH_COMMAND`, and `GIT_SSH_VARIANT`; then supplies its own shell-quoted canonical path through `GIT_SSH_COMMAND` and fixes `GIT_SSH_VARIANT=ssh`. The owned environment value overrides `core.sshCommand`, avoids Git's variant probe, and contains only fixed noninteractive options. This is an implementation conclusion from Git's documented precedence plus the local fixture, not a GitHub authentication requirement. User SSH configuration outside the identified untrusted roots remains inside skilload's same-account trust boundary.

## Git Helper and Repository Environment Consequence

Invoking a trusted absolute Git binary is also insufficient for HTTPS or local repository inspection when caller Git environment variables survive. On Git 2.50.1, a local fixture set `GIT_EXEC_PATH` to a temporary directory containing a marker `git-remote-https`; `/usr/bin/git ls-remote https://example.invalid/repo` executed that marker. Passing `/usr/bin/git --exec-path=<validated-safe-directory>` selected the safe fixture instead even while the hostile inherited value remained. The implementation conclusion is stronger and easier to audit: construct an allowlisted child environment with no inherited `GIT_*` name, discover and validate the selected binary's default exec directory in that clean environment, fix it through the command-line option, and add back only application-owned Git variables.

Repository inspection needs the same runner. On Git 2.50.1, a tracked `.skilload/state/deployments.json` disappeared from `git ls-files --error-unmatch` when `GIT_INDEX_FILE` selected a valid empty alternate index. A clean `rev-parse` resolved the canonical worktree, per-worktree Git directory, and effective index; invoking `ls-files` with explicit `--git-dir`, `--work-tree`, and an application-owned `GIT_INDEX_FILE` bound to that recorded index found the tracked file. This supports clearing all inherited repository-selection/configuration variables, recording the effective resources, and revalidating them before treating a manifest as untracked. It does not claim protection against the explicitly excluded same-account concurrent mutation after final revalidation.

## Bounded Pack-Receive Consequence

An exact refspec, shallow depth, or blob filter controls which history or object classes the server should send; none is a total transport budget. Likewise, the selected Skill's post-fetch 50 MiB tree ceiling cannot prevent a commit with a much larger reachable object graph from filling staging first. Git's documented `index-pack --max-input-size` proves that the pack byte stream can be rejected during indexing, but ordinary `git fetch` does not expose that option or a total object-count limit.

Revision 1 therefore needs a supervised pack-receive boundary rather than an unbounded porcelain fetch followed by validation. The receiver must count bytes before forwarding each chunk, inspect the pack header's declared object count before allowing object allocation, impose its own wall-clock deadline, and pass the same byte maximum to the validated indexer as defense in depth. It must terminate the transport/indexer process group and delete the private staging object database on any limit or timeout. This is an implementation conclusion from the documented interfaces; it does not claim that GitHub itself enforces skilload's client budget.

## Repository-Root Name Consequence

A repository-root Skill has no final Git directory segment. The only stable repository spelling available after redirect and identity verification is the fresh REST metadata `name`. Because GitHub restricts that value to ASCII letters, digits, `.`, `-`, and `_`, skilload can define one byte algorithm without locale or Unicode behavior: lowercase ASCII letters, collapse each maximal run of `.`, `_`, and `-` to one hyphen, and strip leading/trailing hyphens. The original metadata spelling remains display evidence. An empty or over-64-byte result is not a valid root Skill name.

## Rename and Transfer Cautions

A transfer preserves repository content and many associated objects, and GitHub redirects old links and Git operations. A rename likewise redirects most repository traffic. Neither redirect should authorize path/ref changes inside a source. The migration rule is deliberately narrower: a new owner/repository name whose fresh ID equals the source's stored repository ID, the same Skill path, and the same ref. If the old path has been reused, its different current ID is reported but does not erase the stored binding.

## Sources

* [REST API: Get a repository](https://docs.github.com/en/rest/repos/repos#get-a-repository)
* [REST API authentication](https://docs.github.com/en/rest/authentication/authenticating-to-the-rest-api)
* [GraphQL guide: Using global node IDs](https://docs.github.com/en/graphql/guides/using-global-node-ids)
* [GitHub: Renaming a repository](https://docs.github.com/en/repositories/creating-and-managing-repositories/renaming-a-repository)
* [GitHub: Transferring a repository](https://docs.github.com/en/repositories/creating-and-managing-repositories/transferring-a-repository)
* [Git environment: `GIT_SSH`, `GIT_SSH_COMMAND`, and `GIT_SSH_VARIANT`](https://git-scm.com/docs/git#Documentation/git.txt-codeGITSSHcode)
* [Git invocation and environment: `--exec-path`, `GIT_EXEC_PATH`, repository paths, and index selection](https://git-scm.com/docs/git)
* [Git configuration environment: dynamic key/value and config-source overrides](https://git-scm.com/docs/git-config#Documentation/git-config.txt-ENVIRONMENT)
* [Git worktree details: per-worktree Git directories and indexes](https://git-scm.com/docs/git-worktree#_details)
* [Git config: `core.sshCommand` and `ssh.variant`](https://git-scm.com/docs/git-config#Documentation/git-config.txt-coresshCommand)
* [GitHub repository-name constraints](https://docs.github.com/en/repositories/creating-and-managing-repositories/creating-a-new-repository)
* [GitHub Enterprise Cloud username considerations](https://docs.github.com/enterprise-cloud@latest/admin/managing-iam/iam-configuration-reference/username-considerations-for-external-authentication)
* [GitHub login validation expression](https://raw.githubusercontent.com/shinnn/github-username-regex/master/index.js)
* [Git fetch options and object filtering](https://git-scm.com/docs/git-fetch)
* [Git index-pack input-size limit](https://git-scm.com/docs/git-index-pack)
* [Git 2.50.1 fetch-pack receive/index flow](https://github.com/git/git/blob/v2.50.1/fetch-pack.c)

Last updated: 2026-08-20.
