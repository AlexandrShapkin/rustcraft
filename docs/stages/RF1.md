# RF1 — structural codebase refactor & code-graph optimization

## Goal

Structural codebase refactor & code-graph optimization.

## Context

The [registry](../stages.toml) owns execution order, state and focus. This contract owns scope and acceptance. Its authored requirements were extracted from the roadmap at `81b7b3a`.

## Current state

Verify actual source and labelled issues before starting; historical reports are baseline evidence, not live status.

## Target state

Meet the accepted requirements below through the public mechanism/game-policy boundary.

## Scope

Purpose: perform a comprehensive behavior-preserving structural refactor after the pre-M5 contracts
have stabilized, so code-graph search, ownership reasoning, future edits and automated maintenance are
materially cheaper before networking multiplies cross-cutting complexity.

RF1 is deliberately late in the pre-M5 sequence: R2/F1/A1/C2/BG1/DX2 may still change boundaries. The
refactor happens after those contracts settle and before VS1 expands the spatial model; READY1
then freezes readiness. RF1 lowers change/search/reasoning cost, not merely file sizes.

### RF1.1 — architecture/code-graph inventory

Before moving code, produce an evidence-based inventory of:

- crate dependency graph and public dependency direction;
- largest/most-connected modules, files, types and functions;
- modules with excessive fan-in/fan-out or unrelated responsibilities;
- repeated orchestration/state-machine code;
- transitional adapters and compatibility layers that no longer have active consumers;
- duplicate semantic concepts with different names/types;
- cross-crate accesses that bypass the intended owner/service boundary;
- test-only/public APIs that inflate the normal code graph;
- compilation hot spots where structural changes can reduce incremental rebuild scope without
  distorting architecture.

Do not use arbitrary line-count limits as the only criterion. Prioritize ownership clarity, dependency
shape and change locality.

### RF1.2 — behavior-preserving structural refactor

Refactor the complete active codebase in bounded slices. Expected work includes, where evidence
supports it:

- decompose oversized orchestration modules and "god" state holders into subsystem-owned state and
  services with explicit lifecycle boundaries;
- give major client concerns clear homes, e.g. startup/session, input, fixed simulation, presentation,
  streaming/residency, persistence coordination, rendering, diagnostics and automation;
- narrow crate/module visibility and make public APIs intentional;
- replace cross-module field poking with small semantic operations where it improves ownership;
- remove dead transitional adapters after all real consumers have migrated;
- consolidate duplicate helpers/state machines and vocabulary;
- move tests next to the contract they verify and keep expensive integration tests at explicit
  composition boundaries;
- normalize naming so semantic search produces one canonical concept instead of historical aliases;
- preserve generic engine/game ownership and prevent `minecraft-b173` policy from leaking back into
  generic crates;
- keep generated/runtime-local handles out of external or durable identities;
- reduce unnecessary rebuild coupling where crate/module boundaries can do so without creating tiny
  artificial crates.

Do NOT:

- rewrite working subsystems solely for style;
- introduce a general ECS because modules are large;
- split every file to satisfy a numerical LOC target;
- add macro/reflection frameworks that make navigation harder;
- change gameplay/render/storage/network semantics under the label "refactor";
- combine RF1 with new features or optimization campaigns.

### RF1.3 — code-map and maintenance surface

Produce/update a concise canonical code map, preferably `docs/CODE_MAP.md`, that lets a human or
code-graph agent answer quickly:

- where a client input event enters and becomes semantic intent;
- where fixed simulation authority lives;
- where presentation interpolation lives;
- where world streaming/residency is coordinated;
- where persistence requests and durability acknowledgements flow;
- where semantic resources compile and reach the renderer;
- where Control/Diagnostics/Config are owned;
- where `minecraft-b173` game policy begins;
- where headless server composition differs from graphical client composition;
- which APIs are stable contracts versus runtime-local implementation details.

Documentation must describe actual post-refactor ownership, not an aspirational diagram.

### RF1.4 — refactor acceptance

RF1 closes only when:

- pre/post code-graph/dependency evidence is recorded;
- major ownership hotspots identified in RF1.1 are resolved or explicitly justified;
- no new cyclic crate dependency is introduced;
- generic dependency direction remains valid;
- normal `rustcraft-client` and `rustcraft-server` composition paths remain clear;
- canonical external/durable semantic identities are unchanged unless a previously approved A1
  migration explicitly required them;
- persistence formats and frozen generation hashes remain unchanged;
- R2 semantic resource contracts remain unchanged;
- F1 unified client workflow remains unchanged;
- A1 capability/identity/trust contracts remain unchanged;
- C2 common content-definition/capability/handler contracts remain unchanged;
- BG1 semantic model/shape and cube-fast-path contracts remain unchanged;
- DX2 canonical tooling remains valid;
- behavior-equivalence tests pass after each bounded refactor slice and in the final workspace;
- incremental compile/search/change locality is measured or qualitatively demonstrated with concrete
  before/after hotspots rather than claimed from reduced line counts;
- `cargo fmt`, workspace check/tests, strict Clippy, dependency hygiene, headless/server checks and
  public Ubuntu/Windows CI are green;
- no version bump, tag or release is created.

Create `docs/RF1_REPORT.md` with the inventory, refactor map, before/after dependency/code-graph
summary, deliberately retained complexity and regression evidence.

## Invariants

Preserve all explicit semantic, authority, performance and compatibility constraints in Scope; architecture/domain documents own shared invariants.

## Out of scope

Do not implement other stages or expand the explicit exclusions in Scope without owner authorization.

## Implementation constraints

Use the navigation anchors and relevant decisions in the registry. Changes to these hints do not change public API ownership.

## Acceptance

Prove every required behavior and acceptance condition in Scope. Do not infer closure from code completion alone.

## Validation

Run focused regression/proof checks first, then required wider headless/render/platform checks and Ubuntu/Windows CI. Record conditional hardware evidence honestly; never invent budgets.

## Documentation updates

Update changed canonical contracts and record `docs/RF1_REPORT.md` using the [report template](../templates/REPORT.md); add evidence navigation in [EVIDENCE_INDEX](../EVIDENCE_INDEX.md).

## Issue reconciliation

Query the registry issue label. Follow [DEFECTS](../DEFECTS.md) and [WORKFLOW](../WORKFLOW.md); account for closure, justified owner transfer or explicit waiver. No owned P0/P1 may silently remain unresolved.

## Closeout

After acceptance and required CI, record report/evidence, reconcile issues, update registry state/focus deliberately, run docs-sync/docs-check, and stop before the next stage. M5 activation remains a separate owner-authorized pass.
