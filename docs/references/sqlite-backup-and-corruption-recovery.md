# SQLite Backup and Corruption Recovery Reference

范围：与 skilload database migration 及 `database-corruption-v1` 相关的 SQLite online backup、rollback-journal/WAL generation、integrity checks 与 salvage 行为；2026-08-21 已按官方 SQLite 文档核验。read-only open 的 sidecar 行为另已于 2026-08-20 通过 isolated local experiments 核验。

## Why It Matters

skilload keeps durable product state in one SQLite database. Migration backups must be consistent even when the source uses WAL mode, and corruption recovery must distinguish a valid backup restore from best-effort row salvage. The operator procedure also needs to preserve the files that make up the observed database generation without presenting them as a ready standalone backup.

## Key Conclusions

* SQLite's online backup API copies an open source database into a destination database and produces a consistent snapshot. It is the correct planned mechanism for a standalone migration backup; copying only a live main database file can miss committed WAL content or produce an inconsistent result.
* On the ordinary Unix and Windows VFSes, an active WAL-mode database may be represented by the main database, `-wal`, and `-shm` files. Evidence preservation after stopping all processes therefore treats those sibling files as one labelled observed generation. A standalone backup produced through the backup API does not reuse a WAL or SHM file from another generation.
* A read-only SQLite connection is not filesystem-inert on a WAL-mode database. Opening one in a writable directory creates and retains a `-shm` sidecar; when the header says WAL mode and no sidecars exist yet, the open creates both `-shm` and `-wal` (verified 2026-08-20 with a WAL+`-wal` fixture and a WAL-header-no-sidecar fixture, both using `mode=ro`). Opening with `immutable=1` avoids creating sidecars but ignores WAL content entirely (`no such table` for tables whose committed content lives only in the WAL), so it cannot be used to diagnose a WAL generation. Consumers that must not touch the live directory therefore classify the generation from the main-file header journal-version bytes (offsets 18 and 19; (1,1) is rollback-journal) and a `-wal`/`-shm` sibling census before any SQLite open, and refuse WAL-mode or sidecar-bearing generations as corruption-class without opening them.
* 在 rollback-journal mode 中，SQLite 会在取得 RESERVED lock 后创建 `-journal`；正常 SQLite pathname reader 可以与该 active writer 共存，而 crash 留下的 hot journal 必须先 rollback 才能读取。descriptor-bound 的 `/dev/fd/<fd>` open 无法安全关联原 pathname 的 journal，因此 skilload 先在 SQLite open 前经 held directory descriptor 拒绝每个已观察到的 `-journal`、`-wal`、`-shm`。随后同一 read transaction 用最小 `PRAGMA schema_version` read 获取 SQLite SHARED snapshot，并在读取 Library schema/data 前再次从 held directory 盘点 companion；第二次发现 companion 返回 `database_corrupt`，而其后才启动的 writer 在 snapshot 结束前不能以 EXCLUSIVE 写入 main image。standalone backup validator使用相同的 snapshot-bound companion check，任何 `.db-journal`、`.db-wal` 或 `.db-shm` 都使 pair 不可广告为可恢复 backup。回归 `read_snapshot_rejects_a_journal_created_after_generation_gate` 与 `backup_inventory_rejects_pairs_with_sqlite_sidecars` 覆盖这两个边界。
* `PRAGMA integrity_check` returns `ok` when its database checks find no error, but it does not detect foreign-key violations. `PRAGMA foreign_key_check` returns one row per violation, so backup validation needs both forms of evidence when using the external SQLite CLI.
* `PRAGMA quick_check` deliberately skips some work, including UNIQUE-constraint and index-content consistency checks. It is useful diagnostically but is not the strongest optional restore-candidate check.
* SQLite's recovery API and CLI `.recover` command are best-effort salvage mechanisms. Recovered output can violate foreign keys, uniqueness, CHECK constraints, or STRICT typing. skilload may use it only as evidence for manual reconstruction; it is not a validated replacement database.
* 当普通 schema load 仅因 `library_fts` 的 malformed `sqlite_master` SQL 文本失败时，read-only connection 的 `PRAGMA writable_schema=ON` 仍可读取完整 base tables（2026-08-21 local probe 与 bundled-SQLite regression 均验证）。skilload 只在 descriptor-bound generation gate 之后将它作为 connection-local base inspection tolerance；它不以该 pragma 写 base rows。修复先删除已证明只属于 FTS 的 virtual/shadow schema rows，再用 `PRAGMA writable_schema=RESET` 关闭并 reload schema；单纯 `OFF` 不会 reload 已缓存的 schema。直接删除 schema rows不会把旧 b-tree pages 加入 freelist：bundled regression 的整库 `PRAGMA integrity_check` 会报告 `Page …: never used`。因此 repair 必须先提交 detach，在同一持锁 connection、无 open transaction 时运行 `VACUUM` 回收不可达 pages并重验 database identity，然后才在第二笔 transaction重建 FTS；SQLite 官方文档明确说明 `VACUUM` 在 connection 有 open transaction 时失败。若进程在 phases 间中断，FTS table 保持 missing/invalid，重复 doctor repair 再执行 detach/compact/rebuild，不会将带 orphan pages 的重建 index 报为 healthy。
* `database_corrupt` details 的候选 migration backup 验证先在 no-follow regular-file descriptor 上检查长度；超过 268,435,456 bytes（当前 67,108,864-byte portable Library document 上限的四倍）的 candidate 在 SQLite open、SHARED snapshot validation 或 SHA-256 前拒绝，不作为可恢复 backup 广告。该 ceiling限制 hostile sparse file 的诊断工作量，不影响已发布且处于该资源预算内的 standalone pair。
* 同一 268,435,456-byte ceiling也约束 migration 自己将要发布的 backup：adapter在创建 staging 前检查 source 的 `page_count × page_size`，并在每个 512-page online-backup step后检查 SQLite 报告的 page count，staging length 在 SHA-256 前再次检查。超过预算时返回 `migration_backup_too_large`，不 hash、不发布 recovery pair、不升级 live schema；这避免“可迁移但其刚创建的 backup 又不被 validator承认”的矛盾。v1 migration baseline还必须拒绝每个非固定 Library base table的 `sqlite_master` object；extra table/index/view/trigger即使自身尚未被查询，也不能进入 backup 或 schema upgrade。
* `rusqlite 0.40.2` 将 `sqlite3_backup_step` 的 `More`、`Busy` 与 `Locked` 暴露为可重试的成功结果，`run_to_completion` 会无限重复它们。skilload 的 migration 不能直接采用无界循环：每次 incomplete 512-page step以现有 `LOCK_RETRY` 让出执行，并以现有 `LOCK_WAIT` 截止为 typed database `busy`，从而不无限持有 durable `database.lock`。
* Migration backup publication必须从已持有的 `data/skilload` descriptor相对创建并打开 `backups` child，且 final DB/manifest pair在 directory sync和任何 post-publication hook后仍须与 held descriptors逐项匹配，才允许 live schema write。诊断 inventory还必须将 manifest 的 source device/inode与当前 diagnosed main-file identity匹配；没有可证明 current main generation的 orphan-sidecar case返回空 inventory，而不是广告可能来自其他 database 的 pair。
* Candidate validation必须区分确定无效内容与 operational failure。缺失、symlink/nonregular、格式不兼容、corrupt、sidecar或超限 pair不被广告；held-directory `openat`、metadata、manifest read、SQLite snapshot、SHA-256 或 final entry-revalidation的 I/O/error 则向 corruption diagnostic 传播为 `XDG_DATA_HOME` failure，不能静默遗漏一个可能需要 operator处理的 recovery asset。

## Design Consequences

Migration creates and validates a standalone backup through SQLite's backup API before changing schema. The corruption procedure stops processes and preserves the original main/WAL/SHM evidence together, but validates only copies in isolated roots. Restore stages one validated standalone current-schema database and removes stale live WAL/SHM siblings before opening it. An expert salvage attempt runs against an evidence copy and must flow through normal import/re-establishment rather than direct installation.
live read/default-doctor gate 先在 SQLite open 前、再在 held descriptor 的 SQLite SHARED snapshot 建立后盘点 `-journal`、`-wal` 或 `-shm` sibling；任一盘点观察到 companion 都返回 typed `database_corrupt`。snapshot 后才出现的 active writer 不能把未提交 main image 混入已持有的 read result；operator 仍必须停止 writer 并保留 complete generation，再进行 recovery。

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
* [SQLite VACUUM](https://www.sqlite.org/lang_vacuum.html)
* [SQLite PRAGMA writable_schema](https://www.sqlite.org/pragma.html#pragma_writable_schema)

最后更新：2026-08-22。
