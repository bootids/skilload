# Claude Code and Codex Skill Discovery

Scope: native local Skill discovery behavior verified on 2026-08-18. This is integration evidence, not a product specification. Vendor behavior can change and must be rechecked when an Agent adapter changes.

## Why It Matters

skilload deploys links into directories the Agents already scan. It does not wrap either Agent or translate Skill content. Correct target selection, conflict checks, and profile identity therefore depend on the Agents' native discovery rules.

## Claude Code

* A project Skill lives at `.claude/skills/<name>/SKILL.md`.
* A personal Skill lives at `~/.claude/skills/<name>/SKILL.md`. When `CLAUDE_CONFIG_DIR` is set, Claude's documentation says every `~/.claude` path on the configuration page moves under that directory, so the effective personal root is `<CLAUDE_CONFIG_DIR>/skills`.
* A `<name>` entry may be a symlink to a directory elsewhere; Claude follows it and reads the target `SKILL.md`.
* Current Claude documentation says enterprise Skills override personal Skills and personal Skills override project Skills with the same name. Plugin Skills are namespaced. This makes semantic preflight broader than checking only the exact project target.
* Current Claude versions detect many `SKILL.md` changes live, but a newly created top-level skills directory may require restart. skilload intentionally promises visibility only on the next Agent launch, which remains valid across versions.
* Cloud and Cowork sessions do not read a machine's personal Skill directory. skilload 0.1 therefore supports local Claude Code sessions only.

## Codex

* Codex scans `.agents/skills` from the current working directory through the repository root for repository-scoped Skills.
* The current documented personal root is `$HOME/.agents/skills`.
* Codex supports symlinked Skill folders and follows their targets.
* If two discovered Skills share the same frontmatter `name`, Codex does not merge them; both may appear. skilload therefore treats an internal duplicate as invalid and reports confirmed external same-name conflicts as degraded rather than assuming deterministic shadowing.
* The current Codex source still loads `$CODEX_HOME/skills` as a deprecated user location for backward compatibility while also loading `$HOME/.agents/skills`. New skilload global external and manager deployments target the documented `$HOME/.agents/skills`; adapter preflight includes the deprecated root when checking semantic conflicts and profile state.
* Codex detects many Skill changes automatically but its documentation recommends restart when an update is not visible. skilload guarantees next-launch visibility only.

## Integration Conclusions

Workspace sync targets `.claude/skills/<name>` for Claude and `.agents/skills/<name>` for Codex. Global/profile resolution targets the effective Claude personal root and Codex's canonical `$HOME/.agents/skills`. The profile fingerprint retains effective `HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, and all resolved roots because environment changes can alter discovery or conflict results.

Directory symlinks are an appropriate deployment mechanism for both Agents. Native support does not relax skilload's ownership rule: an exact existing user path is never adopted, replaced, or deleted merely because an Agent can read it.

## Cautions

Agent discovery is version-sensitive. In particular, Codex changed its preferred personal root while retaining a compatibility root, and Claude's reload behavior varies by directory creation and content type. End-to-end adapter tests should use fresh Agent processes and assert the roots observed by the installed version rather than relying only on these notes.

## Sources

* [Claude Code: Extend Claude with skills](https://code.claude.com/docs/en/skills)
* [Claude Code: Explore the .claude directory](https://code.claude.com/docs/en/claude-directory)
* [OpenAI: Build skills](https://developers.openai.com/codex/skills)
* [Codex source: host Skill roots at commit 5ee6baee](https://github.com/openai/codex/blob/5ee6baee2fcc0b6ffd413d9611f5538dad40d0f2/codex-rs/ext/skills/src/host_roots.rs)
* [Codex source: symlink-following discovery at commit 5ee6baee](https://github.com/openai/codex/blob/5ee6baee2fcc0b6ffd413d9611f5538dad40d0f2/codex-rs/ext/skills/src/loader/discovery.rs)

Last updated: 2026-08-18.
