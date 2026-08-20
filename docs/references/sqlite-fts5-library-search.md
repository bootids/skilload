# SQLite FTS5 Library 搜索参考

范围：`PLAN-0005` 的 Library FTS5 索引、纯文本查询编译、派生索引诊断和 v1→v2 SQLite 迁移；核对日期 2026-08-20。

## 为什么与本仓库有关

skilload 的 `SKL-LIB-004` 要求使用 bundled SQLite FTS5 搜索本地 Library 元数据，同时不能把 SQLite 查询语言意外变成产品接口。已有 v1 database 没有 FTS table，因此新增持久索引还必须遵守 `SKL-OPS-003` 的独立 backup 与 transactional forward migration。默认 `doctor` 又必须只读，不能为了验证 FTS 创建文件或改写 live database。

## 已验证结论

* 仓库锁定的 `rusqlite 0.40.2` 通过 `libsqlite3-sys` bundled build 启用 `SQLITE_ENABLE_FTS5`；现有测试已经用 `PRAGMA compile_options` 验证 `ENABLE_FTS5`。P4 不需要加载动态 SQLite extension。
* FTS5 把未加引号的 `AND`、`OR`、`NOT`、`NEAR`、括号、列过滤和 prefix 标记解释为查询 grammar。官方文档还明确说明，当前作为 syntax error 的未加引号字符可能在未来获得新语义。稳定的纯文本产品接口因此必须把每个用户词项编码成 FTS5 string：以双引号包围，并把内部双引号按 SQL/FTS5 规则写成两个双引号。
* 仅以空白隔开的 quoted phrases 由 FTS5 解释为 implicit AND。`"code" "review"` 因此要求两个词项同时命中，但不要求二者相邻。把整个输入编码成一个 `"code review"` phrase 会额外要求相邻和顺序，不符合本仓库选择的词项 AND 语义。implicit AND 只覆盖裸 quoted phrases：括号表达式与后续 phrase 或另一个括号 group 之间不存在隐式组合，`("Review" OR "review") "code"` 与 `("Review" OR "review") ("code" OR "CODE")` 都返回 `fts5: syntax error`（2026-08-20 isolated 实验证实）。因此带 raw/folded alternatives 的查询编译必须以显式 `AND` 连接各词项 group，例如 `("Review" OR "review") AND "code"` 正常命中。
* 内建 `unicode61` tokenizer 以 Unicode 6.1 的 letter/number/private-use 分类形成 token，并执行其版本固定的大小写无关比较；默认还移除部分 Latin diacritics。`tokenize='unicode61 remove_diacritics 0'` 可避免把不相关 free text 的重音符号主动折叠。它不能替代 `SKL-LIB-008` 固定的 Unicode 15.1.0 tag 规则，所以索引必须同时保存 tag display spelling 与 comparison key，查询编译也必须用仓库本地 Unicode 15.1.0 数据生成 NFC 原文和完整默认大小写折叠后的 NFC alternative。
* FTS5 不承诺产品需要的 canonical-source 顺序；`rank`/`bm25()` 只提供相关性排序。`SKL-LIB-005` 要求在分页前按 source order 排序，因此 list/search SQL 必须显式使用 canonical source 的 binary order，而不能依赖 rowid、MATCH 返回顺序或 rank。
* External-content FTS table 的维护 trigger 不会为创建 trigger 之前的 rows 自动补建索引；官方文档要求显式执行 `rebuild`。Library tags 还是独立多行关系。使用普通 content-bearing FTS table，并由同一 adapter transaction 显式替换每个 entry 的聚合 FTS row，可避免 trigger 与 Rust mutation 各自实现一套 tag 聚合规则。该 table 是派生数据，不是第二个 Library owner。
* FTS5 的 `integrity-check` special command 会验证索引内部一致性；对于 ordinary content-bearing table，它也比较索引和该 FTS table 自己保存的内容。命令形式是向 table 同名 hidden column 执行 `INSERT ... VALUES('integrity-check')`，所以 read-only SQLite connection 会拒绝它，即使成功检查不改变产品数据。默认 doctor 可以从 read-only live connection 使用 SQLite online backup 复制到内存 destination，再在该副本上运行 `integrity-check`，从而保持 live filesystem 和 database bytes 不变。
* FTS5 的 `rebuild` special command会删除并按当前 FTS content 重新生成全文索引。若 FTS content rows 自身与 Library base rows 不一致，doctor 不能只调用 `rebuild`；它必须在 live repair transaction 中丢弃并按已验证 base rows 重新创建、填充整个派生 table。
* `rusqlite 0.40.2` 的 `backup` module 要求 source 与 destination 是两个不同的 `Connection`。`Backup::new` 创建 handle；`run_to_completion(pages_per_step, pause_between_pages, progress)` 重复 step，并允许 source 在分块之间处理并发操作；`Backup` 的 `Drop` 调用 `sqlite3_backup_finish`。该 API 由 crate 的 `backup` feature 提供，适合从 live source generation 生成 standalone migration backup，也适合只读 doctor 的 in-memory snapshot。
* crates.io 在 2026-08-20 将未撤回的 `sha2 0.11.0` 列为当前稳定版；它声明 `rust-version = 1.85`，低于仓库固定的 Rust 1.97.1，许可证为 MIT OR Apache-2.0。P4 可用 `Sha256` 对 backup file 做流式摘要，不需要把完整 database 读入内存或引入第二个编码依赖。

## 对 `PLAN-0005` 的约束

* 产品输入永远不是 raw FTS5 expression。先按仓库固定的 Unicode 15.1.0 `White_Space` 分隔词项，再为每个词项生成原文/完整折叠 alternatives并做 string quoting；同词项 alternatives 以括号内 OR 组成一个 group，不同词项的 group 之间以显式 `AND` 连接（括号 group 与后续词项之间没有 implicit AND，只能显式组合）。空词项集合在 SQLite 前失败。
* Schema v2 使用一个普通 content-bearing `library_fts` virtual table。`canonical_source` 只用于 identity/join，不进入 token index；其余八类字段按产品规格分列。Tag display 和 comparison key 使用不同列，多个值以 tokenizer 明确认作 separator 的 ASCII newline 连接。
* Import、metadata mutation、migration 和 doctor repair 只能通过一个共享的 adapter helper生成 FTS row；changed product transaction 同时维护 base row 与索引。单独修复派生索引不得推进 `state_revision`。
* Read-only doctor 对 live database 只做 no-follow identity-bound read；需要 FTS special command 时只操作内存副本。`doctor --fix` 才可持有 durable database lock、创建 backup 或写 live database。
* v1→v2 migration 必须先通过 SQLite online backup 生成、sync、摘要并验证 standalone backup及其 completed manifest，随后才在 live database 的一个 transaction 中创建/填充 FTS table并更新 schema version。不得复制一个可能遗漏 WAL 的 main file来伪装 backup。

## 注意事项

* `PRAGMA integrity_check` 与 `PRAGMA foreign_key_check` 仍分别负责 SQLite/base-table 与 foreign-key 证据；FTS5 special `integrity-check` 不是二者的替代品。
* FTS table 保存的是可重建的 metadata 副本。Library durable truth仍是 `library_entries` 与 `library_tags`；搜索、doctor 或 migration 不得通过 FTS row反向修改 base metadata。
* Query 语义、tokenizer 配置或 Unicode query-normalization 数据一旦对外发布就属于兼容契约。后续改变它们必须更新产品 behavior revision并提供显式索引重建或 schema migration，不能静默改变同一 revision 的结果。

## 来源

* [SQLite FTS5 Extension](https://www.sqlite.org/fts5.html)：query strings、implicit AND、`unicode61`、external-content pitfalls、`integrity-check`、`rebuild` 与 ranking。
* [rusqlite 0.40.2 `backup.rs`](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/src/backup.rs)：`Backup` 生命周期、step/progress 与 `run_to_completion`。
* [SQLite Online Backup API](https://www.sqlite.org/backup.html)：live database 的一致 standalone snapshot。
* [crates.io `sha2` API](https://crates.io/api/v1/crates/sha2)：0.11.0 版本、Rust version 与许可证 metadata。
* [`rust-sqlite-unicode-library-foundation.md`](rust-sqlite-unicode-library-foundation.md)：本仓库 locked bundled SQLite 与 Unicode 15.1.0 基础。
* [`sqlite-backup-and-corruption-recovery.md`](sqlite-backup-and-corruption-recovery.md)：backup、WAL generation、integrity/foreign-key check 与恢复边界。

最后更新：2026-08-20。
