---
name: implement
description: Implement a single ferrish GitHub issue end-to-end — reads the issue, creates an isolated worktree, writes code and tests following ferrish conventions, verifies with cargo test and clippy, commits, pushes, and opens a PR. Use this whenever asked to implement, fix, or work on a specific issue number. Always produces a PR URL, never a plan.
---

# Implement

End-to-end implementation of a single ferrish issue. The only output that matters is a PR URL — not a plan, not a summary of what could be done.

## Repo constants

- Owner: `cdprice02`, Repo: `ferrish`, Base branch: `main`
- Worktrees: `.claude/worktrees/` (gitignored)

## Steps

### 1. Read the issue

Use `mcp__plugin_github_github__issue_read` (method: `get`) for the issue body and labels. If the issue has comments, fetch them too (`get_comments`) — they often contain scope clarifications or prior decisions that affect the implementation.

If the issue is genuinely ambiguous after reading it, ask one focused question before starting. Don't guess and implement the wrong thing.

### 2. Plan the branch name

- Prefix by label: `fix/` for bug, `feat/` for enhancement, `chore/` for docs/infra/refactor
- Format: `<prefix>/issue-<N>-<slug>` where slug is 2–4 kebab-case words from the title
- Example: `feat/issue-47-help-version-flags`

### 3. Create the worktree

```bash
git worktree add .claude/worktrees/issue-<N> -b <branch-name>
```

All subsequent edits and commands happen inside that worktree directory.

### 4. Implement

Read the relevant source files before editing — understand the existing pattern before adding to it.

Key conventions:
- File-based modules only — no `mod.rs`
- Errors: `thiserror` derives in `src/error.rs`; `anyhow` for `Shell::run()` return type
- All I/O goes through `ShellIo` — never print or read stdin directly; use `MockIo` in tests
- New builtins belong in `src/command/builtin.rs`; register them in the parser
- For anything user-visible, write an integration test using the `ShellTest` builder in `tests/harness.rs`
- Tests that exercise internal logic belong as `#[cfg(test)]` modules in the relevant source file

### 5. Verify

Run both of these from the worktree root and fix anything that fails before moving on:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Do not push a failing build. Clippy warnings are CI failures — `-D warnings` is enforced.

One important constraint on tests: never spawn the shell binary as a subprocess in tests. `cargo-tarpaulin` cannot trace across process boundaries, and `--follow-exec` is blocked by seccomp in GitHub Actions. Call library code directly via `ShellTest` or unit test the function in-process.

### 6. Commit

One conventional commit per logical change:

| Prefix | When |
|--------|------|
| `feat:` | new capability |
| `fix:` | bug correction |
| `refactor:` | restructuring without behavior change |
| `chore:` | deps, CI, tooling |
| `docs:` | documentation only |

Keep the diff minimal — don't clean up surrounding code unless the issue asks for it.

### 7. Push and open PR

```bash
git push -u origin <branch-name>
```

Then open the PR via `mcp__plugin_github_github__create_pull_request`:
- `title`: mirrors the issue title, under 70 characters
- `body`: one-paragraph description of the approach, followed by `Closes #<N>`
- `base`: `main`

### 8. Done

Return the PR URL. That's the deliverable.
