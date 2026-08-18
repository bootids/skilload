# Executable Script and Interpreter Resolution

Scope: executable-script dispatch and `env` PATH lookup relevant to supported macOS and Linux hosts, verified on 2026-08-18.

## Why It Matters

Validating only the path named by an Agent, Git, SSH, helper, or manager preflight is insufficient when that file is a script. A shebang can select another local executable, and the common `#!/usr/bin/env node` form asks `env` to search PATH. If skilload passes a project-controlled PATH, a directly safe script can therefore start an unsafe interpreter before its own code runs.

## Key Conclusions

* Linux `execve` recognizes an executable text file beginning with `#!interpreter [optional-arg]` and invokes that interpreter with the script path. Apple XNU likewise handles executable scripts inside the kernel exec path; supported-host behavior must not be treated as a direct native-binary launch.
* POSIX `env` invokes the named utility and uses its effective PATH for utility lookup. A shebang such as `#!/usr/bin/env node` therefore introduces a second executable resolution step.
* Kernel shebang tokenization and recursive-script limits are platform details. skilload should not rely on them for security or accept arbitrary shell syntax. It can instead parse a deliberately small portable shebang grammar, resolve every interpreter itself, and launch the terminal interpreter plus script arguments as an explicit argv array.
* A resolver-built PATH must contain only already accepted absolute search directories. Reusing the caller PATH after validating the first executable reintroduces empty, relative, project, source, and cache lookup locations to `env` or a child runtime.

## Design Consequence

Revision 1 accepts native Mach-O/ELF executables and scripts whose first line ends by newline or file EOF within 4,096 bytes. A direct interpreter token matches `/[A-Za-z0-9._+/-]+` with an optional single ASCII-space argument matching `[-A-Za-z0-9._+/=:,]+`; the only PATH-search form is `/usr/bin/env` followed by one basename matching `[A-Za-z0-9][A-Za-z0-9._+-]*`. The interpreter and every recursively encountered script are resolved under the same canonical-path, containment, executable-file, and identity rules as the original candidate. Chains are limited to four scripts, reject repeated file identities, and reject relative direct interpreters, `env -S`, env options/assignments/paths, tabs or other spacing, additional operands, NUL, carriage return, and an unterminated over-limit first line.

Before spawn, skilload revalidates every recorded file identity and constructs an explicit argv chain with no shell. In the env form, `/usr/bin/env` is only the recognized syntax marker: skilload resolves the basename itself and launches that resolved interpreter directly. The child receives a PATH formed only from the accepted absolute directories used by the resolver. Unsupported or drifting chains fail before any candidate or interpreter executes. This conservative grammar covers the common Node launcher while avoiding platform-dependent shebang parsing as an authorization boundary.

## Cautions

This procedure does not protect against an interpreter or script changed concurrently by another process running as the same account after final revalidation; that actor is outside the local threat model. It also does not make arbitrary interpreter startup configuration safe. Each spawned program still receives the operation-specific allowlisted environment and private working directory required by its adapter.

## Sources

* [Linux `execve(2)` interpreter-script semantics](https://man7.org/linux/man-pages/man2/execve.2.html)
* [Apple open-source XNU exec implementation](https://github.com/apple-oss-distributions/xnu/tree/main/bsd/kern)
* [POSIX `env` utility and PATH behavior](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/env.html)

Last updated: 2026-08-18.
