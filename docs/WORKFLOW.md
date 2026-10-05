# Development workflow

Work in coherent vertical batches. The owner wants low interaction overhead: make routine technical
decisions autonomously, implement them, validate them and report results.

For a player-observable Beta-like feature, use the reference-first gate before implementation:
inspect the narrowest relevant locked source/assets, record `reference/notes/features/<feature>.md`,
then define semantic invariants, safe deviations, optimization opportunities and acceptance
criteria. Preserve recognizable behavior; exact reproduction is one option rather than the
default requirement. Do not ship an arbitrary placeholder when a researched presentation or an
intentional documented alternative is practical.

## Canonical commands

Use `just` recipes instead of repeatedly inventing command sequences.

Typical loop:

```text
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

Update architecture/decision documents when the implementation changes a durable contract. Do not
rewrite documentation as a substitute for code.

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

## Current plan and future structural work

Read the current [ROADMAP](ROADMAP.md), not the original PRE_M5 sequence as an active plan.
R2 is closed; F1 → A1 → C2 → BG1 → DX2 → RF1 → VS1 → READY1 precedes separate M5 activation.
Each pass starts only its authorized stage. Distinguish implemented facts, accepted invariants and
planned migration; retain historical reports as evidence of their measured workloads.

RF1 starts after foundational contracts settle and preserves behavior/performance unless fixing a
separately tracked defect. Inventory dependencies, orchestration/change hotspots and transitional
boundaries first; optimize change locality, not LOC. Its concise `docs/CODE_MAP.md` must explain
where mechanisms/policy live, mutation/presentation/persistence/content/control paths and extension
points, then be updated for VS1. No ECS/trait-object storage rewrite or format churn for aesthetics.
READY1 re-audits the expanded architecture, including moving-space storage/lifetime evidence; repeating
only the historical PRE_M5 checklist cannot pass. Docs-only edits need diff/link/consistency checks,
not expensive compilation; bootstrap-check currently includes builds and is unsuitable for that scope.

## GitHub issue implementation and stage closure

GitHub Issues owns live defect/debt state; [DEFECTS](DEFECTS.md) defines labels, severity and migration
identity. Before work, inspect/revalidate the issue and use `Refs #N` or `Issue #N` in implementation
notes/commits. Do not use auto-close keywords while required acceptance or CI is still pending.

After committed implementation, focused tests, required wider acceptance and Ubuntu/Windows CI pass,
post the final SHA/tests/acceptance/CI/invariant comment, then close completed. Not-planned and duplicate
closures need an explicit rationale/canonical link; use the correct reason, not completed. Migration
does not resolve the underlying concern. See [TOOLING](TOOLING.md) for portable query/close examples.

Before F1/A1/C2/BG1/DX2/RF1/VS1/READY1 closeout, query open issues for that stage. Account for each
closure, owner transfer with rationale or explicit bounded waiver. No unresolved owned P0/P1 may be
silently carried past closure. ROADMAP still owns sequencing; do not ticket every future capability.
Offline builds/tests remain independent of GitHub; unavailable tracking access means reconciliation
and issue closure cannot be claimed complete.
