# BG1 — generalized block geometry & model system

## Goal

Generalized block geometry & model system.

## Context

The [registry](../stages.toml) owns execution order, state and focus. This contract owns scope and acceptance. Its authored requirements were extracted from the roadmap at `81b7b3a`.

## Current state

Verify actual source and labelled issues before starting; historical reports are baseline evidence, not live status.

## Target state

Meet the accepted requirements below through the public mechanism/game-policy boundary.

## Scope

Purpose: make the voxel cell a placement/addressing unit rather than an assumption that every block is
a full axis-aligned cube. Cube rendering remains the optimized fast path; non-full and complex static
block shapes become first-class generic content through C2 capabilities and semantic model/shape
resources.

Required geometry model:

- `BlockState` resolves through semantic model/shape roles rather than hard-coded block classes;
- support an efficient full-cube fast path;
- support composed box/prismatic primitives for common shapes;
- support wedge/sloped geometry, including at least a 45-degree slope proof;
- support general bounded static mesh geometry for decorative/building shapes that cannot be expressed
  cleanly as boxes;
- allow state-dependent model selection/transform such as facing, top/bottom half, straight/inner/outer
  corner or other game-defined variants without introducing engine classes such as `StairBlock` or
  `WedgeBlock`;
- authored `ModelKey`/shape identities compile to runtime-local validated handles, analogous to the
  existing semantic-resource/profile compilation model.

Geometry semantics must keep separate contracts for:

- render geometry;
- collision shape;
- selection/raycast shape;
- face/volume occlusion and neighbor culling;
- light/coverage behavior;
- state/transform rules.

Visual geometry and collision do not have to be identical. Complex visual meshes may use a simpler
validated collision/selection representation when game policy chooses it.

Meshing/culling requirements:

- remove active generic assumptions that a block always contributes six full square faces;
- retain the optimized cube path for ordinary terrain;
- introduce a compact occlusion/coverage contract so partial neighbors can cull safely without requiring
  expensive arbitrary polygon clipping in the normal hot path;
- keep room for a more precise fallback where a model genuinely needs it;
- ensure lighting can distinguish full occluders, partial/transparent coverage and non-occluding models
  without requiring physically exact arbitrary-mesh light transport in BG1;
- chunk meshing remains generic and does not learn concepts such as stairs, slabs, fences or Minecraft
  connection policy.

Content/game policy:

- `minecraft-b173` and later games/mods may define slabs, stairs, columns, panels, beams, quarter blocks,
  inner/outer corners, 45-degree wedges/slopes, frames, arches, decorative trims and other shapes using
  the same mechanism;
- BG1 does NOT require shipping a large decorative catalog; infrastructure plus a compact proof set is
  sufficient;
- the minimum proof set should include a full cube, slab, stair-like composed shape, 45-degree wedge,
  multipart/static-mesh shape and an independent `sandbox-test` example;
- adding a new decorative shape should not require edits to renderer/collision/raycast/persistence
  switches across the engine; only genuinely new generic capability semantics justify engine changes.

Persistence and future networking:

- durable/network-facing identity remains semantic block/state/model identity, never runtime mesh/GPU
  handles;
- existing semantic BlockState persistence remains compatible unless an explicit versioned extension is
  required;
- model resources are presentation/content resources and must not be serialized as process-local handles;
- BG1 must not design the M5 wire protocol or M6 mod transport.

Acceptance must prove:

- non-full geometry renders correctly across chunk boundaries;
- collision, raycast/selection and placement remain coherent for partial/sloped shapes;
- occlusion/culling does not create missing faces or obvious internal overdraw regressions in the proof
  set;
- state rotation/variant selection is deterministic;
- cube-heavy terrain keeps its specialized fast path and does not regress materially;
- independent game content can define complex shapes through the same generic mechanism;
- no generic engine code needs named special cases for slab/stair/wedge classes;
- all normal workspace/headless/render/public CI gates remain green.

Create `docs/BG1_REPORT.md` with the model/shape contract, fast-path design, proof shapes, performance
comparison and known deferred geometry limitations.

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

Update changed canonical contracts and record `docs/BG1_REPORT.md` using the [report template](../templates/REPORT.md); add evidence navigation in [EVIDENCE_INDEX](../EVIDENCE_INDEX.md).

## Issue reconciliation

Query the registry issue label. Follow [DEFECTS](../DEFECTS.md) and [WORKFLOW](../WORKFLOW.md); account for closure, justified owner transfer or explicit waiver. No owned P0/P1 may silently remain unresolved.

## Closeout

After acceptance and required CI, record report/evidence, reconcile issues, update registry state/focus deliberately, run docs-sync/docs-check, and stop before the next stage. M5 activation remains a separate owner-authorized pass.
