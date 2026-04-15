---
name: pr-review
description: Respond to review comments on a ferrish pull request — fetches all unresolved comments, classifies them, implements mechanical fixes, runs tests and clippy, pushes, and replies to threads via MCP. Surfaces design questions and judgment calls to the user. Use whenever asked to address, respond to, or fix review comments on a PR.
---

# PR Review Response

Addresses review comments on a single ferrish PR end-to-end. Mechanical fixes get implemented and pushed. Design questions get a reasoned reply. Anything requiring human judgment gets surfaced to you.

## Repo constants

- Owner: `cdprice02`, Repo: `ferrish`, Base branch: `main`
- Worktrees: `.claude/worktrees/` (gitignored)

## Tool priority

Use `mcp__plugin_github_github__*` for all GitHub reads and writes (fetching PR details, reading review comments, posting replies). Fall back to `gh` CLI via Bash only when a specific operation is unavailable through the MCP.

## Steps

### 1. Fetch the PR and its review comments

Use `mcp__plugin_github_github__pull_request_read`:
- `get` — PR metadata, current branch name, head SHA
- `get_review_comments` — all inline review threads; each thread includes `is_resolved` — skip threads where `is_resolved: true`
- `get_reviews` — top-level review summaries (approved, changes requested, commented)

Focus on unresolved threads (`is_resolved: false`). Skip threads that are already resolved or where the author has already replied with a fix.

### 2. Classify each comment

| Class | Criteria | Action |
|-------|----------|--------|
| **Mechanical** | Naming, formatting, missing test, trivial refactor, typo | Fix in code, push, reply |
| **Design question** | Alternative approach, architectural concern, "why not X?" | Reply with reasoning via MCP; update code if the reviewer's point is valid |
| **Needs human judgment** | Product decision, security concern, scope change, contradicts a prior decision | Surface to user with full comment text |

When in doubt between mechanical and design, treat it as design — don't silently change semantics.

### 3. Set up the worktree

Check out the PR branch in an isolated worktree:

```bash
git worktree add .claude/worktrees/pr-<N>-review -b pr-<N>-review --track origin/<branch-name>
```

All edits and verification commands run inside that worktree.

### 4. Implement mechanical fixes

Read the relevant source files first. Apply all mechanical fixes in one pass. Follow ferrish conventions:
- File-based modules only — no `mod.rs`
- All I/O through `ShellIo`; `MockIo` in tests
- Integration tests via `ShellTest` builder in `tests/harness.rs`

### 5. Verify

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Fix anything that fails before pushing. Do not push a red build.

### 6. Commit and push

One commit for all mechanical fixes:
```
fix: address review comments on PR #N
```

```bash
git push
```

### 7. Reply to addressed threads

For each resolved mechanical comment, reply via `mcp__plugin_github_github__add_reply_to_pull_request_comment` with a one-line summary:
> Fixed — <what changed in one sentence>

For design questions where the original approach is sound:
> Keeping the current approach because <reason>. <Optional: what would need to change for the alternative to be preferable.>

For design questions where the reviewer's point is valid and code was updated:
> Good call — updated in <commit SHA short>. <One sentence on what changed.>

### 8. Surface judgment calls

For each `Needs human judgment` comment, present to the user:

```
PR #N — comment by @<reviewer> on <file>:<line>
"<comment text>"

This needs your input because: <reason — product decision / security concern / scope question>
```

Wait for direction before taking any action on these.

### 9. Report

Return a summary:
```
PR #N review response complete.
- Mechanical fixes: N (pushed, threads replied)
- Design replies: M (no code change)
- Awaiting your input: K (listed above)
```
