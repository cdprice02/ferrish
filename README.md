# ferrish 🦀

[![build](https://github.com/cdprice02/ferrish/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/cdprice02/ferrish/actions/workflows/build.yml)
[![test](https://github.com/cdprice02/ferrish/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/cdprice02/ferrish/actions/workflows/test.yml)
[![lint](https://github.com/cdprice02/ferrish/actions/workflows/lint.yml/badge.svg?branch=main)](https://github.com/cdprice02/ferrish/actions/workflows/lint.yml)
[![coverage](https://coveralls.io/repos/github/cdprice02/ferrish/badge.svg?branch=main)](https://coveralls.io/github/cdprice02/ferrish?branch=main)
<!--
[![crates.io](https://img.shields.io/crates/v/ferrish.svg)](https://crates.io/crates/ferrish)
[![docs.rs](https://docs.rs/ferrish/badge.svg)](https://docs.rs/ferrish)
-->

`ferrish` — a modern, Rust-powered shell focused on safety, performance, and a clean interactive experience.

> ⚠️ **Status:** ferrish is in early development and is not yet ready for daily use.

---

## Why ferrish?

Most shells in common use today were designed decades ago. While they are powerful, they also carry historical complexity, unsafe defaults, and difficult-to-maintain semantics.

ferrish explores what a shell can look like when it is:

* Built in **Rust** from the ground up
* Designed with **safety and correctness** as first-class goals
* Opinionated about **clarity over cleverness**
* Friendly to modern tooling and workflows

ferrish is not intended to be a drop-in replacement for existing shells. It is an experiment in better foundations.

---

## Core Principles

ferrish is guided by a small set of principles that influence every design decision:

* **Safety by default**
  Avoid footguns, undefined behavior, and surprising side effects.

* **Explicit over implicit**
  Favor clear, readable behavior instead of clever but opaque magic.

* **Predictable semantics**
  The same input should always produce the same result.

* **Composable, but understandable**
  Pipelining and composition should remain powerful without becoming unreadable.

* **Fast enough, then correct**
  Performance matters, but never at the cost of correctness.

---

## Non-Goals

ferrish deliberately does **not** aim to:

* Be fully compatible with bash, zsh, or POSIX shell syntax
* Reimplement decades of legacy shell quirks
* Optimize for every possible one-liner at the cost of readability
* Replace existing shells overnight

Compatibility may be explored selectively, but only when it aligns with ferrish’s principles.

---

## Development

Install [pre-commit](https://pre-commit.com) once, then activate the hooks:

```bash
uv tool install pre-commit
pre-commit install --hook-type pre-commit --hook-type pre-push
```

On every `git commit`: `cargo fmt --check`, `cargo check`, and `cargo clippy` run automatically.
On every `git push`: `cargo mutants --in-diff` runs against the diff being pushed. Bypass with `git push --no-verify` when needed.

---

## Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md) for branch conventions, PR workflow, test requirements, and project principles.

---

## Installation

Installation instructions will be added once `ferrish` reaches a usable milestone.

---

## License

ferrish is licensed under the MIT License.
