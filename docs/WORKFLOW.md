# Development workflow

Work in coherent vertical batches. The owner wants low interaction overhead: make routine technical
decisions autonomously, implement them, validate them and report results.

For a player-observable Beta-like feature, use the reference-first gate before implementation:
inspect the narrowest relevant locked source/assets, record `reference/notes/features/<feature>.md`,
then define semantic invariants, safe deviations, optimization opportunities and acceptance
criteria. Preserve recognizable behavior; exact reproduction is one option rather than the
default requirement. Do not ship an arbitrary placeholder when a researched presentation or an
intentional documented alternative is practical.

## Owner workspace and command directory

The normal owner workspace convention is `~/Projects/rustcraft`. It describes where the owner keeps
source, not an absolute path scripts/tests enforce. Discover the actual Git root at launch; agents
work in that checkout and verify HEAD/worktree before editing. A mismatch must be reported, never
resolved by silently choosing a remembered checkout or creating a second clone/worktree. Additional
checkouts require explicit owner authorization. No workspace migration is performed by these commands.

`/tmp` checkouts are recovery/temporary only. Milestone acceptance clones collect isolated evidence;
they are not normal development workspaces. Neither historical reports nor a legacy checkout name
selects today's source root. `just status` reports the actual checkout and command working directory.
By default, `just` discovers the nearest justfile and runs recipes from its directory, so normal
project commands operate at that checkout's root even when invoked in a subdirectory.

Proprietary assets are owner-supplied local data, separate from tracked source. The preferred ignored
location is `reference/assets/vanilla-b1.7.3/` beneath the actual chosen checkout (the ignored zip is
also supported). Keep assets untracked; source/CI does not depend on another historical checkout to
provide them. See [reference sources](../reference/SOURCES.md) for policy. No proprietary asset is
committed or automatically copied by this workflow.

## Canonical commands

Use `just` recipes instead of repeatedly inventing command sequences.

Normal development commands are shell-neutral invocations of the repository recipes:

| Command | Purpose |
| --- | --- |
| `just status` | Discover checkout, command directory, HEAD/worktree and focused stage. |
| `just client` | Normal human/developer client in the development profile. |
| `just client-release` | Same client runtime in release for ordinary play. |
| `just smoke` | Headless shared-runtime smoke. |
| `just sample-game` | Independent-game composition proof. |
| `just test` | Workspace tests. |
| `just ci` | Documentation/tooling, formatting, check, tests and strict Clippy. |

Historical milestone/acceptance recipes remain specialist evidence tools. WF1 retires none of them;
DX2 still owns broader equivalence/retirement. Fish, Bash and PowerShell users invoke the same recipes;
recipe implementation handles project operations rather than owner-specific shell snippets.

Typical loop:

```text
just status
just doctor
just refs-status        # when reference material may matter
just bootstrap-check

implement a coherent batch

just ci
just smoke
just sample-game       # dependency-direction integration check
```

Before extending an existing mixed M0-M3 subsystem, consult `ARCHITECTURE_AUDIT.md`. Put reusable
mechanism in engine/Game API crates and game rules in `minecraft-b173`; migrate only the slice
needed for the change.

For a substantial optimization, document the expected performance/frame-time, memory,
scalability and extensibility benefits against visual cost, semantic cost, implementation
complexity and maintenance cost. Use measurements where possible and qualitative reasoning where
measurement is not yet available; do not invent numerical scores.

Fetch public research inputs with `just refs-fetch` when needed. Do not make normal builds depend on
network access or on reference repositories being present.

## Defect batching

Fix immediately: build blockers, crashes in the required scenario, security/memory-safety issues,
data corruption, broken contracts and architecture mistakes that become expensive if left.

Search open and closed GitHub Issues before recording confirmed actionable concerns; update the
canonical ticket or create a labelled issue with evidence and acceptance. Batch lower-priority work
within authorized stages. `docs/DEFECTS.md` owns tracking policy/history/index, never live status.

## Documentation discipline

Use [INDEX](INDEX.md) to select the canonical owner. Update architecture/decisions when a durable
contract changes. Reports preserve their baseline. Only docs-sync owns generated navigation; never
rewrite manual prose through generation or update docs as a substitute for implementation.

## Research

For current Rust/library behavior, prefer official docs. If Context7 is available, use it. For
codebase-wide symbol work, prefer Serena when available. Use local tools (`rg`, `git`, `jq`,
`cargo metadata`, `cargo tree`, source reading) as a reliable fallback.

For Beta behavior, protocol, legacy formats or visual conventions, read `reference/SOURCES.md`, run
`just refs-status` and select the narrowest source that answers the question. Cross-check
nontrivial ambiguities rather than inheriting one reference implementation blindly. Durable
project conclusions belong in `reference/notes/`, not as pasted source excerpts.

`just refs-lock` may be used after significant research to record the exact third-party reference
commits consulted.

For block/item presentation regressions, start with `just render-test face-DIRECTION --mode
corners`, then `--mode uv`, before changing atlas mappings. Check both positional and UV
triangle orientation. Use `just render-test-all` for world/dropped/GUI comparisons, including
rear views and native GUI scale; inspect the images, not only the command exit status.
`just fidelity-m3` now runs these deterministic offscreen captures and exits. Keep PNGs under
ignored `target/render-tests/`; do not commit local proprietary-texture captures. CPU tests
remain GPU/asset independent. Captures do not replace manual acceptance when a milestone requires
interactive review; M3's required review is complete.

## Planning and closeout ownership

GitHub Milestones group large product goals/delivery horizons; [ROADMAP](ROADMAP.md#product-delivery-horizons)
defines the initial model. They complement the existing planning layers:

- `stages.toml` alone owns execution order/state/focus; a milestone never activates a stage.
- `stage:*` identifies an issue's technical stage owner; `area:*`, `type:*`, `severity:*` retain
  their existing semantics in [DEFECTS](DEFECTS.md).
- Issues remain bounded actionable work; PRs remain integration/acceptance units. An issue can carry
  both a milestone and those labels, without either replacing the other.

Do not create a milestone per stage or equate milestones with SemVer/releases. Do not invent due
dates. Add further milestones only for sufficiently defined product goals. Assign existing issues
only when their contribution to that goal is unambiguous; leave uncertain/unassigned backlog alone.
Missing issues for future capabilities do not require placeholder tickets to fill a milestone.
GitHub owns live milestone state and membership; do not mirror their progress/counts locally.

Close a milestone only after its product result actually exists in main and its member issues are
reconciled against acceptance, including explicit disposition of any incomplete work. Zero open
issues alone does not prove the goal is delivered. Historical goals may be recorded and closed after
reconciling existing completed issues with accepted main evidence. Query/verify milestones at
planning/reconciliation using [TOOLING](TOOLING.md#github-milestone-commands), not in a polling loop.

[Registry](stages.toml) owns order/state/focus; [stage contracts](stages/INDEX.md) own detailed scope.
Use `just codex-context [STAGE]` and the selected contract. Neither the historical PRE_M5 audit nor
registry focus activates a stage. [INDEX](INDEX.md) defines current versus historical ownership.

Prepare proposed closeout by reconciling labelled issues, adding a baseline-specific report using the
[report template](templates/REPORT.md), updating [EVIDENCE_INDEX](EVIDENCE_INDEX.md), and deliberately
proposing registry state/focus changes within owner authorization. The report and proposed closeout
may be part of the same PR as implementation. A closed state in an unmerged branch is a proposal;
merge after acceptance and final green PR CI makes closeout effective on main. Do not claim pending
CI, merge or issue closure as completed evidence. Closed stages remain in the sequence.
Run `just docs-sync`, `just docs-check` and `just docs-test`; stop before the next stage.
`just status` reports live checkout/registry facts; it is not a second mutable planning database. New planning uses `just stage-new` plus one contract;
architectural changes additionally update their canonical domain docs and decisions.

Docs-only work uses docs tests/checks and diff review, not expensive Rust bootstrap. Tooling changes
also validate affected command/CI paths. Normal `just ci` includes offline docs integrity.

## Branches and PR publication

`main` is the sole primary integration branch. Substantial stage/feature/fix work defaults to a
focused branch and enters main through a PR. Prefer one branch/PR per stage, e.g. `stage/DX2` or
`stage/RF1`; split only for a genuinely independent bounded slice. Do not introduce GitFlow,
`develop` or release branches. Small docs-only changes may use direct-main publication only with
explicit owner permission; substantial code, architecture or stage changes use PRs.

Complete local implementation, required checks and diff review before opening/finalizing the PR.
Publish only when authorized. Keep the PR body compact, with **Scope**, **Changes**, **Validation**,
**Issues**, **Deferred**; link detailed evidence in stage/report docs rather than copying logs.
Review the complete final diff, including report and proposed closeout, and wait for required
Ubuntu/Windows PR CI on that revision before an authorized merge. Changes after checks require
appropriate validation and final PR CI. One green final PR CI over the complete result is sufficient:
separate implementation/closeout publications and two CI passes on main are not required. Existing
CI triggered by merge may still run; this workflow does not change CI configuration.

These rules replace the historical two-publication/manual-closure procedure in earlier reports or
execution notes. Reports retain their original baseline evidence. RustCraft's registry/contracts and
[issue taxonomy](DEFECTS.md) remain unchanged; focused branches and explicit PR evidence follow the
practice in [codex-smart development](https://github.com/AlexandrShapkin/codex-smart/blob/main/docs/development.md).

## GitHub issue reconciliation and stage closure

GitHub Issues owns live defect/debt state; [DEFECTS](DEFECTS.md) defines labels, severity and migration
identity. Query live issues at planning/reconciliation, including open and closed matches before
recording a finding. Query PR/check state during publication; do not continuously poll GitHub during
local implementation. Offline context/build/test remain independent of tracking access.

Use `Closes #N` in the PR only when the complete PR satisfies that issue's acceptance; otherwise use
`Refs #N`. Required acceptance and final green PR CI gate merge, so a closing reference may be prepared
before that CI completes. After merge, standard GitHub automation may close completed issues. A
separate closure comment is needed only for additional explanation/evidence not already available
from the PR and linked report. Not-planned/duplicate closures still require explicit rationale or a
canonical link and the correct reason. Migration alone does not resolve a concern.
See [TOOLING](TOOLING.md) for portable publication/query examples.

Before any stage closeout, query open issues using its registry issue label. Account for each
closure through the PR, owner transfer with rationale or explicit bounded waiver. Verify actual
issue disposition after merge rather than reporting automation as already completed. No unresolved
owned P0/P1 may be silently carried past closure. The registry owns sequencing; do not ticket every future capability.
After merge, verify that actual main contains the accepted result and proposed closeout, and check
relevant milestone membership/state without treating a stage merge as automatic milestone closure.
Offline builds/tests remain independent of GitHub; unavailable tracking access means reconciliation
and issue closure cannot be claimed complete.
