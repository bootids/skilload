# CLI Contract

Status: planned baseline for the skilload CLI MVP.

The CLI is both the human interface and the stable machine interface used by the built-in manager Skill. Product-domain semantics remain authoritative in their domain specifications; this file fixes routing, output, and compatibility behavior.

## SKL-CLI-001 - Canonical command surface (Revision 1)

**Behavior.** The 0.1 command tree MUST be exactly:

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

`source migrate` covers Library, Trust, and global database source spelling; workspace source migration remains separate.

**Acceptance.** Generated help and parser tests enumerate every path above and no additional product command. Every accepted path dispatches to a real application operation.

## SKL-CLI-002 - No-argument help (Revision 1)

**Behavior.** Invoking `skilload` without arguments MUST print top-level help and exit successfully. It MUST NOT start an interactive TUI, server, setup wizard, or implicit mutation.

**Acceptance.** In an empty isolated HOME, `skilload` exits zero, prints help to stdout, and creates no file or network request.

## SKL-CLI-003 - No aliases or placeholders (Revision 1)

**Behavior.** The MVP MUST NOT provide shorthand command aliases, legacy wrapper commands, hidden UI/server commands, or stub subcommands. Unknown or removed command names MUST produce a usage error rather than silently routing to another action.

**Acceptance.** Representative `add`, `rm`, `use`, `init`, `claude`, `codex`, `tui`, `web`, and `collection` invocations fail as unknown commands and make no state change.

## SKL-CLI-004 - Single versioned JSON envelope (Revision 1)

**Behavior.** Every operation with machine output MUST support the documented JSON mode and write exactly one JSON value to stdout. The envelope MUST contain `api_version: 1`, an operation identifier, a success/failure indicator, and one typed result or error payload. Progress and diagnostics MUST never contaminate JSON stdout.

**Acceptance.** Parsing stdout requires one JSON decode with no prefix/suffix lines. Success, idempotent success, confirmation-required, and error fixtures all carry `api_version: 1` and the invoked operation.

## SKL-CLI-005 - Structured errors and exit status (Revision 1)

**Behavior.** Successful and idempotent outcomes MUST exit zero. Usage and operational failures MUST exit nonzero and include a stable machine error code, human-readable message, and relevant structured details. Exact numeric nonzero codes MAY be grouped by the CLI design but MUST be documented and tested.

**Acceptance.** A caller can distinguish invalid arguments, `not_found`, Trust requirement, confirmation requirement, conflict, `busy`, network/authentication, integrity, and recovery failures without parsing prose.

## SKL-CLI-006 - JSON never prompts (Revision 1)

**Behavior.** JSON mode MUST never read an interactive confirmation. A confirmation-required operation MUST return its preview plus the bound token from `SKL-TRUST-004`; the caller completes the action in a separate explicit invocation carrying that token.

**Acceptance.** Running JSON mode with closed stdin never hangs. The first call returns a typed confirmation requirement, and a valid second call against unchanged state completes.

## SKL-CLI-007 - Idempotent success outcomes (Revision 1)

**Behavior.** A mutation whose desired state is already satisfied MUST exit successfully with `unchanged`, `already_exists`, or the more specific `already_immutable`. It MUST NOT rewrite files or metadata merely to report success.

**Acceptance.** Repeated add/install/sync/set operations return the documented idempotent outcome, preserve byte-identical state, and exit zero.

## SKL-CLI-008 - Missing mutation target (Revision 1)

**Behavior.** A mutation that requires an existing target and cannot find it MUST return nonzero `not_found`. This includes remove, revoke, uninstall, metadata mutation, update/pin selectors, and migration selectors where absence is not already-satisfied state.

**Acceptance.** Human and JSON output agree on `not_found`, include the selector/domain, and make no state change.

## SKL-CLI-009 - Human output language and streams (Revision 1)

**Behavior.** Human-facing 0.1 output MUST be English. Primary results and help go to stdout; warnings, progress, and diagnostics go to stderr. Noninteractive terminals MUST receive usable output without relying on color or cursor control. Every repository-controlled, filesystem-derived, environment-derived, or user-supplied field MUST be rendered with one injective terminal-safe quoted encoding: surround the field with ASCII double quotes; encode a double quote as backslash-double-quote and a backslash as two backslashes; use the visible two-byte ASCII escapes `\n`, `\r`, and `\t`; encode every other C0 control, DEL, C1 control, U+2028, U+2029, and bidirectional-format code point U+061C, U+200E-U+200F, U+202A-U+202E, or U+2066-U+2069 as `\u{XXXX}` with uppercase hexadecimal zero-padded to at least four and at most six digits; and encode each invalid UTF-8 byte as `\xHH` with exactly two uppercase hexadecimal digits. Only renderer-owned layout and optional renderer-owned ANSI styling MAY emit raw control bytes. This safety rule applies in TTY, non-TTY, colored, and `--no-color` modes. JSON MUST preserve the original logical string through standards-compliant JSON escaping rather than replacing it with the human-display encoding.

**Acceptance.** Snapshot tests cover both TTY and non-TTY modes, with equivalent meaning and no required ANSI sequences. Fixtures containing ESC/CSI/OSC, BEL, carriage return, newlines, invalid UTF-8 path bytes, quotes/backslashes, U+202E, and U+2066 never emit those controls from a data field and remain distinguishable through the quoted encoding. A hostile first-Trust description cannot erase, relabel, link, or visually reorder approval content. Decoding JSON preserves the original valid string value, and JSON stdout remains unaffected by stderr diagnostics.

## SKL-CLI-010 - Explicit Library metadata commands (Revision 1)

**Behavior.** Alias, category, tag, and note changes MUST be exposed only through the nested commands in `SKL-CLI-001`. Generic edit syntax or implicit metadata mutation during add/refresh MUST NOT be part of 0.1.

**Acceptance.** Help and parser tests show the explicit verbs, and add/refresh tests prove user-authored metadata remains unchanged unless one of those verbs runs.

## SKL-CLI-011 - Configuration command boundary (Revision 1)

**Behavior.** `config get|set|unset|list` MUST expose only keys allowed by `SKL-OPS-006`. They MUST validate types and schema before mutation, never reveal secrets, and use the common JSON/error/idempotency contract.

**Acceptance.** Supported keys round-trip through human and JSON modes. Unknown or wrong-typed keys return a structured error without rewriting configuration.

## SKL-CLI-012 - Offline reads and API evolution (Revision 1)

**Behavior.** Read-only CLI commands MUST obey the network and lazy-creation boundaries in `SKL-OPS-005` and `SKL-OPS-008`. JSON API version 1 MAY add optional fields but MUST NOT remove, rename, or reinterpret existing required fields; breaking machine-output changes require a new API version.

**Acceptance.** A version-1 consumer fixture continues to parse later 0.1.x optional-field responses. Offline read tests observe neither network nor filesystem creation.
