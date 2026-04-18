---
name: pr-review
description: Respond to review comments on a ferrish pull request — fetches all unresolved comments, classifies them, implements mechanical fixes, runs tests and clippy, pushes, and replies to threads via MCP. Surfaces design questions and judgment calls to the user. Use whenever asked to address, respond to, or fix review comments on a PR.
---

# PR Review Response

Addresses review comments on a single ferrish PR end-to-end. Mechanical fixes get implemented and pushed. Design questions are surfaced to you with options before any reply is posted. Anything requiring human judgment gets surfaced to you.

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
| **Design question** | Alternative approach, architectural concern, "why not X?" | Surface to user with options — see Step 2.5 |
| **Needs human judgment** | Product decision, security concern, scope change, contradicts a prior decision | Surface to user with full comment text — see Step 8 |

When in doubt between mechanical and design, treat it as design — don't silently change semantics.

### 2.5 Surface design questions to user before acting

For each design question, present it to the user before posting any reply or changing any code:

```
PR #N — design question from @<reviewer> on <file>:<line>
"<comment text>"

My read: <one sentence on what the reviewer is asking and why it matters>

Options:
A. <approach — e.g., keep current approach> — <one-sentence rationale>
B. <alternative approach> — <one-sentence rationale>
[C. ...if a third option exists]

How would you like to respond to the reviewer?
  1. Decide now — tell me which option (A/B/C) and I'll implement + reply
  2. Open the question to the reviewer — I'll post the options above as a comment and leave the thread unresolved for further discussion
```

Wait for the user's direction before writing any code or posting any comment for this thread.

Once the user decides:
- **Option 1 chosen**: implement the chosen approach, push, then reply and resolve the thread per Step 7
- **Option 2 chosen**: post the options summary as a PR comment via `mcp__plugin_github_github__add_reply_to_pull_request_comment`, leave the thread unresolved, and note it in the final report as "awaiting reviewer input"

Handle all design questions this way before proceeding to mechanical fixes in Steps 3–6.

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
git push origin HEAD:<branch-name>
```

Use the PR's upstream branch name (from `get` in step 1) as `<branch-name>`. This is explicit and works correctly regardless of `push.default` when the local worktree branch name differs from the upstream.

### 7. Reply to addressed threads and resolve them

For each resolved mechanical comment, reply via `mcp__plugin_github_github__add_reply_to_pull_request_comment` with a one-line summary that includes the commit link:
> Fixed in <commit SHA short> — <what changed in one sentence>

GitHub auto-links short SHAs in PR comments. Use the short SHA of the commit you just pushed (first 7 characters).

Then resolve the thread. The thread's `node_id` is returned by `get_review_comments` — use it with the GraphQL API:

```bash
gh api graphql -f query='mutation { resolveReviewThread(input: {threadId: "<thread node_id>"}) { thread { isResolved } } }'
```

For design questions where the user chose an approach and it was implemented, reply and resolve:
> Went with <approach> — <one sentence on what changed>. <commit SHA short>

**Do not resolve** threads where Option 2 was chosen (open question to reviewer), `Needs human judgment` items, or threads explicitly inviting further discussion.

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
- Mechanical fixes: N (pushed, threads replied and resolved)
- Design decisions implemented: M (user chose approach, pushed, threads resolved)
- Awaiting reviewer input: K (options posted to PR, threads left open)
- Awaiting your input: J (listed above)
```
