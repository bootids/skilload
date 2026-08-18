# Product and Release Scope

Status: planned baseline for the skilload CLI MVP.

This specification defines who skilload serves, what the first usable release includes, and how version numbers communicate compatibility. Domain-specific behavior is defined in the other product specifications.

## SKL-PROD-001 - Product purpose and first user (Revision 1)

**Behavior.** skilload MUST let an individual developer who uses local Claude Code and local Codex CLI manage GitHub-hosted Skills as trusted references, organize them in a Library, pin reproducible workspace or global deployments, and expose selected Skills through each Agent's native Skill discovery locations. GitHub remains the source of external Skill content; skilload owns local metadata, desired state, integrity records, cache, and managed links.

**Acceptance.** A complete user journey can add and approve a GitHub Skill, find it in the Library, add and lock it in one workspace, deploy it to both supported Agents, and later restore the same locked content when either GitHub still serves the commit or the verified cache entry remains.

## SKL-PROD-002 - CLI MVP boundary (Revision 1)

**Behavior.** The 0.1 MVP MUST be a canonical command-line application containing the Library, Trust, source migration, workspace, global deployment, manager, cache, configuration, and doctor command families specified by `SKL-CLI-001`. Every command exposed in a release MUST have its real behavior; the product MUST NOT expose stub or scaffold-only commands.

**Acceptance.** `skilload --help` in the 0.1 release lists exactly the specified command families, and every listed command has acceptance coverage rather than returning a generic not-implemented response.

## SKL-PROD-003 - Explicit non-goals for 0.1 (Revision 1)

**Behavior.** The 0.1 MVP MUST NOT include an Agent wrapper or launcher, temporary runtime mounting, Collections, TUI, Web UI, HTTP server, daemon, `init`, shorthand aliases, MCP server, marketplace, Skill compiler, cross-Agent semantic conversion, embeddings, AI classification, cloud sync, accounts, remote Agent sessions, non-GitHub sources, or automatic recommendation and enablement. It MUST NOT create dormant UI or server scaffolding.

**Acceptance.** The shipped command tree and repository implementation contain none of these product surfaces. Documentation may mention them only as non-goals, superseded history, or independently planned future work.

## SKL-PROD-004 - Supported environments (Revision 1)

**Behavior.** The 0.1 release MUST support local Claude Code and local Codex CLI on macOS and Linux, for arm64 and x86_64. Remote/cloud Agent sessions and Windows are outside the supported 0.1 contract. Agent integrations MUST use native filesystem discovery rather than translating Skill semantics.

**Acceptance.** Release and integration tests cover the four OS/architecture artifact targets and both local Agent adapters. Unsupported environments receive a clear diagnostic rather than a false support claim.

## SKL-PROD-005 - Version milestones and distribution (Revision 1)

**Behavior.** Development and incomplete MVP artifacts MUST use `0.0.x`. The complete CLI MVP MUST be released as `0.1.0`, and stable Homebrew distribution begins no earlier than that release. TUI and Web functionality, if later approved, belong to subsequent 0.x minors. Version `1.0.0` is reserved until both TUI and Web exist and the combined product has completed stability testing. GitHub Releases MUST be an official distribution channel; `cargo install` MAY be offered as a developer path.

**Acceptance.** No `0.0.x` release is labelled as the complete MVP or published as the stable Homebrew formula. A `0.1.0` release contains the entire CLI surface and no claim that TUI, Web, or 1.0 stability is complete.

## SKL-PROD-006 - Compatibility and provenance (Revision 1)

**Behavior.** Within `0.1.x`, the CLI command names, JSON API version 1, workspace format, lock format, and Library export format MUST remain backward compatible; a breaking change requires a later minor and an explicit migration path. Version 1.x MUST provide a stronger stable compatibility commitment. Official artifacts MUST publish SHA-256 checksums and GitHub artifact attestations, and the Homebrew formula MUST use the published checksums. Code signing and notarization MAY be added after 0.1.

**Acceptance.** A patch upgrade within 0.1.x reads existing 0.1 formats and preserves documented commands and JSON semantics. Release evidence includes checksums and attestations for every supported artifact, and Homebrew metadata matches those checksums.

## SKL-PROD-007 - Apache-2.0 license continuity (Revision 1)

**Behavior.** The skilload repository and official source distributions MUST remain licensed under the Apache License 2.0. Every official binary release archive MUST include the repository's license text.

**Acceptance.** Repository and source-package metadata identify Apache-2.0, and every supported release archive contains the same `LICENSE` text tracked at the repository root.
