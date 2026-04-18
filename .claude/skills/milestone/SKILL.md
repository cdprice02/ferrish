---
name: milestone
description: Work through all open issues in a ferrish milestone over time — lists issues, identifies dependencies, spawns parallel subagents to implement independent issues using the implement skill, monitors CI and review status, and iterates until the milestone is clear. Use this when asked to burn down a milestone, work through a milestone, or implement all issues in a milestone. Escalates only genuine ambiguity.
---

# Milestone Burndown

Orchestrates end-to-end resolution of all open issues in a milestone. You are the supervisor — you dispatch work, monitor outcomes, and keep moving. You only surface things to the user when a decision genuinely requires human judgment.

## Repo constants

- Owner: `cdprice02`, Repo: `ferrish`, Base branch: `main`

## Recommended workflow

Run `/triage` before starting a milestone burndown to ensure all issues are labeled, scoped to the right milestone, and free of unresolved duplicates. Milestone assumes issues are already organized — it does not re-triage mid-run.

## Tool priority

Use `mcp__plugin_github_github__*` for all GitHub operations (issue listing, PR status, CI checks). Fall back to `gh` CLI via Bash only when a specific operation is unavailable through the MCP.

## Phase 1: Survey the milestone

Use `mcp__plugin_github_github__list_issues` with a milestone filter, or `mcp__plugin_github_github__search_issues` with query:
```
repo:cdprice02/ferrish is:open is:issue milestone:"<milestone title>"
```

List every open issue. For each one note: title, labels, any mentioned dependencies in the body (e.g., "depends on #N", "blocked by #N", "after #N").

Build a simple dependency map. Issues with no blockers are the first wave; issues that depend on others wait until their blockers have merged PRs.

## Phase 2: First wave — implement in parallel

For all unblocked issues, spawn one subagent per issue using the `implement` skill. Spawn them all in the same turn so they run concurrently.

Brief each subagent with:
```
Use the implement skill to implement ferrish issue #<N>.
Work in .claude/worktrees/issue-<N>.
Deliver a PR URL. Do not return a plan.
```

## Phase 3: Monitor and iterate

After the first wave completes, check the status of every PR using `mcp__plugin_github_github__pull_request_read` (methods: `get`, `get_check_runs`, `get_review_comments`, `get_reviews`).

For each PR:

| State | Action |
|-------|--------|
| CI pending / running | Wait; revisit next cycle |
| CI failing | Read failure output via MCP, fix in the worktree, push again |
| Review comments present (unresolved) | Spawn a subagent using the `pr-review` skill — see below |
| Merge conflict | Rebase the branch onto main, re-verify, push |
| CI passing, review pending | Leave idle — awaiting code owner review. Do not merge. |
| CI passing, review approved | Surface to user: "PR #N has been approved and is ready for you to merge." |

### Handling review comments

When a PR has unresolved review comments, delegate to the `pr-review` skill rather than handling comments inline. Brief the subagent:
```
Use the pr-review skill to respond to review comments on PR #<N>.
```

The `pr-review` skill handles classification, mechanical fixes, and the design-question dialogue with the user. Resume monitoring this PR after the subagent completes. If the volume of design questions requires extended back-and-forth with the user, pause the monitoring loop and let `/pr-review <N>` run interactively before resuming.

## Phase 4: Unblock the next wave

Once a blocking issue's PR is merged, check the dependency map for issues that were waiting on it. Add them to the active work queue and spawn their subagents.

Use `mcp__plugin_github_github__list_issues` to re-query the milestone periodically and confirm what's still open.

## Phase 5: Done

When all remaining open PRs are green and awaiting review, stop and report:

```
All implementation work is complete. The following PRs are green and awaiting code owner review:
- PR #N — <title> — <url>
- PR #M — <title> — <url>

Merge these at your discretion once approved. Issues will close automatically via the "Closes #N" link.
```

Do not poll indefinitely after reaching this state.

When all issues in the milestone have closed PRs (or explicit escalations), report a final summary:
- Issues implemented: list with PR URLs
- Issues escalated: list with reason
- Issues still open: list with current blocker

## Escalation criteria

Stop and surface to the user when:
- An issue's scope is genuinely unclear after reading the body and all comments
- A PR has been through two fix cycles and CI is still failing in a way you can't diagnose
- A reviewer requests a design change that would affect other issues in the milestone
- Two issues in the same milestone modify the same core module in conflicting ways

Everything else — test failures, clippy warnings, merge conflicts, mechanical review comments — handle without interrupting.
