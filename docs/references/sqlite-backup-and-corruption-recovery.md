# SQLite Backup and Corruption Recovery Reference

Scope: SQLite online backup, WAL-mode file generations, integrity checks, and salvage behavior relevant to skilload database migration and `database-corruption-v1`, verified against official SQLite documentation on 2026-08-18. Sidecar behavior of read-only opens was additionally verified by isolated local experiments on 2026-08-20.

## Why It Matters

skilload keeps durable product state in one SQLite database. Migration backups must be consistent even when the source uses WAL mode, and corruption recovery must distinguish a valid backup restore from best-effort row salvage. The operator procedure also needs to preserve the files that make up the observed database generation without presenting them as a ready standalone backup.

## Key Conclusions

* SQLite's online backup API copies an open source database into a destination database and produces a consistent snapshot. It is the correct planned mechanism for a standalone migration backup; copying only a live main database file can miss committed WAL content or produce an inconsistent result.
* On the ordinary Unix and Windows VFSes, an active WAL-mode database may be represented by the main database, `-wal`, and `-shm` files. Evidence preservation after stopping all processes therefore treats those sibling files as one labelled observed generation. A standalone backup produced through the backup API does not reuse a WAL or SHM file from another generation.
* A read-only SQLite connection is not filesystem-inert on a WAL-mode database. Opening one in a writable directory creates and retains a `-shm` sidecar; when the header says WAL mode and no sidecars exist yet, the open creates both `-shm` and `-wal` (verified 2026-08-20 with a WAL+`-wal` fixture and a WAL-header-no-sidecar fixture, both using `mode=ro`). Opening with `immutable=1` avoids creating sidecars but ignores WAL content entirely (`no such table` for tables whose committed content lives only in the WAL), so it cannot be used to diagnose a WAL generation. Consumers that must not touch the live directory therefore classify the generation from the main-file header journal-version bytes (offsets 18 and 19; (1,1) is rollback-journal) and a `-wal`/`-shm` sibling census before any SQLite open, and refuse WAL-mode or sidecar-bearing generations as corruption-class without opening them.
* `PRAGMA integrity_check` returns `ok` when its database checks find no error, but it does not detect foreign-key violations. `PRAGMA foreign_key_check` returns one row per violation, so backup validation needs both forms of evidence when using the external SQLite CLI.
* `PRAGMA quick_check` deliberately skips some work, including UNIQUE-constraint and index-content consistency checks. It is useful diagnostically but is not the strongest optional restore-candidate check.
* SQLite's recovery API and CLI `.recover` command are best-effort salvage mechanisms. Recovered output can violate foreign keys, uniqueness, CHECK constraints, or STRICT typing. skilload may use it only as evidence for manual reconstruction; it is not a validated replacement database.

## Design Consequences

Migration creates and validates a standalone backup through SQLite's backup API before changing schema. The corruption procedure stops processes and preserves the original main/WAL/SHM evidence together, but validates only copies in isolated roots. Restore stages one validated standalone current-schema database and removes stale live WAL/SHM siblings before opening it. An expert salvage attempt runs against an evidence copy and must flow through normal import/re-establishment rather than direct installation.

## Cautions

Filesystem copy and rename durability still require the platform-specific file and parent-directory fsync sequence in the persistence adapter. SQLite's documentation does not make three independent filesystem names atomically replaceable as a set. skilload therefore claims atomic replacement only for the staged standalone live database while processes are stopped, retains the old labelled generation for rollback, and never mixes rows, WAL, SHM, journals, or ownership evidence across generations.

## Sources

* [SQLite Online Backup API](https://www.sqlite.org/backup.html)
* [SQLite online backup C API](https://www.sqlite.org/c3ref/backup_finish.html)
* [SQLite WAL-mode file format](https://www.sqlite.org/walformat.html)
* [SQLite PRAGMA integrity and foreign-key checks](https://www.sqlite.org/pragma.html)
* [Recovering data from a corrupt SQLite database](https://www.sqlite.org/recovery.html)

Last updated: 2026-08-20.
