# VS1 — voxel spaces, kinematics & optional composite physics

## Goal

Voxel spaces, kinematics & optional composite physics.

## Context

The [registry](../stages.toml) owns execution order, state and focus. This contract owns scope and acceptance. Its authored requirements were extracted from the roadmap at `81b7b3a`.

## Current state

Verify actual source and labelled issues before starting; historical reports are baseline evidence, not live status.

## Target state

Meet the accepted requirements below through the public mechanism/game-policy boundary.

## Scope

Canonical concept: [VOXEL_SPACES.md](../VOXEL_SPACES.md). One local-grid model with stable spatial
identity and parent-relative transforms supports Static, Kinematic and Dynamic motion sources.
The overworld remains its cheap static fast path; dynamic physics is optional per space and aggregates
structures rather than creating one body per block. C2 typed physical properties and BG1 shapes feed
this mechanism; the game owns transport/force/connectivity meaning.

Bounded scope and migration:

- explicit stable SpaceId/local positions in commands, queries, interaction, extraction, entity
  reference frames, Bot/Agent context and persistence, with deliberate root-space compatibility;
- transform-only translation/rotation without voxel-coordinate rewrites or unchanged-section remesh;
- local adjacency/lighting independent of cross-space geometric contact;
- bounded acyclic parent-child transforms and independent space lifecycle;
- static/kinematic/dynamic proof, moving-platform/rotating reference-frame behavior and generalized
  space-vs-world/space-vs-space collision using BG1 semantic shapes;
- foundational connectivity/split lifecycle preserving identities, local content and motion state;
  optional merge/connect and future joints/docking evolve the same model, not one-off vehicle systems;
- versioned space/parent/transform/motion/content/entity/revision persistence and explicit reopen/
  migration rules; destination durability before source retirement, idempotent partial transfer;
- bounded accounting for spaces, sections/jobs, GPU/physics resources, transforms, saves and references.

Acceptance:

- unchanged large-space motion has no whole-voxel rewrite or whole-section remesh churn;
- static performance remains acceptable and body count is independent of block count;
- dirty aggregate-property rebuilds, hierarchy traversal and cross-space broadphase are bounded;
- local mechanisms survive movement, generalized shape contact is coherent and entity frames cross
  spaces without implicit permanent ownership;
- split/lifecycle and deterministic crash/reopen conserve identity/content and avoid dangling parents;
- representative moving-space routes, late/cancel/revisit jobs and failure retry show bounded lifetime;
- S1 reconsideration triggers are measured against many spaces, transform updates, local columns,
  split/merge and changing ownership; backend reconsideration only if those triggers fire;
- independent project-authored proof and Ubuntu/Windows regressions remain green.

Create `docs/VS1_REPORT.md` with migration boundaries, measured static/moving evidence, lifecycle and
fault matrix, storage-trigger results and deferred extensions. No arbitrary numeric budgets, chosen
physics library or complete transport catalog is mandated here. M5 requires accepted readiness evidence before a separate activation pass.

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

Update changed canonical contracts and record `docs/VS1_REPORT.md` using the [report template](../templates/REPORT.md); add evidence navigation in [EVIDENCE_INDEX](../EVIDENCE_INDEX.md).

## Issue reconciliation

Query the registry issue label. Follow [DEFECTS](../DEFECTS.md) and [WORKFLOW](../WORKFLOW.md); account for closure, justified owner transfer or explicit waiver. No owned P0/P1 may silently remain unresolved.

## Closeout

After acceptance and required CI, record report/evidence, reconcile issues, update registry state/focus deliberately, run docs-sync/docs-check, and stop before the next stage. M5 activation remains a separate owner-authorized pass.
