---
name: triage
description: Triage open ferrish issues — reads each issue body, proposes label assignments, milestone placements, and flags duplicates. Presents a report for approval before applying any changes. Use when asked to triage, organize, label, or prioritize open issues.
---

# Issue Triage

Reads every open issue, proposes label and milestone assignments, flags duplicates, and surfaces ambiguous cases. Nothing is applied until you confirm.

## Repo constants

- Owner: `cdprice02`, Repo: `ferrish`

## Recommended workflow

Run triage before starting a `/milestone` burndown. Milestone assumes issues are already labeled and scoped — triage is the preparation step that makes that true. The suggested order is:

1. `/triage` — organize the backlog, confirm milestone assignments
2. `/milestone <name>` — implement everything in the milestone

## Tool priority

Use `mcp__plugin_github_github__*` for all GitHub reads and writes. Fall back to `gh` CLI via Bash (e.g., `gh milestone list`) only when the MCP does not expose the needed operation.

## Steps

### 1. Fetch current state (run in parallel)

- **Open issues:** `mcp__plugin_github_github__list_issues` (state: open)
- **All labels:** `gh label list` via Bash — the MCP does not expose a label-listing operation, so `gh` is the correct tool here per the tool priority guidance. Fetches the full set of repo-defined labels. Use this as the label universe when proposing assignments.
- **Open milestones:** `gh milestone list` via Bash — the MCP does not expose milestone listing, so `gh` is the correct tool here. Lists all milestones directly, including those with zero open issues which `list_issues` would miss.

### 2. Read each issue

Use `mcp__plugin_github_github__issue_read` (method: `get`) for the full body of each open issue. Skim for:

| Signal | Suggests |
|--------|----------|
| panic, crash, fail, wrong, unexpected, broken | `bug` |
| add, support, allow, implement, introduce | `enhancement` |
| CI, workflow, deps, dependency, tooling, lint | `chore` |
| document, explain, clarify, readme | `docs` |
| restructure, reorganize, rename, move, extract | `refactor` |
| references to a specific module or file | fits in an existing milestone targeting that area |
| "depends on #N", "blocked by #N" | dependency — note for ordering |

If an issue already has a label, do not propose changing it unless there is a clear mismatch.

### 3. Build the triage report

Do not apply any changes yet. Produce a structured report:

```
## Triage Report — <date>

### Proposed label assignments
- #N "<title>": (unlabeled) → bug — "panics when empty input..."
- #M "<title>": (unlabeled) → enhancement — "adds support for..."
[omit issues that already have correct labels]

### Proposed milestone assignments
- #N → Milestone X — fits scope alongside issues #A, #B (same module)
- #P → no clear fit — see questions below

### Possible duplicates / related issues
- #N and #M both describe quoting edge cases — consider consolidating or cross-linking
[omit if no overlaps found]

### Questions for you
- #P: unclear whether this is a bug fix or a new capability — which label and milestone?
- #Q: mentions a design change that might conflict with #R — should these be sequenced?
[only include genuinely ambiguous cases]
```

### 4. Present and confirm

Show the report. Ask:
> Apply all proposed changes, or review item by item?

Wait for the response before proceeding. If the user says "apply all," proceed to step 5. If "item by item," walk through each proposed change and apply only confirmed ones.

### 5. Apply confirmed changes

For each confirmed assignment, use `mcp__plugin_github_github__issue_write` (method: `update`):
- Labels: the GitHub API replaces the label set entirely — do not send only the new label. First read the issue's current labels, then send the union of existing labels plus the newly proposed one.
- Milestone: set the `milestone` field using the milestone number

For flagged duplicates: only surface them. Never close, link, or comment on issues without explicit instruction.

### 6. Report applied changes

```
Triage complete.
- Labels applied: N issues updated
- Milestone assignments: M issues updated
- Flagged for your attention: K items (duplicates / questions)
```
