# Library

Status: 部分实现。`PLAN-0003` 实现了 Revision 4 的 `SKL-LIB-009` 与 Revision 5 的 `SKL-LIB-010`；`PLAN-0005` 将 `SKL-LIB-009` 更新到 Revision 5，以拒绝覆盖已发布的 migration backup pair。`PLAN-0004` 实现了 Revision 1 的 `SKL-LIB-001` 与 `SKL-LIB-008`；`PLAN-0005` 实现了 Revision 2 的 `SKL-LIB-004`、Revision 1 的 `SKL-LIB-005` 与 Revision 1 的 `SKL-LIB-011`（`library list`/`search`/`get` 离线读取、FTS5 索引搜索、确定性分页与 10,000-entry 规模证据）。其余 Library 行为仍为 skilload CLI MVP 的 planned 范围。

The **Library** is the user's durable, searchable collection of source metadata. It is not a content store, Trust store, workspace manifest, or deployment list.

## SKL-LIB-001 - Stable entry identity and metadata (Revision 1)

**Behavior.** A Library entry MUST be identified by canonical source, not by displayed name. It MAY store one optional globally unique alias, one optional free-text category, normalized deduplicated tags, a free-text note, and derived name, description, and repository metadata, all subject to `SKL-LIB-008`. Alias MUST NOT change the verified install name. A derived name MAY be accepted as a convenience selector only when it resolves uniquely; ambiguity MUST return candidates instead of guessing. Tags use the normalization and display contract in `SKL-LIB-008` everywhere they are stored, compared, imported, exported, or indexed.

**Acceptance.** Two sources whose Skills share a name can coexist, while an alias collision fails. Editing category, tags, note, or alias leaves source identity, Trust, pins, and deployments unchanged.

## SKL-LIB-002 - Adding a GitHub source (Revision 1)

**Behavior.** `library add` from a GitHub source MUST resolve and validate the source. If exact Trust does not exist, it MUST use the approval behavior in `SKL-TRUST-003` and `SKL-TRUST-004`. Successful approval MAY create Trust and Library membership in one atomic operation. Adding from workspace state is not implicit.

**Acceptance.** A first source add shows or returns the required approval preview, and refusal leaves neither Trust nor Library entry. Adding an already trusted source needs no second Trust confirmation.

## SKL-LIB-003 - Idempotent re-add (Revision 1)

**Behavior.** Adding an exact source already in Library MUST succeed with `already_exists`. It MUST NOT refresh derived metadata, overwrite user metadata, advance a ref, or reorder unrelated entries.

**Acceptance.** Repeating the same add produces no durable diff and returns the idempotent success outcome even when upstream metadata has changed.

## SKL-LIB-004 - Full-text search fields (Revision 2)

**Behavior.** Library search MUST 使用嵌入式 SQLite FTS5，并索引 verified name、description、alias、tags、category、note 与 repository。每个 tag 都 MUST 同时索引 `SKL-LIB-008` 的 stored display spelling 与 comparison key，使规范等价或大小写等价的 tag 输入得到一致结果，同时不得改变其他 free-text 字段的规范化。`library search <QUERY>` MUST 把 `<QUERY>` 解释为纯文本，而不是 FTS5 查询语言：按仓库固定的 Unicode 15.1.0 `White_Space` code points 分隔非空词项；每个词项生成 NFC 原文及 Unicode 15.1.0 完整默认大小写折叠后再 NFC 的形式；这两个形式作为同一词项的文字 alternatives，所有不同词项必须同时命中同一 entry，但可以命中不同索引字段。实现 MUST 对所有词项进行 FTS5 string quoting，因此 `AND`、`OR`、`NOT`、`NEAR`、引号、星号、括号、冒号与连字符都不得获得查询运算符含义。没有任何词项的查询 MUST 在执行 SQLite 查询前以 `validation_failed` 和 `library_search_query_empty` 失败。

**Acceptance.** 查询可以分别通过每个索引字段（包括 user note）命中 entry，且不读取 Skill content、不联系 GitHub。`code review` 只返回同时含两个词项的 entry，即使两个词项不相邻或位于不同字段；它不要求完整短语。`OR`、`NOT`、`*`、`name:review` 与包含双引号的输入都作为普通文本处理，不得扩大查询范围或触发 FTS5 grammar error。由 `Review`/`review`、组合/分解形式 `café`/`cafe\u0301` 产生的 tag 查询命中同一 entry；空字符串或全为 Unicode 15.1.0 `White_Space` 的查询以规定的 validation error 失败且不创建或修改状态。

## SKL-LIB-005 - Offline reads (Revision 1)

**Behavior.** `library list`, `library search`, and `library get` MUST read only local durable metadata and MUST NOT perform refreshes, update checks, or any other network request. List and search MUST sort their complete matching set by the API-v1 source order before paging. Only those two commands accept `--limit <COUNT>` and `--offset <COUNT>`: limit is an unsigned decimal integer from 1 through 1,000 and defaults to 100; offset is an unsigned 64-bit decimal integer and defaults to 0. Paging skips the first offset matches and returns at most limit entries. An offset at or beyond the complete matching count succeeds with an empty page. JSON MUST return the active offset/limit, returned count, and complete pre-page total defined by `LibraryEntriesData` or `LibrarySearchData`.

**Acceptance.** These commands return the same results with networking disabled and do not mutate timestamps, cache state, or derived metadata. Against unchanged data, omitted arguments equal `--limit 100 --offset 0`, adjacent pages do not overlap, an offset equal to total is empty, and limit 0, limit 1,001, a negative value, or an offset beyond unsigned 64-bit range is a usage error before the query runs.

## SKL-LIB-006 - Explicit refresh (Revision 1)

**Behavior.** `library refresh` MUST be the only Library operation that deliberately retrieves updated derived source metadata. It MUST validate identity and Trust constraints and MUST NOT update workspace locks, global pins, or user-authored alias/category/tags/note. Workspace lock/update/pin and global deployment operations MUST NOT mutate Library metadata as a side effect.

**Acceptance.** Refresh may change derived name or description after approval/preflight but leaves every workspace lock commit and global deployment commit unchanged.

## SKL-LIB-007 - Removal preserves independent state (Revision 1)

**Behavior.** `library remove` MUST delete only Library membership and its Library metadata. It MUST NOT revoke Trust, delete cache, alter workspace files, or remove global desired state/links. Before removal it MUST report known workspace and global references; those references do not prevent explicit removal.

**Acceptance.** Removing a referenced entry succeeds after the normal command confirmation contract, reports the references, and leaves their source records and links intact. Trust remains queryable.

## SKL-LIB-008 - Explicit metadata mutations (Revision 1)

**Behavior.** Library metadata MUST change only through `alias set|clear`, `category set|clear`, `tag add|remove`, and `note set|clear`. Alias is globally unique. Alias and category are valid UTF-8 free text of at most 256 Unicode scalar values and 1,024 UTF-8 bytes each; note is valid UTF-8 free text of at most 4,096 Unicode scalar values and 16,384 UTF-8 bytes. One Library entry may contain at most 64 distinct tag comparison keys. Tags MUST use this exact version-1 algorithm based on Unicode 15.1.0 data: require valid UTF-8; trim code points with the Unicode `White_Space` property from both ends; normalize the remaining text to NFC for the stored display spelling; reject an empty result, any C0/C1 control or DEL, U+2028/U+2029, bidirectional-format code point U+061C, U+200E-U+200F, U+202A-U+202E, or U+2066-U+2069, more than 64 Unicode scalar values, or more than 256 UTF-8 bytes; and preserve all permitted internal whitespace and case in that display spelling. The comparison key is the NFC result of applying Unicode 15.1.0 full default case folding (`CaseFolding.txt` statuses `C` and `F`, never locale-specific `T`) to the display spelling. Tags with the same comparison key are equivalent. `tag add` of an equivalent value succeeds unchanged and retains the existing display spelling; `tag remove` resolves by comparison key. Import applies the same field limits and tag algorithm, retains the first display spelling in document order for duplicate keys, and export emits stored display spellings in comparison-key order. A future Unicode data or algorithm change requires an explicit metadata/schema migration rather than silent renormalization. A missing target returns `not_found`; an already-satisfied mutation succeeds unchanged.

**Acceptance.** Adding ` Review ` and then `review` stores one tag displayed as `Review`; composed `caf\u00e9` and decomposed `cafe\u0301` likewise share one key and retain the first display spelling. Internal whitespace is not collapsed, Turkish case folding is locale-independent, and empty/control/oversized tags, a 65th distinct tag, a 257-scalar or 1,025-byte alias/category, and a 4,097-scalar or 16,385-byte note fail without mutation. Removing through any equivalent spelling removes the one stored value. Clearing an already empty note returns unchanged, and attempting a duplicate alias changes neither entry.

## SKL-LIB-009 - Export boundary (Revision 5)

**Behavior.** Library export MUST 生成确定性、版本化的 JSON，其中只含可移植的 Library 来源与元数据；它 MUST 排除 Trust、全局 desired state、manager records、已知 workspace paths、本机 profile IDs、凭据、cache content 和不可移植的操作时间。`library export --output <PATH>` MUST 向请求的原生路径原子写入且仅写入一个可移植的 `LibraryExportData` 文档；正常的人类输出或 `--json` 操作结果 MUST 保持在该文件之外。创建 staging 文件前，export MUST 通过不跟随 symlink 的检查和有效 XDG root identity 比较，拒绝指向活动 `data/skilload.db`、`skilload.db-wal`、`skilload.db-shm`、`state/locks/database.lock`，或任一已发布 `data/backups/skilload-db-v1-to-v2-*.db` / sibling `.manifest.json` recovery-pair entry 的 target；它不得替换这些 skilload-owned path 或在其处创建临时文件。对其余既有普通文件，export MUST 在其父目录创建临时文件、完整写入并 sync 文件、原子 rename，再 sync 父目录；rename 前的失败 MUST 保留旧 target 或无 target 并清理 staging。若 rename 后的父目录 sync 失败，命令 MUST 返回错误且不得声称旧 target 仍在；新文档 MAY 已可见，但不得报告成功。

活跃 SQLite 在 DELETE rollback journal 模式下出现的 `data/skilload.db-journal` 也是同一 database generation 的成员；export MUST 在创建 staging 前以相同 no-follow、root identity 和 file-identity 规则拒绝该 target，绝不得移动、删除或以 JSON 替换该 journal。

同一 `LibraryExportData` 是当前唯一的可移植传输文档；因此任何 P2 持久 Library 的确定性导出都 MUST 同时不超过 `SKL-LIB-010` 的 10,000-entry 与 67,108,864-byte 输入上限。导出在创建暂存文件前发现任一上限已超出时，MUST 以 `validation_failed` 的 `library_portable_document_entries` 或 `library_portable_document_bytes` 约束值失败且不写 output。实际导入与预演 MUST 在返回计划或写入持久状态前，对完整的导入后 Library 计数同一确定性文档；若它将超过任一上限，MUST 以相同错误拒绝，因此本二进制成功导出的文档始终可由本二进制导入。

在最终 publish 前及 parent-directory sync 后，export MUST 证明 held staging descriptor 仍与已验证 parent descriptor 中的 output entry 相同；若发现 identity drift，MUST 返回错误且不得报告成功，不得删除未知 replacement。rename 前的 drift 保留旧 target 或无 target；rename 后的 drift 允许本调用文档已被外部移动，但请求 output 的未知 replacement 必须保留。

**Acceptance.** 检查 export 找不到本机绝对路径或 authorization/deployment record。未改变 Library 状态的重复 export MUST 产生语义相同、排序稳定的数据，并以原子替换完成请求路径的输出；输出文件本身必须是 `LibraryExportData`，命令的人类或 API-v2 操作结果不得混入其中。针对活动 database、WAL、SHM、database lock、已发布 migration backup DB、matching manifest 及其 same-inode alias 的 target fixture 必须在创建 staging 前失败且保持该路径不变。注入 rename 前失败时旧普通 target 或无 target 必须保留；注入 rename 后父目录 sync 失败时命令必须失败，fixture 可以观察到新 target，且不得把该情形断言为旧 target 保留。若 same-account process 在 rename 后、最终 parent sync 前替换 output entry，命令 MUST 以 identity-drift 错误退出、不得报告成功，并保留 replacement。

活动 DELETE-mode rollback journal target fixture 也必须在创建 staging 前失败且 journal 保持不变。


## SKL-LIB-010 - Atomic import and conflicts (Revision 5)

**Behavior.** `library import --input <PATH> [--dry-run]` MUST 从请求的原生路径读取同一可移植文档；这是当前版本唯一的传输方式，不得增加命令别名或第二种 API 信封。在任何 byte scanner 读取前，import MUST 以不跟随 symlink、nonblocking 的 descriptor 打开 input，并以 descriptor metadata 证明它是 regular file 且仍与已检查的 path identity 一致；symlink、directory、FIFO、socket、device 或 identity drift MUST 在建立 model 或 `ImportPlan` 前失败。Library import MUST 支持 dry-run 并在 mutation 前验证完整版本化 JSON batch。schema deserialization 或 `ImportPlan` construction 之前，streaming non-model pass MUST 停止并拒绝超过 67,108,864 bytes、10,000 entry objects、1,000,000 total JSON values（每个 object、array、string、number、Boolean 或 null 各计一次）、八层 object/array 嵌套、单个 string token 中 1,048,576 UTF-8 bytes 或单个 number token 中 128 bytes 的输入。该 pass 同时 MUST 拒绝 duplicate object keys 和 invalid JSON。完整 schema 和每个 metadata value 随后 MUST 满足 `SKL-LIB-008`；unknown fields 或 wrong types 都是错误。每个 `ResolvedSkill` 的 `entry_count` 与 `byte_count` MUST 为正 decimal value，因为有效来源至少包含非空 regular `SKILL.md`；零值是 invalid metadata。每个 batch 的 `skill.source.canonical` MUST 只出现一次；无论 alias 或其他 metadata 是否相同，后出现的相同 canonical entry 都使整个 batch 以 `conflict` 的 `ConflictDetails` 失败，使用 `kind: "internal_duplicate"`、`name: null`、该被拒绝 entry 的 `source` 以及 null `agent`/`path`。batch 是原子的。existing source 在输入中仅出现一次时默认 kept；alias conflict 同样使整个 batch 失败，并 MUST 返回 `conflict` 的 `ConflictDetails`：每个被拒绝的导入 entry 使用 `kind: "internal_duplicate"`、`name` 为冲突 alias、`source` 为该 entry 的 source，`agent` 与 `path` 均为 null。对原本不存在的 Library，SQLite `COMMIT` 前的失败 MUST 返回错误且不得报告成功。它 MAY 保留 `database.lock`、随机 staging sidecar、data/state directory，或任何在删除时不能同时证明由本调用创建且仍绑定到本调用持有 descriptor 的 entry；不得只凭 create 后 pathname metadata、basename 或 first-observed FD 将此类 residual 视为 owned 并删除。它只能删除仍与 held staging descriptor 一致的 staging entry 或尚未完成的 live link。此类失败不得声称 data/state 已恢复 absence。若 `COMMIT` 已成功而必要 durability sync 返回错误，命令 MUST 返回错误且不得声称旧状态或 state absence。explicit replace mode MAY 只替换 Library metadata，MUST NOT import 或改变 Trust、global state、workspace state 或 local paths。

number token 的计数 MUST 随每个 integer、fraction 与 exponent byte 推进；第 129 byte MUST 立即产生 `library_import_number_bytes` limit error，不能继续扫描该 token 的剩余字节。

超出任一非模型输入 ceiling 的 import MUST 使用 API-v2 `library_input_limit_exceeded` 和 `LimitDetails`，其中 `limit_kind` 标识第一个超出的维度、`measured` 与 `allowed` 为无损 decimal 字符串，host input 使用 `path: PathValue`。该 code 与 API-v1 `agent_input_limit_exceeded` 互不复用。

这些 transfer upper bound 同时约束导入后持久 Library 的可移植表示，而不只是单个输入文件：在模式、领域和冲突规划成功后、任何持久 mutation 或预演结果前，import MUST 拒绝使完整确定性 `LibraryExportData` 超过 10,000 entries 或 67,108,864 bytes 的 batch，并分别使用 `validation_failed` 的 `library_portable_document_entries` 或 `library_portable_document_bytes` 约束值。它们保留单条 alias/category/note/tag 的既有 `SKL-LIB-008` 上限；只防止多次逐次均有效的导入累积出当前唯一传输格式无法重新读取的状态。

**Acceptance.** 含一个 invalid、alias-conflicting 或重复 canonical source entry 的 batch 不作任何更改；每个 alias conflict error MUST 以规定的 `ConflictDetails` 字段标识被拒绝 source 和 alias，而 canonical source duplicate error 的 `name` 必须为 null 且 `source` 为后出现的 entry。zero `entry_count` 或 `byte_count` 的 `ResolvedSkill` 在 durable mutation 前失败。FIFO、device 与其他非常规 input fixture 必须在读入、建立完整 model 或分配 `ImportPlan` 前失败而不阻塞。边界 fixtures 接受每个精确 ceiling，拒绝 byte 67,108,865、entry 10,001、value 1,000,001、level nine、string byte 1,048,577、number byte 129 和 duplicate key，并在返回完整 model 或分配 `ImportPlan` 前失败；structured errors 包含 exceeded dimension 与 measured/allowed values。先导入 10,000 entries 后再导入新的第 10,001 条记录，MUST 在 mutation/预演结果前以 `library_portable_document_entries` 失败，且原状态仍可 export/import。对于先前不存在的 Library，注入 `COMMIT` 前 persistence failure MUST 返回错误且不得报告 success；fixture 中的未知或 foreign staging sidecar、directory、database entry 或 durable lock 必须保留，且测试不得断言 data/state roots 为 absent。仍与 held descriptor 一致的 staging entry 或尚未完成的 live link MAY 被清理。注入 `COMMIT` 后 durability-sync failure 必须返回错误且测试不得断言 state 仍不存在。dry-run 对未变基线 MUST 报告与随后实际 import 相同的 planned additions/keeps/replacements，且不创建 state。

## SKL-LIB-011 - Library scale (Revision 1)

**Behavior.** Library list, indexed search, get, metadata mutation, export, and import MUST support at least 10,000 entries without changing semantics or requiring network access for reads.

**Acceptance.** Performance acceptance uses a 10,000-entry fixture and records bounded completion and deterministic results for representative exact and full-text queries; the concrete time budget is set in the implementation Plan before code is accepted.
