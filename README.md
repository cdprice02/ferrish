# ferrish 🦀

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

## Contributing

ferrish is in early development, and contributions are welcome once the core architecture begins to stabilize.

If you’re interested in contributing:

* Open an issue to discuss ideas before starting major work
* Keep changes focused and aligned with ferrish’s core principles
* Prefer clarity and correctness over clever optimizations

More detailed contribution guidelines may be added as the project matures.

---

## Installation

Installation instructions will be added once `ferrish` reaches a usable milestone.

---

## License

ferrish is licensed under the MIT License.
