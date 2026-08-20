---
plan_id: PLAN-0004
branch: codex/p3-library-metadata-mutations
pull_request: https://github.com/bootids/skilload/pull/4
status: plan
depends_on: [PLAN-0003]
---

# 建立显式 Library 元数据变更


本 ExecPlan 是动态文档。执行期间必须持续更新 `Progress`、`Surprises & Discoveries`、`Decision Log`、`Outcomes & Retrospective` 和 `Review Conversation Log`。

仓库根目录的 `docs/PLANS.md` 规定本计划的格式、状态转换、恢复和交付规则；维护本文件时必须始终遵守该规范。

## Delivery Metadata


本交付直接依赖 `PLAN-0003`。该前置计划已经在默认分支的 `docs/exec-plans/completed/p2-library-portable-import-export.md` 中完成，提供 Rust workspace、API-v2 当前 producer、受限可移植 Library 导入/导出、Unicode 15.1.0 标签规范化，以及保存 Library 来源和元数据的 SQLite v1 schema。本计划不重复列出传递依赖 `PLAN-0001` 和 `PLAN-0002`。

本交付只增加显式 Library 元数据变更：alias、category、tag 和 note 的 set/add/remove/clear。它不实现 GitHub 来源解析、Trust、`library add|remove|list|search|get|refresh`、FTS5 表、数据库迁移、cache、workspace、global、manager 或 doctor。未实现的命令继续是 usage error，不能注册占位叶子。本计划关联 Draft PR https://github.com/bootids/skilload/pull/4；在后续人类明确触发 `execute-exec-plan` 前不得修改运行时代码。

## Product Baseline


本交付完整实现并验证以下 Revision 1 原子行为，正文语义和 revision 不变。

* `docs/product-specs/library.md` 中 `SKL-LIB-001` Revision 1：Library entry 继续以 canonical source 作为稳定身份；同名来源可共存，alias 全局唯一；alias、category、tags 和 note 的修改不改变 resolved source evidence、Trust、pin 或 deployment。`PLAN-0003` 已建立该数据模型和导入冲突约束，本交付补齐用户可直接执行的修改路径及其独立性证据。
* 同一文件中 `SKL-LIB-008` Revision 1：只允许 `alias set|clear`、`category set|clear`、`tag add|remove` 和 `note set|clear` 修改元数据；执行精确的文本上限、64-tag 上限、Unicode 15.1.0 NFC/White_Space/full-case-fold 标签算法、alias 唯一性、missing-target 和幂等语义。
`docs/product-specs/cli-contract.md` 中 `SKL-CLI-010` Revision 1 是本切片的命令边界约束，不属于本计划的完成 baseline。其 Acceptance 明确要求已实现的 `library add` 和 `library refresh` 经 user-authored metadata preservation tests 验证；两者均被本交付排除并继续是 usage error。八个新增叶子不能引入 generic edit、简写或隐式 mutation；本切片的 parser/usage 覆盖只证明这一局部边界。`SKL-CLI-010` 保持 planned，直到后续交付能够实现并验证其 add/refresh acceptance。

以下行为约束本切片，但本计划不把这些跨产品行为误报为新完成项。`SKL-CLI-007` 和 `SKL-CLI-008` 要求本切片分别返回 `changed`/`unchanged` 与 `not_found`；`SKL-OPS-005` 要求对空状态的 missing-target 失败不创建任何根；`SKL-OPS-008` 要求这些本地修改不联网；`SKL-CACHE-009` 要求数据库 mutation 使用有界进程锁。`PLAN-0003` 已完成 Revision 2 的 `SKL-CLI-004`、`SKL-CLI-005` 和 `SKL-CLI-012`，本交付必须扩充而不能削弱其 API-v2 envelope、错误和离线约束。`SKL-LIB-009` 与 `SKL-LIB-010` 的已完成 revision 继续要求任一持久 Library 可确定性导出并重新导入，因此 changed mutation 在提交前必须证明完整候选 `LibraryExportData` 仍不超过 10,000 entries 和 67,108,864 bytes。

执行完成时，用户可先用 `library import` 建立一个 entry，再以以下唯一新增语法修改它：

    skilload library alias set <SOURCE> <ALIAS>
    skilload library alias clear <SOURCE>
    skilload library category set <SOURCE> <CATEGORY>
    skilload library category clear <SOURCE>
    skilload library tag add <SOURCE> <TAG>
    skilload library tag remove <SOURCE> <TAG>
    skilload library note set <SOURCE> <NOTE>
    skilload library note clear <SOURCE>

`<SOURCE>` 在本切片中只按完整 canonical source 做精确匹配。`SKL-LIB-001` 对 derived-name convenience selector 使用 `MAY`；本切片不启用 name 或 alias selector，避免在没有 `library get` 和 ambiguity result 的情况下增加未定义的选择优先级。set/add 的文本参数是一个逻辑 UTF-8 参数；含空格时由 shell 引号保护，值本身不进行 shell 解释。alias/category/note 的空字符串没有被产品规格禁止，必须与 clear 后的 `null` 保持不同；tag 空值仍按既有标签算法拒绝。

可观察验收为：八个叶子都到达真实 application operation；首次有效修改返回 `changed`，重复满足同一目标返回 `unchanged` 且数据库 bytes 与 `state_revision` 不变；标签等价拼写保留第一个 display spelling，等价 remove 删除同一 key；第二个 entry 争用 alias 时返回包含 alias 和被拒绝 source 的 `conflict`；不存在的 canonical source 返回 API-v2 `not_found`；无效/超限值和第 65 个 tag 不写状态；最终 `library export` 精确反映修改且仍可由隔离实例重新导入。所有成功结果都使用规定的 `LibraryMutationData`，其中 `network` 为 `{used:false, attempts:[]}`，三个 acquisition-policy 字段为 `null`，当前没有 Trust 表的 entry 如实报告 `trust_state: "missing"`。

执行结束时更新 `docs/product-specs/README.md`、`docs/product-specs/library.md` 和 `docs/product-specs/cli-contract.md` 的实现状态文字，但不改上述行为正文或 revision。`SKL-CLI-001`、`SKL-CLI-010`、`SKL-LIB-011` 及其他未完整实现的行为仍保持 planned。

## Design and Architecture Inputs


`ARCHITECTURE.md` 要求依赖向内：CLI 只解析参数、调用 application、渲染结果；application 通过 focused port 编排；domain 不导入 CLI、SQLite 或 filesystem；SQLite adapter 才持有 SQL、XDG path、锁和 durability。Library 只拥有来源与用户元数据，不拥有 Trust、Skill bytes、workspace/global desire、cache 或 deployment。本计划不得为了返回 `trust_state` 创建 Trust 表或虚构授权；在当前可达状态中没有任何命令能创建 Trust，所以 application 将持久 portable entry 映射为 `trust_state: missing`，未来 Trust 交付再通过独立 reader join 真实状态。

`docs/design-docs/application-and-persistence.md` 规定一个 `LibraryRepository` focused port、`data/skilload.db`、`state/locks/database.lock`、单调 `state_revision`、无状态 lazy read/mutation 边界和持久数据库 sync。`PLAN-0003` 的 schema 已包含 `library_entries` 的 alias/category/note 列和 `library_tags(canonical_source, comparison_key, display)`，因此本交付不改变 schema version，不增加 backup/migration 路径，也不创建 `library_fts`。changed mutation 复用现有 no-follow SQLite main-file identity 检查、根与 data-directory identity 重验、两秒数据库锁、事务提交和 file/parent-directory sync；unchanged mutation 不执行 SQL write、不递增 revision、不做 durability sync。

标签必须只调用 `crates/skilload-core/src/domain/unicode_15_1.rs` 已固定的 `normalize_tag`，不能另写 Unicode 或大小写规则。alias/category/note 继续使用 `PortableLibraryDocument::validate` 相同的 scalar/UTF-8 byte 上限，不能引入第二套 validator。对 changed 候选，在内存中只构造一次完整 entry vector，保留一份结果 entry 后将 vector 移交给现有 deterministic portable-size validator；不得为了校验而无谓复制整个最多 67,108,864-byte 文档。

`docs/design-docs/cli-json-and-release.md` 与 `docs/product-specs/api-v2.md` 固定 current producer。每个新增叶子映射准确 operation identifier：`library.alias.set`、`library.alias.clear`、`library.category.set`、`library.category.clear`、`library.tag.add`、`library.tag.remove`、`library.note.set`、`library.note.clear`。JSON stdout 恰好一个 API-v2 value。人类输出为 English，所有 canonical source 和用户元数据通过已有 terminal-safe quoted encoder；不得让换行、ESC、双向格式字符或其他控制内容进入 renderer-owned layout。

不增加 Cargo dependency，不修改 Unicode、SQLite、rustix 或 Rust toolchain 版本，也不扩大当前唯一局部 SQLite FFI 例外。若实现发现需要新的外部或版本特定事实，先核对 `docs/references/`，再把可复用结论写入相应 reference；本计划创建阶段没有产生新的外部研究结论。

## Purpose / Big Picture


`PLAN-0003` 允许用户导入携带元数据的 portable Library，但导入后只能重新导出，不能用 CLI 修改 alias、category、tag 或 note。本交付补上最小的本地管理闭环：用户可以对一个已导入 canonical source 显式修改每类元数据，立即看到 changed/unchanged 或结构化错误，并从 export 验证持久结果。该价值完全离线，不要求先实现 GitHub、Trust、FTS 或部署。

## Progress


- [x] (2026-08-20 07:08Z) 已在干净且与 `origin/main` 同步的 `main` 上完成工具链、GitHub auth、规格、架构、设计、reference、既有 Plan 和实现基线调查；选定 `PLAN-0004` 的显式 Library 元数据切片。
- [x] (2026-08-20 07:08Z) 已创建 `codex/p3-library-metadata-mutations` 分支和本 `plan` 状态 ExecPlan；首个规划提交 `20d47866f78a904099eeb6b47df6c6e9302c4415` 已推送，没有修改运行时代码。
- [x] (2026-08-20 07:18Z) 已创建 Draft PR https://github.com/bootids/skilload/pull/4，写回 canonical URL并完成第二个 planning metadata 提交；推送后等待后续明确的人类执行授权。
- [x] (2026-08-20 08:06Z) 已完整读取 PR #4 的顶层评论、submitted review 和全部 inline threads；两个有效 planning 问题均已分类为可修复，并在不进入 `active`、不修改运行时代码的前提下修正完成范围与 smoke 可复现性。
- [x] (2026-08-20 08:42Z) 已推送 planning 修订 `f2dd223d38666c015bc00f7c597372067da601d0`，在两个 inline threads 分别回复证据并成功关闭；Review Conversation Log 已记录回复 URL、commit、验证和最终 resolved state。
- [ ] 后续收到明确 `execute-exec-plan` 提示后进入 `active`，实现 domain/application/port/error 合约。
- [ ] 实现 SQLite 原子元数据 mutation、幂等路径、portable closure 和 failure/concurrency 回归。
- [ ] 注册八个真实 CLI 叶子，完成 API-v2、人类输出、usage/not-found/conflict 投影与实际 CLI smoke。
- [ ] 同步产品状态、架构和设计文档，完成 focused、workspace、10,000-entry 和 round-trip 验收并记录证据。
- [ ] 实现、验收、文档和 retrospective 全部提交推送后，运行 `gh pr ready`，核对 ready PR head 等于已推送实现 HEAD，再自动进入 `review` 并推送状态提交。
- [ ] 后续收到明确 merge 提示后完成 preflight、进入 `completed`、通过 required checks、合并、更新 `main` 并删除本地交付分支。

## Surprises & Discoveries


- Observation: P2 的 SQLite v1 schema 和 portable domain 已保存并验证本切片所需的全部 metadata，不需要 schema migration。
  Evidence: `crates/skilload-core/src/adapters/sqlite_library.rs` 的 `library_entries` 已含 alias/category/note，`library_tags` 已含 comparison key/display；`domain/library.rs` 已实施文本、tag、alias-conflict 和 portable-size规则。
- Observation: API-v2 `LibraryMutationData.entry` 强制包含 `trust_state`，但当前 schema 和命令面有意没有 Trust。
  Evidence: `docs/product-specs/api-v2.md` 要求 `LibraryEntry.trust_state`；`ARCHITECTURE.md` 和完成的 `PLAN-0003` 明确 P2 不建立 Trust，portable import 也不得授权。
- Observation: changed metadata 可能让原本合法的完整 portable Library 越过 67,108,864-byte 上限，即使单字段仍符合 `SKL-LIB-008`。
  Evidence: `SKL-LIB-009`/`SKL-LIB-010` 要求每个持久 Library 的唯一传输表示保持 export/import closure；当前 import plan 已对完整候选文档执行该检查。
- Observation: 当前 `AppError` 没有 `not_found`/`LookupDetails` 领域数据，CLI parser operation 识别函数仍以 configuration 命名但已临时覆盖 import/export。
  Evidence: `crates/skilload-core/src/error.rs` 的 enum 不含 lookup variant；`crates/skilload-cli/src/args.rs` 的 `json_configuration_operation` 已同时识别 Library transfer 叶子。
- Observation: `SKL-CLI-010` 的完整 acceptance 依赖尚未实现的 `library add` 与 `library refresh`，不能由本切片对 unknown-command 的 parser rejection 替代。
  Evidence: `docs/product-specs/cli-contract.md` 要求 add/refresh tests 证明用户元数据不变；本 Plan 的 Delivery Metadata 明确排除这两个命令。

## Decision Log


- Decision: `PLAN-0004` 只实现显式 Library 元数据 mutation，不同时实现 list/get/search 或 FTS。
  Rationale: 该范围通过 import → mutation → export 独立可观察，并直接复用 P2 schema。search 需要 FTS query semantics、derived index、schema migration、backup 和 doctor-repair 路径，属于更大的独立交付；混入本 PR 会降低可审阅性。
  Date/Author: 2026-08-20 / Codex
- Decision: 唯一直接依赖为 `PLAN-0003`，计划编号为 `PLAN-0004`，分支与文件 slug 都使用 `p3-library-metadata-mutations`。
  Rationale: `PLAN-0003` 是提供当前数据库与 portable Library model 的直接前置；更早计划均为传递依赖，仓库四个状态目录没有其他 Plan ID 或 P3 分支。
  Date/Author: 2026-08-20 / Codex
- Decision: 八个叶子使用 `<SOURCE>` 加可选第二个 positional value；本切片只精确匹配完整 canonical source。
  Rationale: 这与现有 `config set <KEY> <VALUE>` 风格一致，并落实 canonical-source identity。name selector 在产品中是可选能力，alias selector 和优先级未被规定；不增加它们避免 ambiguity shim 和未来不兼容。
  Date/Author: 2026-08-20 / Codex
- Decision: alias/category/note 的空字符串是有效 set 值，不等同于 clear；tag 空字符串继续拒绝。
  Rationale: Revision 1 对前三者只规定 UTF-8、scalar 和 byte 上限，没有最小长度；tag 算法则明确拒绝 trim 后为空。实现不能私自收紧产品语义。
  Date/Author: 2026-08-20 / Codex
- Decision: application result 明确携带 `LibraryEntry` 和当前 `Missing` trust state；SQLite port 仍只返回其拥有的 portable entry。
  Rationale: presentation 不应猜测领域结果，SQLite Library repository 也不应越界拥有 Trust。当前命令面无法产生 Trust，`missing` 是可证明的真实状态；未来 Trust reader 可以在 application 层替换该映射。
  Date/Author: 2026-08-20 / Codex
- Decision: 不升级 schema、不创建 FTS、不增加 dependency；changed mutation 使用既有 database lock、read-write/no-follow identity gate、单个 SQLite transaction 和 durability sync。
  Rationale: 所需列和约束已存在。无 schema 变化就不应提前引入尚未实现的 forward-migration/backup 系统；复用 P2 原语也避免第二套并发和文件身份协议。
  Date/Author: 2026-08-20 / Codex
- Decision: changed 候选在 SQL write 前验证完整 portable transfer closure；unchanged 候选在领域比较后立即返回，不重新编码或写数据库。
  Rationale: 前者维持已完成的 export/import 不变量，后者符合幂等 mutation 不重写状态并避免无意义的最多 67 MiB 编码。
  Date/Author: 2026-08-20 / Codex
- Decision: 10,000-entry acceptance 对已建立 fixture 的 changed 和 unchanged 代表操作各设 10 秒上限，计时不含 fixture 构造，并记录 release build 实测值；永久测试验证 10,000-entry 语义，但不以共享 CI wall-clock 断言制造 flaky gate。
  Rationale: `SKL-LIB-011` 要求实现计划先给出具体预算；10 秒为本地单用户 CLI mutation 提供明确上界，同时把机器调度噪声与语义回归分开。该 Plan 不声称完整完成仍缺 list/search/get 的 `SKL-LIB-011`。
  Date/Author: 2026-08-20 / Codex
- Decision: `SKL-CLI-010` 保持 planned，且从 `PLAN-0004` 的完成 Product Baseline 移出。
  Rationale: 该行为要求 future `library add` 和 `library refresh` 的 user-authored metadata preservation tests；本 PR 的八个 metadata leaves 可独立完成 `SKL-LIB-001` 与 `SKL-LIB-008`，但不能伪称验证了尚不存在的命令。
  Date/Author: 2026-08-20 / Codex
- Decision: 实际 CLI smoke 固定调用仓库构建的 `./target/debug/skilload`，并显式创建、导出和清理临时 HOME/XDG roots。
  Rationale: 裸 `skilload` 可能缺失或解析到用户安装的旧二进制；明确的绝对 roots 使 mutation 不接触真实本机状态，并使 newcomer 可复现 import、mutation、export 和隔离 re-import。
  Date/Author: 2026-08-20 / Codex

## Outcomes & Retrospective


规划基线已完成并关联 Draft PR https://github.com/bootids/skilload/pull/4；首个规划提交已推送，本 metadata 更新提交将 URL、Progress 和 publication evidence 写回。2026-08-20 08:06Z 至 08:42Z 已处理两项 planning review：`SKL-CLI-010` 改为未完成的跨命令约束，实际 CLI smoke 改为显式仓库二进制与临时 XDG roots；修订已作为 `f2dd223d38666c015bc00f7c597372067da601d0` 推送，两个 inline threads 都已回复并 resolved。没有运行时代码、测试或产品实现状态变化。预期结果仍是八个 canonical Library 元数据叶子拥有真实的离线、原子、幂等行为，并由 portable export/import round trip 证明；search、Trust、add、refresh 和其他 Library 命令保持明确缺席。当前等待单独的人类执行授权。

## Review Conversation Log


### PRRT_kwDOT7YN2s6auhyi — `SKL-CLI-010` 完成范围

Source: 内联线程 `PRRT_kwDOT7YN2s6auhyi`，评论 `PRRC_kwDOT7YN2s7jqe3C`，https://github.com/bootids/skilload/pull/4#discussion_r3819564482（`chatgpt-codex-connector`；未过期；最终已解决）。

Problem: 当前 Plan 把 `SKL-CLI-010` 写入完整完成 baseline，但该行为的 acceptance 要求已实现的 `library add` 和 `library refresh` 经用户元数据保持测试验证；本交付明确不实现这些命令。

Disposition: fixed.

Status: resolved.

Resolution: 提交 `f2dd223d38666c015bc00f7c597372067da601d0` 修改 `docs/exec-plans/plan/p3-library-metadata-mutations.md`：从完成 Product Baseline 移除 `SKL-CLI-010` bullet，将其说明为 planned 命令边界约束，并在 Product Baseline、实现状态、Discovery、Decision Log、Outcomes 和 revision note 中说明其 add/refresh acceptance 留待后续交付。

Evidence: 推送提交 `f2dd223d38666c015bc00f7c597372067da601d0`；`git diff --check` 通过；Plan 校验确认只有该 Plan 变更、frontmatter 仍为 `status: plan`、`SKL-CLI-010` 不再是完成-baseline bullet；`docs/product-specs/cli-contract.md` 的 acceptance 是此处 no-overclaim 的依据。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/4#discussion_r3819949870；thread resolved: true。

### PRRT_kwDOT7YN2s6auhym — 可复现的实际 CLI smoke

Source: 内联线程 `PRRT_kwDOT7YN2s6auhym`，评论 `PRRC_kwDOT7YN2s7jqe3J`，https://github.com/bootids/skilload/pull/4#discussion_r3819564489（`chatgpt-codex-connector`；未过期；最终已解决）。

Problem: smoke block 使用裸 `skilload`，且未给出临时 HOME/XDG roots 的具体设置，可能调用 PATH 中不存在或错误版本的二进制，并污染真实状态。

Disposition: fixed.

Status: resolved.

Resolution: 提交 `f2dd223d38666c015bc00f7c597372067da601d0` 将 Concrete Steps 的 smoke 改为完整可复制脚本：绝对 `mktemp -d` root、HOME/XDG exports、portable input、`trap` 清理、每次真实调用均使用 `./target/debug/skilload`，并以第二套显式 roots 做 re-import/export。

Evidence: 推送提交 `f2dd223d38666c015bc00f7c597372067da601d0`；`git diff --check` 通过；Plan 校验确认只有该 Plan 变更、11 处 `./target/debug/skilload` 调用和合法 portable fixture；提取的 smoke shell 脚本已通过 `sh -n`。

GitHub outcome: 已回复 https://github.com/bootids/skilload/pull/4#discussion_r3819951369；thread resolved: true。

## Context and Orientation


仓库是 Rust 1.97.1 Cargo workspace。`crates/skilload-core` 按 `domain`、`application`、`ports`、`adapters` 分层；`crates/skilload-cli` 是唯一二进制，`args.rs` 定义 clap 树，`main.rs` 组合 adapters 和 dispatch，`json.rs` 生成 API-v2，`human.rs` 生成 terminal-safe English 文本，`tests/cli_contract.rs` 通过隔离 XDG roots 驱动真实二进制。

当前真实命令只有 `config get|set|unset|list`、`library import --input <PATH> [--dry-run]` 和 `library export --output <PATH>`。`crates/skilload-core/src/domain/source.rs` 定义 `SourceIdentity` 和 `ResolvedSkill`；canonical source 是类似 `github:owner/repository#skills/review@refs/heads/main` 的完整稳定 key。`domain/library.rs` 定义 `PortableLibraryEntry`、portable document、metadata validation、确定性排序和 transfer-size 检查。`domain/unicode_15_1.rs` 根据仓库固定数据返回 tag display spelling 与 comparison key。

`ports/library.rs` 当前只公开 `export` 和 `import`；`application/library.rs` 调用 repository 与 transfer store。`adapters/sqlite_library.rs` 的 v1 schema 将 canonical source 作为 `library_entries` 主键，alias 具有 SQLite `UNIQUE`，tags 以 `(canonical_source, comparison_key)` 为主键并 cascade 到 entry。它已经实现 absent database 检查、orphan sidecar 诊断、read-only/read-write no-follow open、SQLite main-file identity FFI、schema/integrity/foreign-key validation、两秒 durable lock、`state_revision`、事务写入及 durable sync。不得绕过这些 helper 直接从 CLI 或 application 发 SQL。

“portable closure”表示当前持久 Library 的确定性 `LibraryExportData` 同时不超过 10,000 entries 和 67,108,864 bytes，因此 export 的结果必能重新通过同一二进制的 import 上限。单字段合法不自动证明完整文档仍合法；changed mutation 必须在 commit 前重算候选文档。`state_revision` 是产品状态的单调 revision：每个 changed transaction 恰好递增一次，unchanged 和 error 不递增。

“terminal-safe quoted encoding”是 `human.rs` 已有的可逆显示规则：外层 ASCII 双引号，控制字符和无效 native bytes 使用可见 escape。新增 renderer 必须复用 `quote_string`，不能用裸 `Display` 输出 canonical source 或 user metadata。

## Plan of Work


Milestone 1 建立 presentation-neutral mutation 合约。更新 `crates/skilload-core/src/domain/library.rs`，加入八种明确 change、changed field、metadata store result、带 trust state 的 Library entry 和 application operation result；公开构造函数在进入 port 前验证 alias/category/note/tag。更新 `error.rs`，加入 logical-selector `NotFound` variant、code `not_found` 和 exit 4。更新 `ports/library.rs` 增加一个 focused `mutate_metadata` 方法；更新 `application/library.rs`，为八个 CLI 叶子提供明确方法并把 port 返回的 portable entry 组合为 `trust_state: missing`。完成后，domain tests 能独立证明边界、空字符串、标签等价与 operation outcome，不涉及 SQLite 或 CLI。

Milestone 2 实现 SQLite 原子 mutation。重构 `adapters/sqlite_library.rs` 的既有 database 打开、验证和锁路径供 import 与 metadata mutation 共同使用，不复制安全协议。数据库 absent 且无 orphan generation 时直接返回 `NotFound`，不得创建 data/state/cache/config root。数据库存在时先取得 `database.lock`，重验 roots 与 data directory，no-follow 打开同一 main-file identity，在一个稳定 snapshot 中加载并验证完整 entries，按 canonical source 精确定位。先在内存应用 change：missing entry 返回 `NotFound`；相同值、等价 tag add、缺失 tag remove 或已 clear 字段返回 unchanged；alias conflict 返回既有 `Conflict::internal_duplicate`；第 65 个 tag或文本/portable ceiling 失败不写。changed 路径再开启 SQL transaction，恰好修改目标列或 tag row、递增一次 revision、核对 affected row count、commit，并复用 file 和 parent-directory sync 后才返回 success。commit 后 sync failure 返回 error，不声称旧状态；重试同一目标必须收敛到 unchanged。

Milestone 3 暴露八个真实 CLI 叶子。更新 `args.rs` 的 nested clap enums 和 operation detection，保留所有其他未实现叶子为 unknown command。set/add/remove 的 logical value 允许 shell 已传入的 leading hyphen，不进行 shell splitting；parser error 在 `--json` 时仍输出可识别 operation 的单一 API-v2 usage envelope。更新 `main.rs`，每个叶子只构造一次 application 请求。更新 `json.rs`，生成 API-v2 `LibraryMutationData` 的完整 required fields；更新 `human.rs`，输出 operation、outcome、quoted source、changed fields、trust state 和最终 metadata。加入 `NotFound` 的 `LookupDetails {domain:"library", selector:<SOURCE>, path:null}` 与 terminal-safe human error。

Milestone 4 完成行为、规模和文档验收。扩展 core adapter failure/concurrency tests和 `crates/skilload-cli/tests/cli_contract.rs`：八叶子 routing、changed/unchanged、missing target、alias conflict、Unicode tag、边界、hostile output、portable ceiling、state revision、database identity、busy 和 import → mutations → export → isolated re-import。以 10,000-entry fixture 验证代表 alias set 和 equivalent tag add 的语义，并用 release binary 记录 changed/unchanged 各自不超过 10 秒的实测值。同步产品 status、`ARCHITECTURE.md` 当前模块描述以及 `docs/design-docs/application-and-persistence.md`、`docs/design-docs/cli-json-and-release.md`；只有发现可复用的新外部事实时才更新 reference。最终完整 gates 和实际 CLI smoke 都通过后更新本计划的所有 living sections 与 retrospective。

## Concrete Steps


所有命令从仓库根目录执行。进入 `active` 后先确认工具链，再在修改 exported symbol 前用 LSP references 检查调用点：

    mise install

实现时先运行 focused tests，测试名称可随最终模块组织调整，但命令必须覆盖下列 crate 和真实二进制：

    mise exec -- cargo test -p skilload-core --all-features --locked library
    mise exec -- cargo test -p skilload-core --all-features --locked sqlite_library
    mise exec -- cargo test -p skilload-cli --all-features --locked

格式化后运行完整本地 gates：

    mise exec -- cargo fmt --all
    mise exec -- cargo fmt --all --check
    mise exec -- cargo clippy --workspace --all-targets --all-features -- -D warnings
    mise exec -- cargo test --workspace --all-features --locked
    mise exec -- cargo build --workspace --all-features --locked

实际 CLI smoke 必须从仓库根目录调用刚构建的 `./target/debug/skilload`，不依赖 PATH。以下命令创建绝对临时 roots、生成与 `crates/skilload-cli/tests/cli_contract.rs` portable fixture 同一 schema 的合法单-entry input，并在 shell 退出时清理全部状态：

    SMOKE_ROOT="$(cd "$(mktemp -d)" && pwd -P)"
    trap 'rm -rf "$SMOKE_ROOT"' EXIT HUP INT TERM
    export HOME="$SMOKE_ROOT/home"
    export XDG_CONFIG_HOME="$SMOKE_ROOT/config"
    export XDG_DATA_HOME="$SMOKE_ROOT/data"
    export XDG_STATE_HOME="$SMOKE_ROOT/state"
    export XDG_CACHE_HOME="$SMOKE_ROOT/cache"
    mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME"
    INPUT="$SMOKE_ROOT/portable-library.json"
    OUTPUT="$SMOKE_ROOT/exported-library.json"
    SOURCE='github:owner/repository#skills/review@refs/heads/main'
    cat > "$INPUT" <<'JSON'
    {
      "format_version": 1,
      "entries": [
        {
          "skill": {
            "source": {
              "canonical": "github:owner/repository#skills/review@refs/heads/main",
              "owner": "owner",
              "repository": "repository",
              "repository_display": "Repository",
              "path": "skills/review",
              "ref_kind": "branch",
              "ref_value": "refs/heads/main"
            },
            "repository_id": "42",
            "commit": "0123456789012345678901234567890123456789",
            "integrity": "sha256:0123456789012345678901234567890123456789012345678901234567890123",
            "name": "review",
            "description": "Portable Library entry",
            "entry_count": "1",
            "byte_count": "10"
          },
          "alias": null,
          "category": null,
          "tags": [],
          "note": null
        }
      ]
    }
    JSON
    ./target/debug/skilload library import --input "$INPUT" --json
    ./target/debug/skilload library alias set "$SOURCE" review-alias --json
    ./target/debug/skilload library category set "$SOURCE" 'Code Review' --json
    ./target/debug/skilload library tag add "$SOURCE" ' Review ' --json
    ./target/debug/skilload library tag add "$SOURCE" review --json
    ./target/debug/skilload library note set "$SOURCE" 'Local note' --json
    ./target/debug/skilload library export --output "$OUTPUT" --json

预期每个首次有效修改为 `changed`，第二次等价 tag add 为 `unchanged`；JSON 有 `api_version: 2`、准确 operation、`trust_state: "missing"`、空 network attempts 和 null policy fields。export 文档显示 alias `review-alias`、category `Code Review`、唯一 display tag `Review` 和 note `Local note`。随后以相同的 `./target/debug/skilload` 调用 clear/remove 并重复一次；首次为 `changed`，重复为 `unchanged`。将 `OUTPUT` 导入第二组明确隔离的 roots 后再次 export，两个 portable document 必须语义相同：

    REIMPORT_ROOT="$SMOKE_ROOT/reimport"
    REIMPORT_OUTPUT="$REIMPORT_ROOT/exported-library.json"
    mkdir -p "$REIMPORT_ROOT/home" "$REIMPORT_ROOT/config" "$REIMPORT_ROOT/data" "$REIMPORT_ROOT/state" "$REIMPORT_ROOT/cache"
    env HOME="$REIMPORT_ROOT/home" XDG_CONFIG_HOME="$REIMPORT_ROOT/config" XDG_DATA_HOME="$REIMPORT_ROOT/data" XDG_STATE_HOME="$REIMPORT_ROOT/state" XDG_CACHE_HOME="$REIMPORT_ROOT/cache" ./target/debug/skilload library import --input "$OUTPUT" --json
    env HOME="$REIMPORT_ROOT/home" XDG_CONFIG_HOME="$REIMPORT_ROOT/config" XDG_DATA_HOME="$REIMPORT_ROOT/data" XDG_STATE_HOME="$REIMPORT_ROOT/state" XDG_CACHE_HOME="$REIMPORT_ROOT/cache" ./target/debug/skilload library export --output "$REIMPORT_OUTPUT" --json

10,000-entry acceptance 使用 release binary，fixture 构造和首次 import 不计入 mutation 计时；记录 changed alias set 与 unchanged repeat 的 wall-clock，以及 equivalent tag add 的 wall-clock，每项要求小于等于 10 秒：

    mise exec -- cargo build --release --workspace --all-features --locked
    /usr/bin/time -p target/release/skilload library alias set <SOURCE> <ALIAS> --json
    /usr/bin/time -p target/release/skilload library alias set <SOURCE> <ALIAS> --json
    /usr/bin/time -p target/release/skilload library tag add <SOURCE> <EQUIVALENT-TAG> --json

将机器、fixture entry count、命令、`real` 值和结果 outcome 摘要写入本计划 `Artifacts and Notes`；不要提交临时 fixture 或 XDG state。

实现完成后检查完整 diff 和 whitespace，再提交、推送。`execute-exec-plan` 流程必须在实现/测试/文档提交均已推送后执行 ready/review 事务，不能在本 `plan` 阶段提前执行：

    git diff --check
    gh pr ready <PR-URL>
    gh pr view <PR-URL> --json isDraft,headRefOid

只有观察到 `isDraft: false` 且 `headRefOid` 等于已推送实现 HEAD，才移动本文件到 `docs/exec-plans/review/`、设置 `status: review`、记录证据并提交推送。

## Validation and Acceptance


Domain acceptance 必须覆盖 alias/category 各 256 scalars/1,024 bytes 接受及下一 scalar/byte 拒绝，note 4,096 scalars/16,384 bytes 接受及下一 scalar/byte 拒绝，空 alias/category/note 与 clear 的区别，tag exact Unicode 15.1.0 normalize/fold/forbidden/size规则，以及第 64/65 个 distinct comparison key。每个失败都发生在 repository write 前。

Repository acceptance 必须以真实 bundled SQLite 验证：两个同名 resolved Skill 可共存；同 alias 的第二个 source 返回 conflict 且两行不变；每类 changed mutation 只改目标 metadata 并恰好增加 revision 一次；每类 unchanged 路径保持数据库 bytes 和 revision；missing database/entry 返回 not_found，空环境不创建根；portable byte ceiling 的最后合法候选成功、下一 byte 候选失败且 export 不变；lock contention 在两秒后返回 `busy`；database/parent identity drift、corrupt schema、orphan sidecar、commit failure 和 post-commit sync failure沿用 P2 的分类与保守恢复，不删除 unknown path。

CLI acceptance 必须逐叶验证 help 和 parser：只新增八个真实叶子；`library list|get|search|add|remove|refresh` 仍拒绝。每个 JSON success 恰好一个 value，operation、outcome、`LibraryMutationData` required fields 和 decimal/path/string表示符合 API-v2。`not_found` 使用 exit 4 与 LookupDetails；duplicate alias 使用 exit 4 与 ConflictDetails。人类输出与 JSON 表达同一 source、entry 和 outcome，hostile alias/category/tag/note/canonical fixture 不产生裸控制字符。closed stdin 不阻塞；全流程不连接网络、不触发外部 executable。

端到端 acceptance 是实际二进制 import → 八类 mutation/幂等重复 → export → 隔离 re-import。导出的 portable entry 保留原 `ResolvedSkill`、source、repository ID、commit、integrity、name、description、entry/byte count，只改变请求 metadata；不存在 Trust、cache、workspace/global 或本机路径。10,000-entry release evidence满足本计划的 10 秒预算，但 `SKL-LIB-011` 仍保持 planned，直到 list/indexed search/get 和完整行为一起验证。

完成判定还要求 `cargo fmt --all --check`、Clippy `-D warnings`、locked all-features workspace tests、workspace build、`git diff --check` 全部成功，受影响产品/架构/设计状态与代码一致，本计划的 Progress、Surprises、Decision Log、Artifacts、Outcomes 和 Review Conversation Log 已更新。

## Idempotence and Recovery


所有 set/add/remove/clear 命令可安全重试。commit 前任何 validation、not-found、conflict、busy、identity 或 SQL error 保持数据库 generation 和 revision 不变；changed commit 完成但 durability sync 报错时不得声称回滚，用户重试相同目标，通过 observed state 收敛到 `unchanged` 或再次报告真实错误。unchanged 不执行 SQL write或 sync。数据库 process lock 始终在返回前显式 unlock；unlock failure覆盖原结果并返回 operational error，与 P2 现有规则一致。

本计划不做 schema migration或 destructive cleanup。测试和 smoke 只使用临时 XDG roots并在结束后删除；不对真实用户数据库运行 mutation。若实现过程中发现必须改变产品语义、schema 或增加 FTS/Trust，停止扩大本 Plan，记录 discovery，并按 `docs/PLANS.md` 重新评估 scope；不得用兼容 shim 或隐藏表绕过。

ready/review transaction 的恢复必须精确遵守 `docs/PLANS.md`。若 `gh pr ready` 失败，本计划保持 `active`。若 PR 已 ready 但 review-state commit 或 push 失败，立即运行 `gh pr ready <PR-URL> --undo`，确认 Draft，再保持或恢复 `active`。review 中发现 material scope/acceptance 缺失时，先将 PR 恢复 Draft并验证，再将 Plan 移回 `active`、记录原因、提交推送；若 active transition 发布失败，则恢复 `review` 并将 PR 恢复 ready，不能留下状态不一致。

收到明确 merge 授权后才可进入 `completed`。若 completion commit 后、GitHub 报告 `MERGED` 前任一 required check、重复 gate、merge queue 或 merge command 实际失败，先查询歧义结果；确认未合并后把 Plan 恢复到 `review`、记录失败、提交推送。只有 GitHub 明确报告 merged 后，completed Plan 在默认分支才成为正式档案，随后更新 `main` 并删除本地分支。

## Artifacts and Notes


规划基线证据：

    mise install
    mise all tools are installed

    gh auth status
    github.com: authenticated account bootids; Git protocol ssh

    git status --short --branch
    ## main...origin/main

    git rev-parse HEAD origin/main
    80c5b430aebebc7d79cc6aef78aee01a26a0904f
    80c5b430aebebc7d79cc6aef78aee01a26a0904f

`docs/exec-plans/plan/`、`active/` 和 `review/` 在基线中没有当前 Plan；`completed/` 依次包含 `PLAN-0001`、`PLAN-0002` 和 `PLAN-0003`。`gh pr list --state open` 返回空数组，fetch 后没有已有 P3 分支。本计划因此使用未占用的 `PLAN-0004` 和 `codex/p3-library-metadata-mutations`，而不是创建重复交付。

当前 `LibraryRepository` 只有以下已实现边界：

    fn export(&self) -> Result<PortableLibraryDocument, AppError>;
    fn import(
        &self,
        document: &PortableLibraryDocument,
        dry_run: bool,
    ) -> Result<LibraryImportOperation, AppError>;

首个规划提交 `20d47866f78a904099eeb6b47df6c6e9302c4415` 已推送到 `origin/codex/p3-library-metadata-mutations`。GitHub 创建 canonical Draft PR https://github.com/bootids/skilload/pull/4；本次第二个 planning metadata 提交写回该 URL 和 publication evidence，推送后完成 plan creation workflow。

## Interfaces and Dependencies


在 `crates/skilload-core/src/domain/library.rs` 中定义或等价实现以下 presentation-neutral 类型；字段名可以为 Rust 惯例作最小调整，但语义、ownership 和返回数据不得改变：

    pub enum LibraryMetadataChange {
        AliasSet(String),
        AliasClear,
        CategorySet(String),
        CategoryClear,
        TagAdd(TagValue),
        TagRemove(TagValue),
        NoteSet(String),
        NoteClear,
    }

    pub enum LibraryChangedField {
        Alias,
        Category,
        Tags,
        Note,
    }

    pub enum LibraryMutationOutcome {
        Changed,
        Unchanged,
    }

    pub enum LibraryTrustState {
        Missing,
        Revoked,
        Active,
    }

    pub struct LibraryEntry {
        pub skill: ResolvedSkill,
        pub alias: Option<String>,
        pub category: Option<String>,
        pub tags: Vec<String>,
        pub note: Option<String>,
        pub trust_state: LibraryTrustState,
    }

    pub struct LibraryMetadataMutation {
        pub selector: String,
        pub change: LibraryMetadataChange,
    }

    pub struct LibraryMetadataStoreResult {
        pub outcome: LibraryMutationOutcome,
        pub entry: PortableLibraryEntry,
        pub changed_fields: Vec<LibraryChangedField>,
    }

    pub struct LibraryMutationOperation {
        pub outcome: LibraryMutationOutcome,
        pub source: SourceIdentity,
        pub entry: LibraryEntry,
        pub changed_fields: Vec<LibraryChangedField>,
    }

`TagValue` 继续由 `domain/unicode_15_1.rs::normalize_tag` 构造；CLI 不直接构造未验证 tag。alias/category/note constructor复用一个共享 bounded-text validator。`LibraryEntry` 的 Active/Revoked variants 为稳定 API vocabulary，不代表本交付创建 Trust；本切片 application 只产生 Missing。

在 `crates/skilload-core/src/ports/library.rs` 的现有 trait 上增加：

    fn mutate_metadata(
        &self,
        mutation: &LibraryMetadataMutation,
    ) -> Result<LibraryMetadataStoreResult, AppError>;

在 `Application` 上提供 `library_alias_set`、`library_alias_clear`、`library_category_set`、`library_category_clear`、`library_tag_add`、`library_tag_remove`、`library_note_set` 和 `library_note_clear`。每个 public method 接收 logical `String` 参数、完成领域构造后调用一个 private shared mutation path；CLI 不接触 repository trait。

在 `AppError` 中加入携带 `domain` 和 logical `selector` 的 `NotFound` variant。其稳定 code 是 `not_found`，exit code 是 4；JSON 映射为 `LookupDetails` 且 `path` 必须为 null。不要把 missing entry 折叠成 Validation、InvalidState 或 SQLite error。

JSON renderer 为 mutation 生成 API-v2：

    LibraryMutationData {
      source: SourceIdentity,
      entry: LibraryEntry,
      changed_fields: ("alias" | "category" | "tags" | "note")[],
      network: NetworkUse { used: false, attempts: [] },
      source_limits: null,
      fetch_budget: null,
      cache_quota: null
    }

changed 时数组恰有对应一个字段，unchanged 时为空。Library adapter不得依赖 CLI projection；CLI renderer不得发 SQL、读取 XDG 或重算 mutation outcome。

依赖继续使用 workspace 已锁定的 `rusqlite 0.40.2` bundled SQLite、`rustix 1.1.4` filesystem capability、`unicode-normalization 0.1.23` 与仓库 Unicode 15.1.0 数据。不得新增 dependency、网络 client、FTS table、migration feature 或泛化 native-I/O abstraction。

## Plan Revision Note


2026-08-20：创建 `PLAN-0004` 规划基线，选择 P2 之后最小可独立验收的显式 Library 元数据 mutation，记录 exact scope、接口、portable closure、SQLite atomicity、CLI/API-v2 和验证要求；尚未实现运行时代码。

2026-08-20 07:18Z：写回 Draft PR https://github.com/bootids/skilload/pull/4、首个远端规划提交和 publication progress，使 Plan frontmatter、正文和 GitHub 交付关系一致；仍未进入 `active` 或修改实现。

2026-08-20 08:06Z：处理 PR #4 的完整 review conversation。根据 `SKL-CLI-010` 的 add/refresh acceptance，将该行为从本计划的完成 Product Baseline 改为 planned 约束；同时将实际 CLI smoke 改为显式 `./target/debug/skilload`、可复制临时 HOME/XDG roots、portable input 和隔离 re-import。两项均为 planning 文档修订，没有进入 `active` 或修改运行时代码。

2026-08-20 08:42Z：planning 修订 `f2dd223d38666c015bc00f7c597372067da601d0` 已推送；已向两个 inline review sources 回复具体修订与验证，并确认两个 threads 均为 resolved。Review Conversation Log 与 GitHub 结果同步，Plan 与 PR 仍保持 `plan`/Draft 状态。
