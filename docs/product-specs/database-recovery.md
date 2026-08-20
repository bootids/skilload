# Database Corruption Recovery Procedure

Status: planned normative out-of-band procedure `database-corruption-v1` for `SKL-OPS-004`.

This procedure is intentionally not a skilload reset command. It is the supported operator path after `database_corrupt` blocks writes because base SQLite records are not provably intact. Paths shown by diagnostics are `PathValue` objects; decode `bytes_base64` when the display form contains escapes or is otherwise ambiguous.

The SQLite backup/WAL/integrity/salvage facts behind the procedure are summarized in [`../references/sqlite-backup-and-corruption-recovery.md`](../references/sqlite-backup-and-corruption-recovery.md).

## 1. Freeze and Preserve Evidence

Stop every skilload process and Agent workflow that could invoke skilload. Do not run `doctor --fix`, move deployment links, delete cache content, or edit workspace manifests during recovery.

Run `skilload doctor --json` once if it can still read the database. Preserve the complete response. Its `database_corrupt` details identify the database, known migration backups, recoverable exports, and this procedure as `database-corruption-v1`.

Create a recovery directory outside all effective skilload config/data/state/cache roots. Copy as one labelled evidence set:

* `skilload.db`;
* sibling `skilload.db-wal` and `skilload.db-shm` when present;
* every listed `data/backups/` database and manifest;
* the doctor response and any successful read-only exports below.

Record byte sizes and SHA-256 digests. Never validate a backup by opening or copying a live WAL database in place; operate on an evidence copy after all processes have stopped. Retain the untouched original set until recovery is accepted.

## 2. Salvage Readable Product Data

当 read 操作成功时，在 replacement 或 reset 前保留其 JSON response；成功的 Library export 还必须将可移植文档保存到本节指定的 recovery directory。


    skilload library export --output <recovery-directory>/library-export.json --json
    skilload trust list --json
    skilload global list --json
    skilload manager status --json --agent claude --agent codex
    skilload doctor --json

在每个仍可访问的已知 workspace 内分别运行 `workspace list --json` 与 `workspace status --json`。保留每个完整 API envelope 以及成功生成的 `library-export.json` 文档；只有该文件才是可直接 import 的 version-1 Library document。Trust、global、manager、workspace、profile、ownership 和 doctor 结果是手工重建的证据，不构成将文件纳入新数据库的授权。失败 read 应记录并跳过，不得以 write-capable repair 重试。

An expert MAY use SQLite's external `.recover` tooling on a copy to inspect otherwise unreadable rows, but recovered SQL is not a supported database replacement and MUST NOT be installed directly. It can only inform a manually reviewed Library import or reconfiguration after reset.

## 3. Select and Validate a Restore Candidate

Consider only a backup whose manifest digest matches, whose recorded schema is not newer than the running binary, and whose creation completed before the corruption event. Try candidates newest first. A timestamp or filename alone is insufficient.

Validate a candidate without touching live roots:

1. Create five empty temporary directories for `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`, and `XDG_CACHE_HOME`; all must be absolute and pairwise non-overlapping.
2. Place a copy at `<temporary-XDG_DATA_HOME>/skilload/skilload.db` with current-user-only permissions. Do not copy a WAL or SHM file from another database generation.
3. Run the same skilload binary with those environment roots and `skilload doctor --json`.
4. If the only blocking database finding is `migration_required` for a supported older schema, keep the original candidate copy unchanged, run `skilload doctor --fix --json` against a second isolated copy, and require the backed-up transactional `SKL-OPS-003` migration to current schema. Record the migrated candidate's new size/digest and the temporary migration-backup evidence. A current-schema candidate skips this step.
5. Require the resulting candidate to produce no `database_corrupt`, `schema_newer`, `migration_required`, foreign-key, or base-row finding. FTS-only findings may be repaired on the isolated copy under the normal `doctor --fix` rule and then must disappear on a second read-only doctor run.
6. Optionally run a compatible external SQLite CLI against the resulting standalone copy with `PRAGMA integrity_check` and `PRAGMA foreign_key_check`; require `integrity_check` to return only `ok` and the foreign-key query to return no row.

If no candidate passes, continue to explicit reset. Never fall back silently from a failed restore to reset.

## 4. Restore Atomically

With all skilload processes still stopped:

1. Reconfirm the live `skilload.db`, WAL, and SHM identities/digests match the preserved evidence set. Drift means another process wrote and recovery must restart.
2. In the live data directory, stage the validated standalone backup under a random temporary name, set restrictive permissions, fsync the file, and fsync the parent directory.
3. Move the corrupt database, WAL, and SHM to a second labelled rollback set outside the live pathname. Do not leave an old WAL or SHM beside the replacement.
4. Atomically rename the staged candidate to `skilload.db` and fsync the parent directory.
5. Run `skilload doctor --json`. Then run `library list`, `trust list`, `global list`, and each known workspace status with networking denied. Require expected identities and no base-database error before permitting mutation.
6. If validation fails, stop all processes, archive the rejected replacement as evidence, remove its WAL/SHM siblings, and restore every member of the original labelled database generation before re-running read-only doctor. The processes remain stopped throughout this logical rollback; do not claim the three path changes are one filesystem-atomic operation and do not merge rows between generations.

After a successful restore, later normal operations may replay only journals whose transaction and database anchor evidence match the restored state. A mismatched journal remains `recovery_blocked`; it is never guessed forward.

## 5. Explicit Destructive Reset

Reset is a last resort and loses every durable domain owned only by SQLite. It is authorized only by the operator physically moving the corrupt database, WAL, and SHM out of the live data directory after completing sections 1 and 2. There is no `skilload reset` command.

With no database file at the live path, the first normal persistent mutation may lazily create an empty current-schema database. Choose an explicit re-establishment order; a validated version-1 `library import` is a suitable first mutation when one was salvaged. After the empty database exists:

* import that Library document if a different first mutation created the database;
* reapprove every source Trust decision; read-only Trust evidence is not importable authorization;
* recreate global desired state and manager installs explicitly;
* revisit every workspace from its surviving portable config/lock files;
* treat existing workspace/global links, local manifests, manager copies, and cache entries as unowned until new exact evidence is established.

skilload MUST NOT adopt those paths. Compare them with the preserved evidence, remove or relocate them manually only when ownership is certain, and then use normal add/sync/install commands to create new ownership records. A cache object may be reused only after exact source, commit, digest, active Trust, and current cache verification all succeed.

Run doctor and every domain list/status after re-establishment. Preserve a written list of irrecoverable Trust, desired deployment, profile, manager, workspace-index, ownership, and transaction data. The reset is accepted only when doctor reports no corruption/recovery blocker and each user-visible desired deployment has been explicitly reconciled.

## 6. Reset Rollback

Before accepting new mutations, rollback is the reverse file-set operation: stop all processes, archive any newly created database/WAL/SHM as a separate generation, and restore the original evidence set together. After new mutations have committed, never overwrite them silently; archive both generations and choose one through the restore validation procedure. Database rows, WAL files, ownership records, and journals from different generations MUST NOT be combined.
