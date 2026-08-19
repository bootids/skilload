# Rust SQLite 与 Unicode Library 基础依赖

范围：`PLAN-0003` 的可移植 Library 导入/导出持久化基础；记录于 2026-08-19。

## 为什么与本仓库有关

可移植 Library 文档必须在 macOS 和 Linux 上使用同一嵌入式 SQLite 行为，并且 `SKL-LIB-008` 明确要求 Unicode 15.1.0 的 NFC、`White_Space` 裁剪和完整默认大小写折叠。系统 SQLite 或会随版本变化的 Unicode 表都不能构成该契约。

## 已验证结论

* crates.io 在 2026-08-19 将未撤回的 `rusqlite` 最新稳定版列为 `0.40.2`。该版本没有声明 `rust-version`；交付必须使用仓库已固定的 Rust 1.97.1 完整构建、测试和锁定依赖来验证兼容性。
* P2 在 workspace 声明 `rusqlite = { version = "0.40.2", default-features = false, features = ["bundled"] }`。关闭默认特性避免引入其默认的缓存和 wasm 后端；`bundled` 选择随包编译的 SQLite，而不是链接各操作系统提供的 SQLite。
* `rusqlite 0.40.2` 的 `bundled` 特性通过 `libsqlite3-sys 0.38.2` 构建 SQLite；该构建脚本明确传入 `SQLITE_ENABLE_FTS5`、`SQLITE_DEFAULT_FOREIGN_KEYS=1` 等编译选项。P2 以 `PRAGMA compile_options` 包含 `ENABLE_FTS5` 作为可观测验证，但不提前创建 FTS 表或暴露 `library search`。
* `unicode-normalization 0.1.25` 的生成表声明 `UNICODE_VERSION = (17, 0, 0)`，不能用于 Revision 1。P2 使用精确 `unicode-normalization =0.1.23`，其生成表声明 `(15, 1, 0)`；不能使用允许 Cargo 解析到更新 Unicode 数据的兼容版本范围。
* P2 将 Unicode 15.1.0 的 `CaseFolding.txt`、`PropList.txt` 与 Unicode License v3 置于 `crates/skilload-core/unicode/15.1.0/`；2026-08-19 获取的文本 SHA-256 分别为 `4e55acfdc32825a22e87670e9056a3bf94ad7c5400065778e9e10f8314372bcf`、`05672956317b6296bc2ec3d6cef1f6452b57ff4f2efc6dc55b0a19277d5fcfd1` 与 `e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96`。build script 只读取这些本地输入，抽取 `C`/`F` case-fold mappings 与 `White_Space`，构建和运行时均不联网。
* P2 直接依赖 `libc =0.2.189`，仅为 Unix `O_NOFOLLOW` 与 `O_NONBLOCK` 常量提供固定来源；它不引入 unsafe code 或新的 native I/O abstraction。
* P2 直接依赖 `rustix = { version = "=1.1.4", features = ["fs"] }`。`rustix::fs::renameat` 接收安全的 `AsFd` directory handle 与相对 path component，使 export 能在持有且已验证 identity 的父目录中发布 staging。相同锁定版本的 `renameat_with(..., RenameFlags::NOREPLACE)` 在 macOS 使用 `renameatx_np(RENAME_EXCL)`、在 Linux 使用 `renameat2(RENAME_NOREPLACE)`，可将首次 database publish 保持为 descriptor-relative no-clobber；`fstat` 加 `statat(..., AtFlags::SYMLINK_NOFOLLOW)` 可将 held staging file identity 与目录 entry 比较。

## 注意事项

* P2 仅建立 Library 元数据表；即使嵌入式 SQLite 已具备 FTS5，也不得预先暴露 `library search` 或声明 `SKL-LIB-004` 已完成。
* Unicode 15.1.0 表的数据文件和生成输出必须保留上游许可证与版本说明。更新表、`unicode-normalization` 或 SQLite 版本属于刻意依赖变更：更新本参考、锁文件、版本断言与完整验证证据，不能作为顺手升级。
* `rustix::fs::renameat` 或 `renameat_with` 必须仅接收已通过 no-follow 打开并经 device/inode 重验的父目录 handle，以及无分隔符的 staging/output 文件名；将任一绝对或未绑定 path 传给它会恢复被本交付禁止的路径重新解析窗口。export 和首次 database publish 都必须在 rename 前后用 `fstat`/`statat(..., SYMLINK_NOFOLLOW)` 比较 held staging FD 与 directory entry；不匹配时不得报告成功，也不得让 `NamedTempFile` path cleanup 删除未知 replacement。
* `rusqlite` 的 `backup` 特性暂不加入；尚未实现的前向数据库迁移和恢复行为继续由后续交付负责。

## 来源

* [crates.io API：rusqlite](https://crates.io/api/v1/crates/rusqlite)
* [rusqlite 0.40.2 Cargo 特性](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/Cargo.toml)
* [libsqlite3-sys 0.38.2 构建脚本](https://raw.githubusercontent.com/rusqlite/rusqlite/v0.40.2/libsqlite3-sys/build.rs)
* [unicode-normalization 0.1.23 生成表](https://docs.rs/unicode-normalization/0.1.23/src/unicode_normalization/tables.rs.html)
* [unicode-normalization 0.1.25 生成表](https://docs.rs/unicode-normalization/0.1.25/src/unicode_normalization/tables.rs.html)
* [Unicode 15.1.0 规范与数据来源](unicode-15-1-tag-normalization.md)
* [`Cargo.lock`](../../Cargo.lock) 中的 `rustix 1.1.4`，以及该已锁版本的本机 `src/fs/at.rs`（`renameat`、`renameat_with`、`statat`）、`src/fs/fd.rs`（`fstat`）和 platform `RenameFlags` 实现。

最后更新：2026-08-19。
