<!-- shared-agent:start -->
# Agent engineering contract

## 1. Purpose and authority

Follow [.github/DEVELOPMENT_INTERFACE.md](.github/DEVELOPMENT_INTERFACE.md), the
single shared workflow interface. These instructions do not grant runtime,
merge, deployment, device, credential or destructive-operation authority.
Preserve stricter product rules in section 10; stop dependent work on a conflict
and continue independent authorized work. Never use RDC for development.

## 2. Repository start (START)

Discover the actual root, full HEAD, branch and Git status. Compare the intended
base SHA with current GitHub evidence; record divergence rather than assuming
freshness. Inventory worktrees, unfinished changes and active PRs, including
intersecting files and dependencies, before creating commits. Do not silently
switch checkout, overwrite unknown work or rewrite existing branch history.

## 3. GitHub task (TASK)

Search open and closed Issues; reuse the owning task and avoid duplicates.
Record goal, scope/non-goals, acceptance, dependencies and risks. Link Issues
and PRs for cross-repository work to the coordinating Issue. Roadmaps and stage
registries provide navigation, not execution authority or a second live queue.

## 4. Necessary context (CONTEXT)

Read AGENTS.md, the shared interface and the product context entrypoint first.
Read only task-relevant contracts, architecture and source; never load the
whole repository by default. Prefer native git/rg and targeted reads for known
locations; use CodeGraph for useful structural context, semantic tools for
symbol questions and Context7 for version-sensitive external APIs when available.
Do not repeat equivalent retrieval or install tools automatically. An absent
tool is a reported limitation, not permission to download or execute it.

## 5. Planning and implementation (IMPLEMENT)

Work on one bounded task/branch from a verified base or explicitly documented
PR dependency. Preserve product architecture and security invariants. Make
minimal coherent changes; add regression and negative tests for behavior.
Create extra clones/worktrees only for a concrete justified need and within
product authorization. Preserve unrelated files, user data and unique evidence.

## 6. Verification and quality (VERIFY)

Run targeted checks first, then the full mandatory product gates in section 10.
A smoke test cannot replace native validation. Report PASS, FAIL, BLOCKED and
UNVERIFIED distinctly; never disable a failing check to obtain green results.
Keep secrets/private data out of source, diagnostics and public reports. CI is
source evidence, not proof of deployment correctness or runtime authority.

## 7. PR, CI, review and acceptance (PR / REVIEW)

Inspect the complete diff, including new files, before committing. Create or
update a scoped PR linked to its Issue; record base and full HEAD SHA, actual
commands/results, exact-head CI, risks and limits. Repeat affected verification
and refresh CI/review evidence after HEAD changes. Keep author verification,
independent technical ACCEPT, formal GitHub APPROVE and merge authority separate.
Never present your own checks as independent review. Keep PRs Draft pending
required independent acceptance. Do not merge, deploy or change GitHub settings
without explicit task authorization and all applicable product gates.

## 8. Temporary resources (CLEANUP)

At completion, abandonment or supersession, inventory obsolete worktrees,
branches and reproducible artifacts; inventory disk usage before expensive work
when space is low. Delete only exact, verified recoverable and unused items
within authority, after checking tasks/PRs, dirty/untracked files and processes.
Never delete active/protected branches, open or Draft PR work, unknown data,
unique results or user files. No blanket git clean, forced worktree removal or
age-based deletion. Preserve uncertain candidates; record what was removed,
what was retained, why, and measured freed space only when actually measured.

## 9. Final evidence (REPORT)

Report: what changed; repository, PR and full SHA; actual tests and CI; remaining
UNVERIFIED evidence; blockers; owner actions; and temporary resources removed
or preserved. Separate source preparation from installation and live-host proof.

## 10. Product-specific rules
<!-- shared-agent:end -->

<!-- agent-project: rustcraft -->

Context entrypoint: `just codex-context [STAGE]`; documentation owner:
`docs/INDEX.md`; workflow: `docs/WORKFLOW.md`. Full native gate: `just ci`,
plus mandatory GitHub CI on Ubuntu and Windows. Smoke alone is insufficient.
Preserve stages, documentation registry, reference-study and engine/game policy.
No new `just verify` alias is needed: the shared interface preserves `just ci`.

### Repository instructions for Codex

#### Start small

First discover `git rev-parse --show-toplevel` from the launch directory and verify HEAD and
`git status --short`; use `just status` to report checkout, command directory and registry state.
Work in the checkout from which the agent was launched. Stop on a requested baseline/checkout
mismatch or unknown dirty work; never silently switch to a remembered path or invent another
clone/worktree without explicit owner authorization.

Read this file, run `just codex-context [STAGE]` (add `--offline` when needed), then read the
selected stage contract and only task-relevant pointers. [Documentation index](docs/INDEX.md)
defines ownership/precedence. Do not load all reports or architecture docs by default.
Registry focus is navigation, not permission to start a stage; implement only the authorized batch.

#### Always-on invariants

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

#### Work safely

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

#### Mandatory cleanup after each task

Prevent disk exhaustion: on task closeout, abandonment or replacement, promptly
audit and remove **verified obsolete** local worktrees, task branches, generated
build/test outputs and caches. Check disk space and inventory candidate paths
before another expensive task when storage is low. For worktrees and branches,
verify actual Git state, active tasks/PRs, dirty or untracked files, running
processes and recoverability of commits before removing anything. Only delete
known project-owned artifacts that are reproducible and not needed as evidence.
Never delete active/default/protected branches, open or Draft PR branches,
unknown work, checkpoints, user data, reference material or unique test data.
Do not use blanket `git clean -fdx`, forceful worktree removal, guessed
recursive paths or automatic age-based cleanup. Remote branch deletion,
device cleanup and uncertain/destructive operations require separate authority.
Record exact candidates, actions and measured reclaimed space; if any
precondition is unknown, preserve the item and report it for review.

