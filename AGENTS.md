## Repository Documentation Structure

```text
├── ARCHITECTURE.md
├── docs/
│   ├── PLANS.md
│   ├── design-docs/
│   ├── exec-plans/{plan,active,review,completed}/
│   ├── product-specs/
│   ├── references/
│   └── generated/
````

* `ARCHITECTURE.md`: Repository structure, module boundaries, dependency rules, and invariants.
* `docs/product-specs/`: Expected product behavior.
* `docs/design-docs/`: Technical designs and their rationale, including the technology stack and directory structure.
* `docs/references/`: Reusable external or domain knowledge.
* `docs/exec-plans/`: Execution plans for complex work.
* `docs/generated/`: Generated artifacts; do not modify them manually unless explicitly instructed.

## Toolchain (mise)

This repository's **Node.js, npm, and pnpm** must all be managed and switched through [mise](https://mise.jdx.dev/). Do not install a parallel toolchain through the system, nvm, fnm, or another mechanism.

* Versions are determined by the repository-root `mise.toml` and the `packageManager` / `engines` fields in `package.json`.
* After entering the repository, run `mise install` before using `node`, `npm`, or `pnpm`.
* Agent and automation scripts must also resolve the same binaries through mise to avoid version drift between CI and local environments.

## Documentation Precedence

When documents conflict, resolve them in this order:

1. `docs/product-specs/`
2. `ARCHITECTURE.md`
3. `docs/design-docs/`
4. `docs/exec-plans/{plan,active,review}/`
5. `docs/generated/`

If a conflict cannot be resolved unambiguously, do not silently guess.

## Facts and Decisions

* Product specifications define the user behavior and acceptance to deliver; `ARCHITECTURE.md` defines boundaries that must not be crossed; design documents explain the implementation approach and rationale; ExecPlans turn these inputs into a verifiable delivery.
* Agents must investigate the repository, toolchain, configuration, and verifiable external facts themselves. Major product tradeoffs must be confirmed by the user; only information known by external parties should be gathered through a questionnaire; low-risk implementation tradeoffs must be explained in the ExecPlan's `Decision Log`.
* Atomic product behavior that is implemented or verified must have a stable ID and revision in the product specifications. Existing unnumbered behavior must be assigned an ID in the first PR that covers it; textual clarification that does not change behavior keeps the revision, while changes to behavioral semantics increment the revision or create a replacement ID.

## ExecPlan

For complex, high-risk, multi-step, cross-module, or migration work, complete the necessary fact-finding and clarify major decisions before making the first repository implementation change, then create an ExecPlan.

Read and follow `docs/PLANS.md`.

Each complex delivery uses a separate branch, one ExecPlan, and one Draft PR; a PR may be associated with at most one ExecPlan. Simple PRs may omit a plan. Work requiring multiple PRs must be split into independently acceptable ExecPlans; preserve the overall intent across PRs in product or design documentation.

Each plan must use YAML frontmatter to record a unique Plan ID, branch, PR, status, and prerequisite plans; field and validation rules are defined in `docs/PLANS.md`, and the plan's directory must match its status.

When creating a complex delivery, use the project-level `create-exec-plan` skill: select an independently acceptable scope from the product specifications, create a branch and a `plan`-status ExecPlan, commit and push them, create a Draft PR, write back the PR URL, then commit and push again. Stop at this point and wait for a human to explicitly trigger execution in a later prompt.

After a human triggers execution, use the project-level `execute-exec-plan` skill. The skill first confirms that all prerequisite plans are `completed` on `main`, then moves the plan to `active/`, updates its status, commits and pushes, and fully implements and accepts the delivery. After all implementation code, tests, and synchronized documentation have been committed and pushed, immediately run `gh pr ready <PR-URL>` and use `gh pr view <PR-URL> --json isDraft,headRefOid` to verify `isDraft: false` and that the head is the pushed implementation commit; then move the plan to `review/`, record the ready evidence, update its status, and commit and push again. If the ready conversion or review-status commit fails, roll back according to `docs/PLANS.md`; never leave PR and Plan state inconsistent.

When handling review conversation for an ExecPlan-backed PR, use the `address-pr-threads` skill. The skill must inspect top-level conversation comments, review bodies, and inline review threads, and record every actual problem's source, assessment, resolution status, resolution method, commit/test evidence or no-fix rationale, GitHub reply, and thread closure result in the corresponding ExecPlan's `Review Conversation Log`. Every unresolved inline thread must either be fixed, replied to, and closed, or receive a reply explaining why it will not be fixed before it is closed; issues that cannot be decided remain open with an explicit blocker. For a permitted simple PR without an ExecPlan, perform the same complete inspection, remediation, validation, replies, and inline closures, but preserve outcomes in GitHub and commits rather than inventing a Plan.

Use the `merge-exec-plan` skill only when a human prompt explicitly asks to merge the current ExecPlan-backed branch or PR. First complete the read-only preflight, then move the corresponding plan to `completed/`, update its status and retrospective, commit and push. Evaluate required checks against that stable completion SHA. The explicit human merge prompt is the approval boundary; do not require or await a GitHub review approval. After required checks and every repeated gate pass, merge. After a successful merge, switch back to `main`, update the local `main`, and delete the corresponding local branch. If an actual post-completion gate, check, queue attempt, or merge fails before GitHub reports the PR merged, restore the plan to `review` and push, leaving the PR unmerged. For a permitted simple PR without an ExecPlan, run the same read-only PR, conversation, validation, and check preflight, then merge only after explicit human authorization without a Plan lifecycle transition.

Plan files are stored according to the following rules; content in `completed/` on the default branch is the sole source of truth for archived status:

* Awaiting human trigger: `docs/exec-plans/plan/`
* In progress: `docs/exec-plans/active/`
* Under review: `docs/exec-plans/review/`
* Completed: `docs/exec-plans/completed/`

## References and Knowledge Capture

Before conducting an external search, inspect `docs/references/`.

If you must consult external documentation, issues, migration guides, or examples, and the results are likely to help future work, organize the verified conclusions in `docs/references/` before completing the task.

Content to archive includes:

* Previously uncertain framework or library usage
* Version-specific behavioral differences or migration guidance
* Integration constraints
* Important API usage rules
* Common errors and their verified fixes
* Platform considerations relevant to this repository

Do not archive:

* Raw search-result dumps
* Unverified claims
* One-off debugging noise
* Duplicate notes
* Task status information that belongs in an ExecPlan

Prefer concise summaries over copying long passages.

## Reference Documentation Rules

Reference documents should be concise and easy to scan. The recommended structure is: title, scope/version, why it matters to this repository, key conclusions, cautions, sources, and last-updated date. Filenames should be clear and specific, such as `tauri-2-window-rules.md` or `postgres-jsonb-reference.md`.

## Documentation Updates Are Part of the Task

When any of the following changes, update the corresponding documentation:

* Behavior changes -> `docs/product-specs/`
* Boundaries or invariants change -> `ARCHITECTURE.md`
* Technical approach changes -> `docs/design-docs/`
* Plan or execution status changes -> `docs/exec-plans/`
* Reusable research knowledge changes -> `docs/references/`
* Generated artifacts change -> `docs/generated/`

Do not treat documentation updates as optional follow-up work.

## Work Sequence

Unless the task is very simple, proceed in this order: read relevant documentation → inspect `docs/references/` → investigate unknown facts and clarify major decisions → use `create-exec-plan` to create a `plan`-status ExecPlan, branch, and Draft PR, then stop → after an explicit human execution trigger, use `execute-exec-plan` to enter `active` → prefer mature libraries → conduct external research when necessary → preserve conclusions in `docs/references/` → implement, accept, commit, and push → mark the PR ready and verify it → automatically enter `review` → use `address-pr-threads` to handle and record review conversation → after an explicit human merge request, use `merge-exec-plan` to complete the Plan, merge, and clean up the branch.

## Final Rules

Do not repeatedly rediscover the same knowledge.

If completing a task taught you something important that will remain useful, record it in the repository.
