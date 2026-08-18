# npx skills Installation Model

Scope: `skills` npm package version 1.5.22, repository commit `c6f69c631292444cc541ac6d91e2226b0ff247da`, inspected on 2026-08-18. This is comparative implementation research; skilload does not depend on the package at runtime.

## Why It Matters

The existing `npx skills` CLI demonstrates a practical multi-Agent install model: put complete Skill directories in a canonical location and symlink Agent-specific locations to that copy. skilload adopts the useful native-link concept but needs stronger source identity, commit pinning, integrity, Trust, ownership, and transaction guarantees.

## Observed Installation Behavior

* Package 1.5.22 is the npm `latest` version at the inspection date, and its `package.json` at the inspected commit also reports 1.5.22.
* The installer recursively copies a Skill directory, excluding `.git`, `__pycache__`, `__pypackages__`, and `metadata.json`. It therefore installs supporting scripts, references, and assets, not only `SKILL.md`.
* In symlink mode it first recreates and copies into canonical `.agents/skills/<name>` (or `~/.agents/skills/<name>` globally), then creates relative symlinks for Agents with different native paths. Claude Code maps to `.claude/skills` and Codex uses the universal `.agents/skills` path.
* If Agent-link creation fails, the implementation falls back to a direct copy. skilload intentionally does not adopt that fallback because it would create divergent content and weaken one-pin ownership.
* The copy routine dereferences source symlinks and skips broken links. skilload instead preserves only validated relative in-tree links and rejects the rest so integrity is explicit.

## Lock and Reproducibility Differences

The inspected v3 lock entry stores source strings, source type/URL, optional ref and Skill path, a GitHub folder tree SHA, timestamps, and optional provider metadata. It does not bind the source to skilload's required numeric repository ID, exact resolved commit plus canonical cross-host SHA-256 tree digest, Agent target ownership, or transaction baseline. Older lock versions are read as an empty lock rather than migrated.

The folder tree SHA is useful for update detection but is not the same contract as restoring an exact commit. skilload therefore defines its own workspace lock and global pin rather than importing this lock as authoritative state.

## Git LFS and Submodules

The clone implementation deliberately disables Git LFS smudge/filter processing and sets `GIT_LFS_SKIP_SMUDGE=1`, leaving LFS pointer files in the checkout. Its clone options do not request recursive submodule initialization. The installer does not add a later materialization step for either case. skilload must detect and reject unmaterialized LFS pointers and gitlinks rather than silently installing incomplete content.

## Cautions

This package evolves quickly. Its supported Agents, restore/update commands, lock schema, exclusions, and fallback behavior can change independently of skilload. Reinspect a pinned release before using any behavior as design evidence.

## Sources

* [npm registry `latest` metadata](https://registry.npmjs.org/skills/latest)
* [`skills` package manifest at the inspected commit](https://github.com/vercel-labs/skills/blob/c6f69c631292444cc541ac6d91e2226b0ff247da/package.json)
* [Installer and recursive copy implementation](https://github.com/vercel-labs/skills/blob/c6f69c631292444cc541ac6d91e2226b0ff247da/src/installer.ts)
* [Claude/Codex Agent directory mapping](https://github.com/vercel-labs/skills/blob/c6f69c631292444cc541ac6d91e2226b0ff247da/src/agents.ts)
* [Lock v3 schema and behavior](https://github.com/vercel-labs/skills/blob/c6f69c631292444cc541ac6d91e2226b0ff247da/src/skill-lock.ts)
* [Git clone, LFS filter, and shallow-clone behavior](https://github.com/vercel-labs/skills/blob/c6f69c631292444cc541ac6d91e2226b0ff247da/src/git.ts)
* [Project README at the inspected commit](https://github.com/vercel-labs/skills/blob/c6f69c631292444cc541ac6d91e2226b0ff247da/README.md)

Last updated: 2026-08-18.
