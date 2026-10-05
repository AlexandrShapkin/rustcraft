# Codex operating notes

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

Start with `git status --short`, `git log -5 --oneline`, `just codex-context [STAGE]` and the relevant
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
