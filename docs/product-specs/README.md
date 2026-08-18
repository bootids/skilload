# Product Specifications

These files are the authoritative source for skilload's user-visible behavior. `ARCHITECTURE.md` constrains implementation boundaries, and `docs/design-docs/` explains implementation choices, but neither may redefine the behavior specified here.

Every normative behavior has a stable ID and revision. Later ExecPlans must name the exact IDs and revisions they implement or verify. A wording-only clarification keeps the revision; a semantic change increments it or introduces a replacement ID.

`PLAN-0002` implements Revision 1 of `SKL-CLI-002`, `SKL-CLI-003`, `SKL-CLI-011`, and `SKL-OPS-006` as the development `0.0.1` configuration slice. Every other behavior in this baseline remains **planned** for the 0.1 CLI MVP unless its owning specification says otherwise.

## Specification Index

* [Product and release scope](product-and-release-scope.md): `SKL-PROD-001` through `SKL-PROD-007`.
* [GitHub sources and Trust](source-and-trust.md): `SKL-SRC-001` through `SKL-SRC-016` and `SKL-TRUST-001` through `SKL-TRUST-008`.
* [Library](library.md): `SKL-LIB-001` through `SKL-LIB-011`.
* [Workspace](workspace.md): `SKL-WSP-001` through `SKL-WSP-027`.
* [Global deployment and manager Skill](global-and-manager.md): `SKL-GLB-001` through `SKL-GLB-013` and `SKL-MGR-001` through `SKL-MGR-009`.
* [Cache and local operations](cache-and-operations.md): `SKL-CACHE-001` through `SKL-CACHE-010` and `SKL-OPS-001` through `SKL-OPS-010`.
* [Database corruption recovery](database-recovery.md): normative operator procedure `database-corruption-v1` for `SKL-OPS-004`; it introduces no separate behavior ID.
* [CLI contract](cli-contract.md): `SKL-CLI-001` through `SKL-CLI-012`.
* [JSON API version 1 schema catalog](api-v1.md): normative field-level detail for `SKL-CLI-004`, `SKL-CLI-005`, `SKL-CLI-006`, and `SKL-CLI-012`; it introduces no separate behavior ID.

## Normative Language

`MUST`, `MUST NOT`, `SHOULD`, and `MAY` are normative. Each behavior's Acceptance paragraph describes what later implementation evidence must demonstrate. Examples clarify the rule but do not narrow it.
