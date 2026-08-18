# GitHub Repository Identity and Metadata Authentication

Scope: GitHub.com REST/GraphQL, repository rename/transfer behavior, and Git SSH-command selection verified on 2026-08-18. This reference explains why skilload binds a path-based source to an API repository ID and must secure Git's transport child process separately from Git itself.

## Why It Matters

`owner/repo` is a mutable location. A repository can be renamed or transferred, and the old location can later be reused. Trusting path spelling alone could therefore authorize a different repository. skilload records the numeric repository `id`, revalidates a proposed new path against that stored ID, and allows path spelling to change only through explicit same-ID migration.

## Key Conclusions

* The REST `Get a repository` response includes both numeric `id` and GraphQL-compatible `node_id` fields as well as mutable `full_name` and `default_branch` fields.
* GitHub recommends persisting global node IDs to reference objects across API versions. skilload's product decision uses the REST numeric repository ID as its immutable binding and may retain `node_id` as supplementary diagnostics; migration always proves equality through a fresh API response.
* GitHub redirects repository web and Git operations after rename or transfer, but redirects can disappear if the old path is reused. A redirect is therefore discovery help, not identity proof.
* `Get a repository` may return `301 Moved Permanently`, and following it yields the current repository metadata. skilload compares fresh proposed-name metadata with stored identity before proposing a migration; an old-path response is discovery/warning evidence only because that path may have been reused.
* Public repository metadata can be requested without authentication. Private repository metadata requires an authenticated token with repository Metadata read access (or equivalent classic-token access).
* An unauthenticated or insufficiently authorized REST request can return `403` or deliberately indistinguishable `404` responses.
* Git uses `GIT_SSH` or `GIT_SSH_COMMAND` instead of the default `ssh`; `GIT_SSH_COMMAND` takes precedence over `GIT_SSH` and is interpreted by a shell.
* Git's `core.sshCommand` can also choose the SSH command, but the official config documentation says an environment `GIT_SSH_COMMAND` overrides it. `GIT_SSH_VARIANT` overrides command-variant detection and can select the fixed OpenSSH argument contract.

## Authentication Consequence

Git over SSH can prove that GitHub allowed a clone, but the Git transport does not return REST repository `id`. It follows that SSH credentials alone cannot establish skilload's first Trust for a private source. Private validation needs `GH_TOKEN`, `GITHUB_TOKEN`, or a token obtained from an already authenticated `gh` CLI, in addition to whichever Git credentials retrieve content. This is an implementation inference from the separate Git and API interfaces, not a statement that GitHub rejects SSH cloning.

skilload does not persist these credentials. API error handling must not interpret private-resource `404` as proof that the repository does not exist.

## Git SSH Child-Process Consequence

Resolving and invoking a trusted absolute `git` binary does not constrain the program Git later starts for an SSH remote. Caller environment, Git configuration, and PATH can independently select that child. On Git 2.50.1, a local fixture with a trusted Git path and `PATH=<worktree>:/usr/bin` executed `<worktree>/ssh` during `git ls-remote` before any remote connection, which confirms that safe Git discovery alone is insufficient.

For an SSH attempt, skilload therefore resolves fixed basename `ssh` through the same absolute-directory, final-target, and file-identity checks as Git; removes caller-provided `GIT_SSH`, `GIT_SSH_COMMAND`, and `GIT_SSH_VARIANT`; then supplies its own shell-quoted canonical path through `GIT_SSH_COMMAND` and fixes `GIT_SSH_VARIANT=ssh`. The owned environment value overrides `core.sshCommand`, avoids Git's variant probe, and contains only fixed noninteractive options. This is an implementation conclusion from Git's documented precedence plus the local fixture, not a GitHub authentication requirement. User SSH configuration outside the identified untrusted roots remains inside skilload's same-account trust boundary.

## Rename and Transfer Cautions

A transfer preserves repository content and many associated objects, and GitHub redirects old links and Git operations. A rename likewise redirects most repository traffic. Neither redirect should authorize path/ref changes inside a source. The migration rule is deliberately narrower: a new owner/repository name whose fresh ID equals the source's stored repository ID, the same Skill path, and the same ref. If the old path has been reused, its different current ID is reported but does not erase the stored binding.

## Sources

* [REST API: Get a repository](https://docs.github.com/en/rest/repos/repos#get-a-repository)
* [REST API authentication](https://docs.github.com/en/rest/authentication/authenticating-to-the-rest-api)
* [GraphQL guide: Using global node IDs](https://docs.github.com/en/graphql/guides/using-global-node-ids)
* [GitHub: Renaming a repository](https://docs.github.com/en/repositories/creating-and-managing-repositories/renaming-a-repository)
* [GitHub: Transferring a repository](https://docs.github.com/en/repositories/creating-and-managing-repositories/transferring-a-repository)
* [Git environment: `GIT_SSH`, `GIT_SSH_COMMAND`, and `GIT_SSH_VARIANT`](https://git-scm.com/docs/git#Documentation/git.txt-codeGITSSHcode)
* [Git config: `core.sshCommand` and `ssh.variant`](https://git-scm.com/docs/git-config#Documentation/git-config.txt-coresshCommand)

Last updated: 2026-08-18.
