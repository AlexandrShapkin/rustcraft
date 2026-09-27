# Repository instructions for Codex

Build the project described in this repository. Do real implementation work; do not stop at a
plan when the requested batch is implementable.

Before changing code, read:

- `docs/PRODUCT.md`
- `docs/ARCHITECTURE.md`
- `docs/GAMEPLAY_CONTRACT.md`
- `docs/AGENTS_AND_BOTS.md`
- `docs/CONTENT_SYSTEM.md`
- `docs/MODDING.md`
- `docs/PERFORMANCE.md`
- `docs/WORKFLOW.md`
- `docs/TOOLING.md`
- `docs/REFERENCE_POLICY.md`
- `docs/ROADMAP.md`
- `docs/DECISIONS.md`
- `docs/DEFECTS.md`

When work depends on Beta behavior, protocol, legacy formats or assets, also read
`reference/SOURCES.md` and run `just refs-status` before choosing a reference source.

## Invariants

- Engine infrastructure must not depend on Beta-specific gameplay concepts.
- First-party gameplay modules are native Rust; third-party downloadable executable content is
  sandboxed, never arbitrary native shared libraries.
- Human input, network players, bots, replays and tests feed the simulation through semantic
  controller/agent intent.
- Bots are a platform feature, not graphical-client emulation.
- A server defines a content profile; clients and bots automatically resolve the content they need.
- Do not reproduce historical bugs unless intentionally promoted to a documented mechanic.
- Do not trade away the near-field visual baseline merely to claim higher performance.
- Profile before exotic optimization. Never claim an unmeasured speedup.
- Reference Minecraft source, external reimplementations and vanilla assets are evidence, not code
  to port or project architecture to inherit.
- Never redistribute proprietary Mojang assets through this repository by default.

## Work style

Use `just` as the normal command surface. Prefer `just bootstrap-check` before and after a batch.
Use repository skills when their descriptions match the task. If Serena is available, use it for
symbol-aware navigation/refactoring. If Context7 is available, use it for current third-party
library/API documentation. Fall back to local tools rather than blocking if an MCP is absent.

For reference-heavy tasks, inspect only the sources relevant to the question. Cross-check
nontrivial historical behavior when useful; do not read every reference repository mechanically.
Write project-authored conclusions into `reference/notes/` when the result is durable.

Fix blockers, corruption, security issues and architecture violations immediately. Record
non-blocking defects in `docs/DEFECTS.md` and batch-fix them later.

Record durable architecture choices in `docs/DECISIONS.md`.
