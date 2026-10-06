# Codex operating notes

## Discover the launched workspace

Begin with `git rev-parse --show-toplevel` from the launch directory, then verify HEAD and
`git status --short`. Use `just status` for the discovered root, actual command working directory,
branch, HEAD, upstream, worktree, remote and focused stage. This is read-only local inspection.
Work in the launched checkout; a remembered absolute path or report is not checkout selection.
On a baseline/checkout mismatch or unknown dirty work, stop and report before edits. Do not invent
another clone/worktree without explicit owner authorization. The owner must launch the intended
checkout or explicitly direct an existing one; do not repair a mismatch by resetting/cleaning.

The documented owner convention is `~/Projects/rustcraft`, not a hardcoded runtime/test path.
`/tmp` checkouts are temporary/recovery only. Milestone acceptance clones are isolated evidence
workspaces, not normal development homes. This convention creates or relocates no checkout.
See [WORKFLOW](WORKFLOW.md) for normal commands and local assets.

## Progressive loading

Read [AGENTS](../AGENTS.md), then run `just codex-context [STAGE]` (or append `--offline`). Read the
selected contract and the small pointer set needed for the task. Open relevant decision sections by
ID; do not scan all DECISIONS or load historical reports/performance files unless evidence requires it.
[INDEX](INDEX.md) defines canonical owners and precedence. Context output is navigation, not a
snapshot of implementation or permission to activate the focus stage.

Use native git/rg/targeted reads for known locations; use structural navigation when discovering
unfamiliar architecture and symbol-aware tools when they add value. Missing optional tools are not
blockers. Use the relevant repository skills, including
[stage-execution](../.agents/skills/stage-execution/SKILL.md) and
[documentation-maintenance](../.agents/skills/documentation-maintenance/SKILL.md).

## Interrupted session recovery

Start by discovering `git rev-parse --show-toplevel`, then `just status`, `git log -5 --oneline`, `just codex-context [STAGE]` and the relevant
GitHub issue state. Establish actual HEAD/remote, inspect unknown changes and existing report/artifact
pointers before repeating expensive work. Never reset, stash, clean or overwrite unknown dirty work
just to reach a remembered baseline. Preserve user planning input. Record unfinished work/evidence in
the authorized report or issue so the next session can recover without chat history.

## Bounded execution

The selected stage contract bounds the work; owner instructions still determine authorization.
Validate focused changes first and broaden only as required by [WORKFLOW](WORKFLOW.md). Update
canonical docs only when contracts change, keeping implementation facts distinct from accepted targets.
For issue search, evidence, non-auto-close references and acceptance/CI closure use
[DEFECTS](DEFECTS.md) and [WORKFLOW](WORKFLOW.md); do not reproduce their lifecycle here.
Stop after the authorized batch even if the next stage appears in context.
