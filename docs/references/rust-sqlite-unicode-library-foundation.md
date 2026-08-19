# Rust SQLite 与 Unicode Library 基础依赖

范围：`PLAN-0003` 的可移植 Library 导入/导出持久化基础；记录于 2026-08-19。

## 为什么与本仓库有关

可移植 Library 文档必须在 macOS 和 Linux 上使用同一嵌入式 SQLite 行为，并且 `SKL-LIB-008` 明确要求 Unicode 15.1.0 的 NFC、`White_Space` 裁剪和完整默认大小写折叠。系统 SQLite 或会随版本变化的 Unicode 表都不能构成该契约。

## 已验证结论

* crates.io 在 2026-08-19 将未撤回的 `rusqlite` 最新稳定版列为 `0.40.2`。该版本没有声明 `rust-version`；交付必须使用仓库已固定的 Rust 1.97.1 完整构建、测试和锁定依赖来验证兼容性。
* 应在工作区中声明 `rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }`。关闭默认特性避免引入其默认的缓存和 wasm 后端；`bundled` 选择随包编译的 SQLite，而不是链接各操作系统提供的 SQLite。
* `rusqlite 0.40.2` 的 `bundled` 特性通过 `libsqlite3-sys 0.38.2` 构建 SQLite；该构建脚本明确传入 `SQLITE_ENABLE_FTS5`、`SQLITE_DEFAULT_FOREIGN_KEYS=1` 等编译选项。因此交付应以 `PRAGMA compile_options` 包含 `ENABLE_FTS5` 和实际 FTS5 表创建作为可观测验证，而不是假设宿主 SQLite 的能力。
* `unicode-normalization 0.1.25` 的生成表声明 `UNICODE_VERSION = (17, 0, 0)`，不能用于 Revision 1。`unicode-normalization 0.1.23` 的生成表声明 `UNICODE_VERSION = (15, 1, 0)`；必须以精确版本 `=0.1.23` 声明，不能使用允许 Cargo 解析到更新 Unicode 数据的兼容版本范围。
* 该 crate 提供 NFC，但不提供 Revision 1 所需的完整默认大小写折叠。实现必须将 Unicode 15.1.0 `CaseFolding.txt` 中状态 `C` 与 `F` 的映射以及 `PropList.txt` 的 `White_Space` 集合以受版本控制、离线构建的表纳入仓库；不得在构建或运行时下载 UCD，也不得用区域敏感的小写替代。

## 注意事项

* P2 仅建立 Library 元数据表；即使嵌入式 SQLite 已具备 FTS5，也不得预先暴露 `library search` 或声明 `SKL-LIB-004` 已完成。
* Unicode 15.1.0 表的数据文件和生成输出必须保留上游许可证与版本说明。更新表、`unicode-normalization` 或 SQLite 版本属于刻意依赖变更：更新本参考、锁文件、版本断言与完整验证证据，不能作为顺手升级。
* `rusqlite` 的 `backup` 特性暂不加入；尚未实现的前向数据库迁移和恢复行为继续由后续交付负责。

## 来源

* [crates.io API：rusqlite](https://crates.io/api/v1/crates/rusqlite)
* [rusqlite 0.40.2 Cargo 特性](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/Cargo.toml)
* [libsqlite3-sys 0.38.2 构建脚本](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/libsqlite3-sys/build.rs)
* [unicode-normalization 0.1.23 生成表](https://docs.rs/unicode-normalization/0.1.23/src/unicode_normalization/tables.rs.html)
* [unicode-normalization 0.1.25 生成表](https://docs.rs/unicode-normalization/0.1.25/src/unicode_normalization/tables.rs.html)
* [Unicode 15.1.0 规范与数据来源](unicode-15-1-tag-normalization.md)

最后更新：2026-08-19。
