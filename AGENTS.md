# Repository instructions for Codex

## Start small

First discover `git rev-parse --show-toplevel` from the launch directory and verify HEAD and
`git status --short`; use `just status` to report checkout, command directory and registry state.
Work in the checkout from which the agent was launched. Stop on a requested baseline/checkout
mismatch or unknown dirty work; never silently switch to a remembered path or invent another
clone/worktree without explicit owner authorization.

Read this file, run `just codex-context [STAGE]` (add `--offline` when needed), then read the
selected stage contract and only task-relevant pointers. [Documentation index](docs/INDEX.md)
defines ownership/precedence. Do not load all reports or architecture docs by default.
Registry focus is navigation, not permission to start a stage; implement only the authorized batch.

## Always-on invariants

- Engine provides mechanism; game packages own policy. Without `minecraft-b173`, generic code must
  remain coherent; that package uses the public Game API with no private gameplay shortcut.
- First-party game modules are native Rust; downloaded executable mods are sandboxed, never arbitrary
  native-library autoexec. Servers define profiles; clients/bots resolve required content.
- Human/network/bot/replay/test controllers feed semantic intent. Bots are first-class participants,
  not graphical-client emulation.
- Semantic IDs are durable/external identity; dense IDs and resource handles are profile-local only.
- Preserve clear recognizable gameplay while prioritizing measured performance, frame stability,
  scalability and extension boundaries. Do not reproduce historical bugs without a documented mechanic.
- Profile before optimization; never claim unmeasured speedups. References are evidence, not code or
  architecture to port. Never redistribute proprietary Mojang assets by default.

## Work safely

Use `just`; inspect existing contracts before edits and run focused validation first. Never discard
unknown dirty work or reset to an assumed baseline. Preserve user input and historical evidence.
Search open/closed GitHub Issues before recording actionable findings; GitHub owns live status.
[Issue policy](docs/DEFECTS.md) and [workflow](docs/WORKFLOW.md) govern acceptance/closure. Report P0/P1
immediately; repair within authorization or obtain explicit disposition, without scope creep.
For player-observable Beta gameplay/UI/control/assets/presentation, use the bounded
[reference-study skill](.agents/skills/reference-study/SKILL.md) and write the feature note before
production edits; read `reference/SOURCES.md` and run `just refs-status` when references matter.
Record durable choices in [DECISIONS](docs/DECISIONS.md). Use repository skills when relevant;
[Codex notes](docs/CODEX.md) cover recovery and [tooling](docs/TOOLING.md) covers commands.
