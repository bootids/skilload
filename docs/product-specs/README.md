# Product Specifications

These files are the authoritative source for skilload's user-visible behavior. `ARCHITECTURE.md` constrains implementation boundaries, and `docs/design-docs/` explains implementation choices, but neither may redefine the behavior specified here.

Every normative behavior has a stable ID and revision. Later ExecPlans must name the exact IDs and revisions they implement or verify. A wording-only clarification keeps the revision; a semantic change increments it or introduces a replacement ID.

`PLAN-0002` 在开发版 `0.0.1` 配置切片中实现了 Revision 1 的 `SKL-CLI-002`、`SKL-CLI-003`、`SKL-CLI-011` 与 `SKL-OPS-006`。`PLAN-0003` 实现可移植 Library 传输切片中 Revision 4 的 `SKL-LIB-009` 与 Revision 5 的 `SKL-LIB-010`，并将当前 JSON producer 切换到 API-v2；`PLAN-0005` 将 `SKL-LIB-009` 更新到 Revision 5，以保护已发布的 migration recovery backup pair。`PLAN-0004` 实现 Revision 1 的 `SKL-LIB-001` 与 `SKL-LIB-008`：canonical-source Library entry 的显式 alias、category、tag 和 note 变更。`PLAN-0005` 实现 Revision 2 的 `SKL-LIB-004`（纯文本词项 AND 的嵌入式 FTS5 搜索）、Revision 1 的 `SKL-LIB-005`（离线 `library list`/`search`/`get` 读取与分页）、Revision 1 的 `SKL-LIB-011`（10,000-entry 规模证据补全）与 Revision 1 的 `SKL-OPS-003`（backup 优先的 v1→v2 transactional forward migration），并交付当前 durable database 的只读 `doctor` 与离线 `doctor --fix`。除非所属规格另有说明，其他基线行为仍为 0.1 CLI MVP 的 **planned** 状态。

## Specification Index

* [Product and release scope](product-and-release-scope.md): `SKL-PROD-001` through `SKL-PROD-007`.
* [GitHub sources and Trust](source-and-trust.md): `SKL-SRC-001` through `SKL-SRC-016` and `SKL-TRUST-001` through `SKL-TRUST-008`.
* [Library](library.md): `SKL-LIB-001` through `SKL-LIB-011`.
* [Workspace](workspace.md): `SKL-WSP-001` through `SKL-WSP-027`.
* [Global deployment and manager Skill](global-and-manager.md): `SKL-GLB-001` through `SKL-GLB-013` and `SKL-MGR-001` through `SKL-MGR-009`.
* [Cache and local operations](cache-and-operations.md): `SKL-CACHE-001` through `SKL-CACHE-010` and `SKL-OPS-001` through `SKL-OPS-010`.
* [Database corruption recovery](database-recovery.md): normative operator procedure `database-corruption-v1` for `SKL-OPS-004`; it introduces no separate behavior ID.
* [CLI contract](cli-contract.md): `SKL-CLI-001` through `SKL-CLI-012`.
* [JSON API version 2 schema catalog](api-v2.md): current normative field-level detail for Revision 2 of `SKL-CLI-004`, `SKL-CLI-005`, `SKL-CLI-006`, and `SKL-CLI-012`; it adds `library_input_limit_exceeded` without reusing the API-v1 Agent-input code.
* [JSON API version 1 schema catalog](api-v1.md): archived historical contract for prior Version 1 producer fixtures; it introduces no separate behavior ID.

## Normative Language

`MUST`, `MUST NOT`, `SHOULD`, and `MAY` are normative. Each behavior's Acceptance paragraph describes what later implementation evidence must demonstrate. Examples clarify the rule but do not narrow it.
