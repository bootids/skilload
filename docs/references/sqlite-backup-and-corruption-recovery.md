# SQLite Backup and Corruption Recovery Reference

范围：与 skilload database migration 及 `database-corruption-v1` 相关的 SQLite online backup、rollback-journal/WAL generation、integrity checks 与 salvage 行为；2026-08-21 已按官方 SQLite 文档核验。read-only open 的 sidecar 行为另已于 2026-08-20 通过 isolated local experiments 核验。

## Why It Matters

skilload keeps durable product state in one SQLite database. Migration backups must be consistent even when the source uses WAL mode, and corruption recovery must distinguish a valid backup restore from best-effort row salvage. The operator procedure also needs to preserve the files that make up the observed database generation without presenting them as a ready standalone backup.

## Key Conclusions

* SQLite's online backup API copies an open source database into a destination database and produces a consistent snapshot. It is the correct planned mechanism for a standalone migration backup; copying only a live main database file can miss committed WAL content or produce an inconsistent result.
* On the ordinary Unix and Windows VFSes, an active WAL-mode database may be represented by the main database, `-wal`, and `-shm` files. Evidence preservation after stopping all processes therefore treats those sibling files as one labelled observed generation. A standalone backup produced through the backup API does not reuse a WAL or SHM file from another generation.
* A read-only SQLite connection is not filesystem-inert on a WAL-mode database. Opening one in a writable directory creates and retains a `-shm` sidecar; when the header says WAL mode and no sidecars exist yet, the open creates both `-shm` and `-wal` (verified 2026-08-20 with a WAL+`-wal` fixture and a WAL-header-no-sidecar fixture, both using `mode=ro`). Opening with `immutable=1` avoids creating sidecars but ignores WAL content entirely (`no such table` for tables whose committed content lives only in the WAL), so it cannot be used to diagnose a WAL generation. Consumers that must not touch the live directory therefore classify the generation from the main-file header journal-version bytes (offsets 18 and 19; (1,1) is rollback-journal) and a `-wal`/`-shm` sibling census before any SQLite open, and refuse WAL-mode or sidecar-bearing generations as corruption-class without opening them.
* 在 rollback-journal mode 中，SQLite 会在取得 RESERVED lock 后创建 `-journal`；正常 SQLite pathname reader 可以与该 active writer 共存，而 crash 留下的 hot journal 必须先 rollback 才能读取。descriptor-bound 的 `/dev/fd/<fd>` open 无法安全关联原 pathname 的 journal，因此 skilload 保守地将每个已观察到的 `-journal` sibling 都视为 non-standalone generation：SQLite open 前返回 `database_corrupt` 并保留 main/journal bytes。回归 `rollback_journal_generation_is_rejected_before_sqlite_opens` 覆盖所有 read/default-doctor path；`reads_reject_live_rollback_journals_before_descriptor_opens` 明确覆盖 active writer 期间的可用性取舍。
* `PRAGMA integrity_check` returns `ok` when its database checks find no error, but it does not detect foreign-key violations. `PRAGMA foreign_key_check` returns one row per violation, so backup validation needs both forms of evidence when using the external SQLite CLI.
* `PRAGMA quick_check` deliberately skips some work, including UNIQUE-constraint and index-content consistency checks. It is useful diagnostically but is not the strongest optional restore-candidate check.
* SQLite's recovery API and CLI `.recover` command are best-effort salvage mechanisms. Recovered output can violate foreign keys, uniqueness, CHECK constraints, or STRICT typing. skilload may use it only as evidence for manual reconstruction; it is not a validated replacement database.
* 当普通 schema load 仅因 `library_fts` 的 malformed `sqlite_master` SQL 文本失败时，read-only connection 的 `PRAGMA writable_schema=ON` 仍可读取完整 base tables（2026-08-21 local probe 与 bundled-SQLite regression 均验证）。skilload 只在 descriptor-bound generation gate 之后将它作为 connection-local base inspection tolerance；它不以该 pragma 写 base rows。修复时先删除已证明只属于 FTS 的 virtual/shadow schema rows，再用 `PRAGMA writable_schema=RESET` 关闭并 reload schema；单纯 `OFF` 不会 reload 已缓存的 schema。

## Design Consequences

Migration creates and validates a standalone backup through SQLite's backup API before changing schema. The corruption procedure stops processes and preserves the original main/WAL/SHM evidence together, but validates only copies in isolated roots. Restore stages one validated standalone current-schema database and removes stale live WAL/SHM siblings before opening it. An expert salvage attempt runs against an evidence copy and must flow through normal import/re-establishment rather than direct installation.
live read/default-doctor gate 在已观察到可能改变 main file 含义的 `-journal`、`-wal` 或 `-shm` sibling 时绝不经 descriptor 打开 SQLite，而是返回 typed `database_corrupt`；operator 必须先停止 writer 并保留 complete generation，再进行 recovery。

## Cautions

Filesystem copy and rename durability still require the platform-specific file and parent-directory fsync sequence in the persistence adapter. SQLite's documentation does not make three independent filesystem names atomically replaceable as a set. skilload therefore claims atomic replacement only for the staged standalone live database while processes are stopped, retains the old labelled generation for rollback, and never mixes rows, WAL, SHM, journals, or ownership evidence across generations.

## Sources

* [SQLite Online Backup API](https://www.sqlite.org/backup.html)
* [SQLite online backup C API](https://www.sqlite.org/c3ref/backup_finish.html)
* [SQLite WAL-mode file format](https://www.sqlite.org/walformat.html)
* [SQLite PRAGMA integrity and foreign-key checks](https://www.sqlite.org/pragma.html)
* [Recovering data from a corrupt SQLite database](https://www.sqlite.org/recovery.html)
* [SQLite File Locking and Concurrency](https://www.sqlite.org/lockingv3.html)
* [SQLite Database File Format: rollback journals](https://www.sqlite.org/fileformat2.html)
* [SQLite PRAGMA writable_schema](https://www.sqlite.org/pragma.html#pragma_writable_schema)

最后更新：2026-08-21。
