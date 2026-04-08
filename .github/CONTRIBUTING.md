# Contributing to ferrish

## Branch Naming

Branches must follow the pattern `type/issue-N-short-description`:

- `fix/issue-52-readme-banners`
- `refactor/issue-12-type-command`
- `feat/issue-7-pipeline-support`

Valid types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`.

## PR Workflow

- Squash merge only; each PR addresses one concern.
- Reference the issue in the PR body with `closes #N`.
- Keep diffs minimal and focused. Avoid unrelated cleanup in the same PR.

## Tests

All changes must pass:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Prefer integration tests via the `ShellTest` harness in `tests/harness.rs` for anything user-visible. Unit tests are appropriate for internal logic that is difficult to exercise from the outside.

## Lint

`cargo clippy -- -D warnings` is enforced by CI. No warnings are allowed. Fix warnings before opening a PR rather than suppressing them with `#[allow(...)]` unless suppression is genuinely justified.

## Core Principles

These principles guide all design and implementation decisions:

- **Correctness over performance** — when a trade-off exists, correctness wins.
- **Safety by default** — avoid footguns, undefined behavior, and surprising side effects.
- **Explicit over implicit** — favor clear, readable behavior over clever but opaque magic.
- **Predictable semantics** — the same input should always produce the same result.
