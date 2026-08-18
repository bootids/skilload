# Library

Status: planned baseline for the skilload CLI MVP.

The **Library** is the user's durable, searchable collection of source metadata. It is not a content store, Trust store, workspace manifest, or deployment list.

## SKL-LIB-001 - Stable entry identity and metadata (Revision 1)

**Behavior.** A Library entry MUST be identified by canonical source, not by displayed name. It MAY store one optional globally unique alias, one optional free-text category, normalized deduplicated tags, a free-text note, and derived name, description, and repository metadata. Alias MUST NOT change the verified install name. A derived name MAY be accepted as a convenience selector only when it resolves uniquely; ambiguity MUST return candidates instead of guessing.

**Acceptance.** Two sources whose Skills share a name can coexist, while an alias collision fails. Editing category, tags, note, or alias leaves source identity, Trust, pins, and deployments unchanged.

## SKL-LIB-002 - Adding a GitHub source (Revision 1)

**Behavior.** `library add` from a GitHub source MUST resolve and validate the source. If exact Trust does not exist, it MUST use the approval behavior in `SKL-TRUST-003` and `SKL-TRUST-004`. Successful approval MAY create Trust and Library membership in one atomic operation. Adding from workspace state is not implicit.

**Acceptance.** A first source add shows or returns the required approval preview, and refusal leaves neither Trust nor Library entry. Adding an already trusted source needs no second Trust confirmation.

## SKL-LIB-003 - Idempotent re-add (Revision 1)

**Behavior.** Adding an exact source already in Library MUST succeed with `already_exists`. It MUST NOT refresh derived metadata, overwrite user metadata, advance a ref, or reorder unrelated entries.

**Acceptance.** Repeating the same add produces no durable diff and returns the idempotent success outcome even when upstream metadata has changed.

## SKL-LIB-004 - Full-text search fields (Revision 1)

**Behavior.** Library search MUST use embedded SQLite FTS5 and index verified name, description, alias, tags, category, note, and repository. Tag normalization and deduplication MUST make equivalent tags search consistently.

**Acceptance.** A query can match an entry through each indexed field, including a user note, without reading Skill content or contacting GitHub.

## SKL-LIB-005 - Offline reads (Revision 1)

**Behavior.** `library list`, `library search`, and `library get` MUST read only local durable metadata and MUST NOT perform refreshes, update checks, or any other network request. Their ordering and pagination MUST be deterministic for unchanged data.

**Acceptance.** These commands return the same results with networking disabled and do not mutate timestamps, cache state, or derived metadata.

## SKL-LIB-006 - Explicit refresh (Revision 1)

**Behavior.** `library refresh` MUST be the only Library operation that deliberately retrieves updated derived source metadata. It MUST validate identity and Trust constraints and MUST NOT update workspace locks, global pins, or user-authored alias/category/tags/note. Workspace lock/update/pin and global deployment operations MUST NOT mutate Library metadata as a side effect.

**Acceptance.** Refresh may change derived name or description after approval/preflight but leaves every workspace lock commit and global deployment commit unchanged.

## SKL-LIB-007 - Removal preserves independent state (Revision 1)

**Behavior.** `library remove` MUST delete only Library membership and its Library metadata. It MUST NOT revoke Trust, delete cache, alter workspace files, or remove global desired state/links. Before removal it MUST report known workspace and global references; those references do not prevent explicit removal.

**Acceptance.** Removing a referenced entry succeeds after the normal command confirmation contract, reports the references, and leaves their source records and links intact. Trust remains queryable.

## SKL-LIB-008 - Explicit metadata mutations (Revision 1)

**Behavior.** Library metadata MUST change only through `alias set|clear`, `category set|clear`, `tag add|remove`, and `note set|clear`. Alias is globally unique. Category and note are free text. Tags are normalized and deduplicated. A missing target returns `not_found`; an already-satisfied mutation succeeds unchanged.

**Acceptance.** Adding the same normalized tag twice stores one value, clearing an already empty note returns unchanged, and attempting a duplicate alias changes neither entry.

## SKL-LIB-009 - Export boundary (Revision 1)

**Behavior.** Library export MUST be deterministic, versioned JSON containing portable Library source and metadata only. It MUST exclude Trust, global desired state, manager records, known workspace paths, local profile IDs, credentials, cache content, and operational timestamps that are not portable metadata.

**Acceptance.** Inspecting an export finds no local absolute path or authorization/deployment record. Repeating export over unchanged Library state yields semantically identical data and stable ordering.

## SKL-LIB-010 - Atomic import and conflicts (Revision 1)

**Behavior.** Library import MUST support dry-run and MUST validate the whole versioned JSON batch before mutation. The batch is atomic. Existing sources are kept by default; an alias conflict fails the batch. An explicit replace mode MAY replace Library metadata only and MUST NOT import or alter Trust, global state, workspace state, or local paths.

**Acceptance.** A batch with one invalid or alias-conflicting entry makes no changes. Dry-run reports the same planned additions/keeps/replacements as the subsequent import against unchanged state.

## SKL-LIB-011 - Library scale (Revision 1)

**Behavior.** Library list, indexed search, get, metadata mutation, export, and import MUST support at least 10,000 entries without changing semantics or requiring network access for reads.

**Acceptance.** Performance acceptance uses a 10,000-entry fixture and records bounded completion and deterministic results for representative exact and full-text queries; the concrete time budget is set in the implementation Plan before code is accepted.
