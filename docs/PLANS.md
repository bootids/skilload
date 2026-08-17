# Codex Execution Plans (ExecPlans)

This document defines the requirements for an execution plan ("ExecPlan"). An execution plan is a design document that a coding agent can follow to deliver a working feature or system change. Treat the reader as a **complete newcomer to this repository**: they have only the current worktree and the single ExecPlan file you provide. They have **no memory of any previous plan and no external context**.

## How to Use ExecPlans and PLANS.md

When writing an executable specification (ExecPlan), you must **strictly follow** `PLANS.md`. If it is not in your context, reread the entire `PLANS.md` file to refresh your memory. Be thorough when reading source material, rereading it as needed to produce an accurate specification. Start from the skeleton when creating a specification and flesh it out as you research. The creation phase of a complex delivery is responsible only for defining the scope, establishing the branch and Plan, opening a Draft PR, and pushing metadata; after completing these actions, it must stop in `plan` and wait for a later human prompt to explicitly trigger execution.

Only after a human explicitly asks to execute a created Plan may you invoke `execute-exec-plan`, move the Plan from `plan` to `active`, and begin implementation. Once in `active`, do not ask the user "what should I do next"; proceed directly to the next milestone. Keep every section current, and at every stopping point add or split entries that explicitly state **the progress already made** and **the next steps**. Agents must investigate the repository and verifiable facts themselves; major product tradeoffs must be confirmed by the user before implementation; low-risk implementation tradeoffs should be decided independently and recorded in the `Decision Log`. After implementation, acceptance, documentation synchronization, and retrospective are complete and have been committed and pushed, the agent must convert the Draft PR to ready for review, verify that the conversion succeeded, and then automatically advance the Plan to `review` without waiting for another prompt.

When discussing an executable specification (ExecPlan), record decisions in the specification's logs so they can be traced later; it must be unambiguous why a particular change was made to the specification. An ExecPlan is a **living document**, and it must always support **restarting implementation using the ExecPlan alone, without relying on any other work**.

When the design requirements you are investigating are complex and contain many unknowns, you may use milestones to implement proofs of concept, toy implementations, and similar work to verify whether the user's proposed approach is feasible. Research deeply, including reading the source code of relevant libraries by locating or obtaining them, and incorporate prototype results into the Plan to guide the fuller implementation that follows.

## Requirements

**Non-negotiable requirements:**

* Every ExecPlan must be **fully self-contained**. Self-contained means that, in its current form, it contains all the knowledge and instructions a newcomer needs to complete the task.
* Every ExecPlan is a **living document**. Contributors must continue revising it as progress is made, new facts are discovered, and design decisions are finalized. After every revision, the document must remain fully self-contained.
* Every ExecPlan must enable a **complete newcomer to the repository** to implement the feature from beginning to end.
* Every ExecPlan must lead to **working behavior that can be demonstrated**, not merely code changes that "meet a definition."
* Every term used in an ExecPlan must be explained in **plain language**; if you cannot explain it, do not use it.
* Every ExecPlan involving product behavior must record a `Product Baseline`: atomic behavior ID, revision, product specification source, and observable acceptance. Design and architecture documents may provide only approaches and constraints; they cannot replace product behavior or acceptance.
* Every complex delivery must be associated with a separate branch and one Draft PR; a PR may be associated with at most one ExecPlan. Work spanning multiple PRs must be split into multiple independently acceptable Plans.
* Every ExecPlan must use YAML frontmatter to record `plan_id`, `branch`, `pull_request`, `status`, and `depends_on`, and `status` must match the parent directory.
* The normal status path is `plan → active → review → completed`; both `plan → active` and `review → completed` require explicit authorization in a later human prompt. Only scope rework and merge-failure recovery specified in this document may move backward.
* Every ExecPlan that enters `review` must maintain a `Review Conversation Log`. Every problem raised in PR conversation must record its source, whether it was resolved, how it was resolved, and the evidence; the record must agree with GitHub replies and thread state.

Begin by explaining the purpose and intent. In a few opening sentences, explain why the work matters, what a user will be able to do after completion that they could not do before, and how they can see that it works. Then guide the reader through the exact steps required to achieve that result, including what to edit, what to run, and what to observe.

The agent executing a Plan can list directories, read files, search, run the project, and run tests. It **knows no prior context** and cannot infer what you mean from earlier milestones. Repeat every premise on which you rely. Do not direct the reader to external blogs or documentation; if knowledge is necessary, state it directly in the Plan in your own words. If an ExecPlan builds on another ExecPlan that has been committed to the repository, it may incorporate that Plan by reference; if the other Plan has not been committed, include all of its relevant context in the current document.

## Metadata, Deliveries, Branches, and PRs

An ExecPlan records one reviewable delivery, not a project roadmap spanning multiple PRs. Simple PRs may omit an ExecPlan, but a PR associated with an ExecPlan cannot carry a second Plan. Every Plan file must begin with the following YAML frontmatter, which is the sole source of truth for mutable delivery metadata:

    ---
    plan_id: <stable-and-unique-plan-id>
    branch: <delivery-branch>
    pull_request: pending
    status: plan
    depends_on: []
    ---

A permitted simple PR without an ExecPlan is outside this lifecycle and does not use the ExecPlan lifecycle skills or status directories. It must still receive a complete read of top-level comments, submitted reviews, and inline threads; every problem must be fixed or justified, answered, and closed when closable; validation and required checks must pass; and merging still requires explicit human authorization. Preserve those outcomes in GitHub and commits rather than creating a retrospective Plan.

Once created, `plan_id` must never change and must be globally unique across all four status directories. `branch` must equal the PR's head branch. `pull_request` may temporarily be `pending` only between the initial push and creation of the Draft PR; it must then be changed to the PR's canonical HTTPS URL and committed and pushed again. `status` may only be `plan`, `active`, `review`, or `completed`, and the file must be in the directory with the corresponding name. `depends_on` must be a YAML list of Plan IDs; use `[]` when there are no prerequisite Plans, and do not use `none` or omit the field. A dependency is satisfied only when the target Plan ID exists with `completed` status on the default branch.

The `Delivery Metadata` body section explains only necessary context and exceptions; it does not repeat a separately editable list of fields. Product behavior IDs and revisions remain in `Product Baseline` because they are an acceptance baseline, not delivery-routing metadata.

The delivery lifecycle is:

1. `plan`: Create a separate branch from the updated default branch and write a self-contained Plan in `docs/exec-plans/plan/`. Commit and push it first with `pull_request: pending`, open a Draft PR, then write back the URL, commit, and push. The creation workflow ends here and must not begin implementation along the way.
2. `active`: After receiving a later human execution prompt, use `execute-exec-plan` to verify from the default branch that every entry in `depends_on` is `completed` and that the open Pull Request is Draft. If it is ready, return it to Draft with `gh pr ready <PR-URL> --undo` and verify `isDraft: true` before changing Plan state. Then move the file to `docs/exec-plans/active/`, change the status to `active`, update `Progress`, and commit and push. Plan implementation may occur only in this status and the Pull Request must remain Draft.
3. `review`: After implementation, acceptance, documentation synchronization, and `Outcomes & Retrospective` are all complete, first commit and push all code, tests, documentation, and active Plan updates. Immediately run `gh pr ready <PR-URL>`, then run `gh pr view <PR-URL> --json isDraft,headRefOid`; you must observe `isDraft: false` and a `headRefOid` equal to the pushed implementation HEAD. Only after this verification passes may you move the file to `docs/exec-plans/review/`, change its status to `review`, record the ready evidence, and commit and push again. Ordinary review fixes remain in `review`; return to `active` only when scope or acceptance is found to be materially incomplete.
4. `completed`: Enter this status only after a human prompt explicitly asks to merge the current branch or PR. First use read-only checks to confirm the worktree, PR, review threads, mergeability, validation evidence, and branch scope. Then move the Plan to `docs/exec-plans/completed/`, change its status to `completed`, finish the final retrospective, and commit and push. Capture that completion SHA and wait only for required checks with `gh pr checks <PR-URL> --watch --required --fail-fast` or the repository-equivalent command. Reconcile a fresh full conversation read, including top-level comments and review bodies, and require the PR head to remain the completion SHA. The explicit human merge prompt is the final authorization; do not require or await a GitHub review approval. Once required checks and every mutable gate pass, pass the stable SHA to `gh pr merge --match-head-commit`. A merge-queue-compatible command must not request branch deletion. If GitHub queues the Pull Request, wait until it reports `MERGED`; only then switch to the default branch, fast-forward it, and delete the local delivery branch. The completion commit on the PR branch is only a declaration awaiting merge; it becomes the official archive only after entering the default branch.

If `gh pr ready` fails, keep the Plan `active` and retry or record the blocker. If the PR has been made ready but the review Plan commit or push fails, run `gh pr ready <PR-URL> --undo` to restore Draft state, confirm `isDraft: true`, then keep or restore the Plan to `active` before retrying. This keeps the ready PR and `review` Plan as a single finalization transaction.

Material scope or acceptance rework discovered during review is the reverse transaction. First run `gh pr ready <PR-URL> --undo` and verify `isDraft: true`; then move the Plan from `review/` to `active/`, set `status: active`, record the reason and remaining work, commit, and push. Stop review handling and resume only through `execute-exec-plan`; after that skill completes the normal ready/review transaction, rerun `address-pr-threads`. If publishing the active transition fails after the PR becomes Draft, restore the Plan to `review`, mark the PR ready again, verify both states, and retry without leaving them inconsistent.

When handling review conversation, `address-pr-threads` must read top-level conversation comments, submitted review bodies, and inline review threads, and extract every actual problem from them. Record every problem in the corresponding Plan's `Review Conversation Log` under a stable source ID or URL, including the problem summary, disposition, `open`/`resolved`/`blocked` status, `fixed`/`no-fix`/`pending` handling, concrete resolution process, commit and validation evidence, GitHub reply URL, and final resolved state of any inline thread. When a top-level comment or review body has no closable state, record "replied; no resolvable thread." Multiple sources for the same problem may share an implementation, but every source must remain traceable. Commit and push the Plan record with the fix first; after GitHub replies and closures are complete, write back the final URLs and statuses, then commit and push again. Merge preflight must confirm that current GitHub conversation matches this log and contains no unrecorded problem or status drift.

If a required check, a rejected or changed repeated PR or conversation gate, a merge-queue attempt, or a merge command fails after the completion commit but before GitHub reports `MERGED`, do not leave the PR in a falsely completed state. Query ambiguous merge results first; while the PR remains unmerged after an actual failure, move the Plan back to `review/`, restore `status: review`, record the failure reason, commit and push. A PR that is closed without being merged likewise must not produce a completed Plan on the default branch. If a merged PR is reverted, keep the original Plan as historical evidence of the delivery at that time and repair the affected behavior with a new ExecPlan and PR. If the work exceeds one reviewable delivery, split the Plan and PR before implementation and preserve the overall goal in product or design documentation.

Before toolchain and CI support exist, the PR template and human review must check the Plan ID, frontmatter/directory consistency, PR association, dependency status, `Product Baseline`, acceptance evidence, consistency between `Review Conversation Log` and GitHub conversation, checks, and final file path. Future CI and branch protection must enforce the same conditions, rejecting a complex delivery when one PR is associated with multiple Plans, required metadata is missing, status and directory disagree, dependencies are incomplete, conversation is unrecorded, or acceptance is incomplete.

## Product Baseline, Design Inputs, and Behavior Changes

Product specifications define the user behavior and acceptance to deliver; `ARCHITECTURE.md` defines invariants and dependency boundaries; design documents explain implementation approaches and rationale. An ExecPlan must select the atomic product behaviors covered by the delivery from the product specifications and record their stable IDs and revisions in `Product Baseline`. Existing behavior without an ID must receive one in the first PR that implements or verifies it.

If implementation reveals that code disagrees with unchanged product behavior, that is an implementation defect; fix the code or tests rather than changing the product specification to accommodate the implementation. When product behavior changes intentionally, update the product specification first; synchronize architecture or design documentation when boundaries or the technical approach are involved; then update the `active` ExecPlan's baseline, acceptance, and Decision Log. Keep `completed` Plans at their original baseline and evidence; do not rewrite them to describe new behavior. Create a new ExecPlan and PR to cover the new revision or replacement ID.

Clarifying wording without changing observable behavior may retain the same revision. Increment the revision when behavioral semantics change; create a new stable ID when behavior is added, split, or replaced, and record its relationship to the old behavior. Validation evidence proves only the behavior revision it records and does not automatically prove that a later revision has been implemented.

## Format

Formatting and wrapper requirements are simple but strict. Every ExecPlan must be a **single** fenced code block labeled `md`, beginning and ending with three backticks. Do not nest additional triple-backtick code blocks inside it; when you need to show commands, terminal output, diffs, or code, use **indented blocks** within the single code block. To avoid accidentally ending the ExecPlan's code block early, prefer indentation for clarity instead of using another triple-backtick block inside it. Leave two blank lines after every heading, use correct Markdown heading syntax such as `#` and `##`, and use correct ordered and unordered list syntax.

If you are writing an ExecPlan into a Markdown (`.md`) file whose contents are **only that ExecPlan**, you may omit the outer triple backticks.

Use plain written prose. Prefer sentences to lists. Avoid checklists, tables, and long enumerations unless the text would otherwise become unclear. **Checklists are permitted and required only in the `Progress` section.** Narrative sections should primarily use prose.

## Guidelines

Self-containment and plain language are the most important qualities. If you introduce a term that is not ordinary English, such as "daemon," "middleware," "RPC gateway," or "filter graph," immediately explain what it means and how it appears in this repository, for example by naming the relevant file or command. Do not write "as described above" or "according to the architecture document"; repeat the necessary explanation here even if it creates duplication.

Avoid common failure modes. Do not rely on undefined terms. Do not describe the literal requirements of a feature so narrowly that the final code compiles but has no genuinely meaningful effect. Do not outsource key decisions to the reader. Resolve ambiguities in the Plan and explain the rationale. Prefer additional explanation of user-visible effects over rigidly specifying incidental implementation details.

Plans must be anchored in **observable results**. State explicitly what users can do after implementation, what commands to run, and what output they should see. Acceptance criteria should describe **human-verifiable behavior**, such as "after starting the service, visiting `http://localhost:8080/health` returns HTTP 200 with a response body of `OK`" rather than "added a `HealthCheck` struct." If the change is internal, explain how to prove its effect, for example by running a test that fails before the change and passes afterward or by showing a concrete scenario that uses the behavior.

Describe the repository context explicitly. Name files using complete paths relative to the repository root, identify functions and modules precisely, and state where new files should be created. If multiple areas are involved, begin with a short orientation explaining how they work together so a newcomer can navigate confidently. When running commands, state the working directory and the complete command line. If the result depends on the environment, explain the assumptions and provide alternatives where reasonable.

Ensure that steps are **idempotent and safe**. They should be repeatable without causing damage or state drift. If a step can fail partway through, explain how to retry or adapt it. If a migration or other destructive operation is required, describe backup or safe rollback procedures. Prefer incremental, testable changes.

Validation is not optional. Include instructions to run tests, start the system when applicable, and observe useful behavior. For every new feature or capability, explain how to test it completely. Provide expected output and error messages so a newcomer can distinguish success from failure. Whenever possible, explain how to prove more than "it compiles," for example through a small end-to-end scenario, a CLI invocation, or an HTTP request/response transcript. Give the exact test commands appropriate to the project's toolchain and explain how to interpret the results.

Preserve evidence. When a step produces terminal output, a short diff, or a log, include it as an **indented example** within the same fenced block. Keep it brief and retain only what is needed to prove success. If a patch is needed, prefer small per-file diffs or excerpts that readers can reproduce by following the steps instead of pasting a large block.

## Milestones

Milestones should be narrative, not bureaucratic process. If you divide the work into milestones, introduce each milestone with a short paragraph describing its scope, what will exist at its end that did not exist before, what commands to run, and what acceptance result should be observed. Write it as a process story: goal, work, result, evidence. Progress and milestones are different: milestones tell the overall delivery story, while progress tracks work at a finer granularity. Both must be present. Do not over-compress milestones for brevity or omit details that may be crucial to future implementation.

Every milestone must be independently verifiable and should advance the ExecPlan's final goal incrementally.

## Living Documents and Design Decisions

* An ExecPlan is a **living document**. When you make a key design decision, update the Plan with the decision itself and the reasoning behind it. Record every decision in the `Decision Log` section.
* New ExecPlans and every `plan`, `active`, or `review` ExecPlan must include and continuously maintain `Progress`, `Surprises & Discoveries`, `Decision Log`, `Outcomes & Retrospective`, and `Review Conversation Log`. `completed` Plans archived before this rule was established do not need GitHub conversation reconstructed retroactively; preserve their historical evidence as-is.
* When you discover optimizer behavior, performance tradeoffs, unexpected bugs, or inversion/rollback semantics that affect the implementation path, record these observations in `Surprises & Discoveries` with a short evidence excerpt, ideally test output.
* If you change direction during implementation, record the reason in `Decision Log` and reflect the effect in `Progress`. The Plan is both a checklist for the current implementer and a guide for the next contributor.
* When a major task or the entire Plan is completed, write an `Outcomes & Retrospective` entry summarizing what was accomplished, what remains, and lessons learned.
* Before pausing or handing off, update the `active` or `review` ExecPlan's `Progress`, `Surprises & Discoveries`, `Decision Log`, and validation results; update PR, branch, and status only in YAML frontmatter. A temporary handoff may add only the current session's next step, uncommitted worktree state, and suggested tools; it should reference existing Plans, specifications, commits, and diffs rather than copying them, and it must not contain sensitive information.

# Prototyping Milestones and Parallel Implementations

When a prototype can reduce the risk of a large change, explicitly adding a prototyping milestone is allowed and often encouraged. Examples include adding a low-level operation to a dependency to validate feasibility, or experimenting with two composition orders and measuring optimizer effects. Prototypes should be incremental and testable. Clearly label the scope as "prototype validation"; explain how to run it and observe the result; and state the criteria for promoting the prototype into the full implementation or discarding it.

For large migrations, prefer "add incrementally, remove safely later, and keep tests passing throughout." Parallel implementations, such as keeping a new adapter alongside the old path during migration, are acceptable when they reduce risk or keep tests passing during the transition. Explain how to validate both paths and how to retire one safely under test coverage. If the work involves multiple new libraries or functional areas, consider creating separate spikes, or experimental implementations, to verify independently that each external library provides the required capability and can satisfy the need on its own.

## Skeleton of a Good ExecPlan

```
---
plan_id: <stable-and-unique-plan-id>
branch: <delivery-branch>
pull_request: pending
status: plan
depends_on: []
---

# <Short, action-oriented description>

This ExecPlan is a living document. As work proceeds, the `Progress`, `Surprises & Discoveries`, `Decision Log`, `Outcomes & Retrospective`, and `Review Conversation Log` sections must be kept current.

If the repository contains a `PLANS.md` file, state its path relative to the repository root here and note that this document must be maintained in accordance with `PLANS.md`.

## Delivery Metadata

Explain the necessary context for the delivery relationships in frontmatter without duplicating a separately drifting field list in the body. After the first push and creation of the Draft PR, change `pull_request: pending` to the canonical PR URL, then commit and push again.

## Product Baseline

List the atomic product behavior IDs, revisions, product specification paths, and behavioral acceptance covered by this delivery. Explain how the Plan will prove that the current revision is implemented. If the delivery does not change product behavior, state why and define the applicable boundary.

## Design and Architecture Inputs

Describe the design-document decisions, `ARCHITECTURE.md` invariants, and dependency boundaries used by the Plan. Product behavior and acceptance must be defined by `Product Baseline` and cannot be replaced by this section.

## Purpose / Big Picture

In a few sentences, explain what users gain when this change is complete and how they can see that it works. State the user-visible behavior you will enable.

## Progress

Use a checklist to summarize fine-grained steps. Record every stopping point here, even if that requires splitting a partially completed task into "done" and "remaining" entries. This section must always reflect the true current state of the work.

- [x] (2025-10-01 13:00Z) Created the branch, `plan`-status ExecPlan, and Draft PR; wrote back the URL and pushed; awaiting a human execution trigger.
- [ ] After receiving a human execution prompt, enter `active` and complete the implementation milestones.
- [ ] Partially completed step example (done: X; remaining: Y).
- [ ] After implementation, acceptance, documentation, and retrospective are all committed and pushed, run `gh pr ready`, verify `isDraft: false`, then automatically enter `review` and push the status commit.
- [ ] After receiving a human merge prompt, pass preflight, complete and push the Plan, merge the PR, return to the updated default branch, and delete the local delivery branch.

Use timestamps to measure progress.

## Surprises & Discoveries

Record unexpected behavior, defects, optimization opportunities, or insights discovered during implementation. Provide concise evidence.

- Observation: ...
  Evidence: ...

## Decision Log

Record every decision made during Plan execution in this format:

- Decision: ...
  Rationale: ...
  Date/Author: ...

## Outcomes & Retrospective

At major milestone completion or overall completion, summarize results, gaps, and lessons learned. Compare the final result with the original goal. On entering `review`, record the associated PR, acceptance evidence, and `Product Baseline` revision; before merging, complete the final review results and state that the completion commit becomes the official archive only after entering the default branch.

## Review Conversation Log

When no PR conversation has been processed, explicitly write "No review conversation has been processed." Afterward, create a small heading for each problem and write concise paragraphs using these fixed fields: `Source` (comment/review/thread ID and URL), `Problem`, `Disposition` (`fixed`, `no-fix`, or `pending`), `Status` (`open`, `resolved`, or `blocked`), `Resolution`, `Evidence` (commit and validation), and `GitHub outcome` (reply URL and whether the thread is resolved). Do not leave these facts only in chat or GitHub.

## Context and Orientation

Assume the reader knows nothing and describe the current state relevant to this task. Name key files and modules using complete relative paths. Define every non-obvious term you will use. Do not refer to previous Plans.

## Plan of Work

Describe in prose the sequence of changes and additions. For each change, name the file, location (function or module), and content to insert or modify. Be specific and concise.

## Concrete Steps

State the exact commands to run and the working directory for each. When a command produces output, provide a short expected-output example for comparison. Keep this section synchronized as work progresses.

## Validation and Acceptance

Explain how to start the system or trigger the relevant behavior and what to observe. Express acceptance criteria as behavior with explicit inputs and outputs. If tests are involved, write something like: "Run <the project's test command>; expect <N> tests to pass; new test <name> fails before the change and passes afterward."

## Idempotence and Recovery

State which steps can be repeated safely. If a step carries risk, provide a safe retry or rollback path. Explain how to use `gh pr ready --undo` to restore Draft/active consistency if `gh pr ready` or the review-status push fails, how to perform and recover the review-to-active rework transaction, and how to restore the Plan to `review` and push if any post-completion gate, check, queue attempt, or merge fails before GitHub reports `MERGED`. Leave the environment clean when the task is complete.

## Artifacts and Notes

Include the most important terminal transcripts, diffs, or code excerpts as indented examples. Keep them concise and retain only what is needed to prove success.

## Interfaces and Dependencies

Be prescriptive. State which libraries, modules, and services must be used and why. Specify the types, traits/interfaces, and function signatures that must exist at the end of the milestone. Prefer stable, explicit names and paths such as `crate::module::function` or `package.submodule.Interface`. For example:

In `crates/foo/planner.rs`, define:

    pub trait Planner {
        fn plan(&self, observed: &Observed) -> Vec<Action>;
    }
```

If you follow this guidance, a **single-run, stateless** agent, or a human newcomer, can read the ExecPlan from beginning to end and produce a working implementation with observable results. That is the standard: **self-contained, self-sufficient, newcomer-guiding, and outcome-oriented**.

When revising a Plan, ensure that the changes are reflected comprehensively throughout every section, including the living-document sections; add a note at the bottom of the Plan describing what changed and why. An ExecPlan should explain not only what to do, but also why wherever possible.
