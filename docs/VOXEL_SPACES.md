# Voxel spaces — accepted VS1 direction

Status: accepted architectural direction; **not implemented**. VS1 starts after RF1 and closes
before READY1. M5 cannot start until RF1, VS1 and READY1 are complete. See [ROADMAP](ROADMAP.md)
for bounded execution and [DECISIONS](DECISIONS.md) D-057–059 for authority.

## Current and target

Current: `engine-core::World` is one voxel-grid container. `BlockPos`, `SectionPos`, world commands,
raycasts and entity observations assume that grid; column persistence is rooted at `chunks/x.z.rcc`.
There is no SpaceId, moving-grid hierarchy or composite structure physics contract.

Target: movable voxel content is not a special block, entity or vehicle. It is a **voxel space**
whose transform relative to a parent/root can change. A space owns a local integer voxel grid,
block/content state, local simulation relationships and coordinates, stable spatial identity,
parent-relative transform and motion state. The ordinary overworld is its static fast-path instance.

One model has three motion sources:

| Mode | Transform authority |
| --- | --- |
| Static | Fixed by spatial authority. |
| Kinematic | Driven directly by deterministic game/system/control logic. |
| Dynamic | Driven by an optional physics solver/motion provider. |

Changing motion source must not convert all contained content to another fundamental representation.
Physics is optional per space: static overworld, kinematic elevator, dynamic ship, static interior and
rotating child mechanism may coexist. Only dynamic consumers pay dynamic solver cost.

## Identity, coordinates and commands

Accepted transform chain: local voxel coordinates → voxel space → space transform → parent space
or root-world coordinates. Moving/rotating a structure updates its transform, not every voxel
coordinate. Transform precision and deterministic semantics must support intended scale; concrete
float precision and binary layouts are deferred to measured implementation design.

Target `SpaceId` / `VoxelSpaceId` names are conceptual, not frozen types. Identity must be stable for
persistence, networking, parent-child/entity references, diagnostics and replay/Bot observations
where needed. A temporary vector index cannot be a durable/network identity; no exact binary ID
representation is selected here.

`BlockPos` cannot universally identify a voxel without space context. Conceptually a spatial block
position is `(space identity, local integer block position)`. VS1 migrates block get/set, commands,
queries, raycast, collision, interaction, Agent/Bot context, persistence and rendering extraction
through explicit adapters. Root-space compatibility must be intentional; never guess a space from
coordinates. This pass introduces no types or protocol/schema changes.

## Local simulation and boundary interactions

Voxel adjacency is space-local. Neighbors remain logical neighbors when their whole space moves or
rotates. Motion must preserve block logic, mechanisms, local connections, updates and internal state.
Voxel semantics and space motion are independent concerns.

**Geometric cross-space contact is not logical voxel adjacency.** A ship touching terrain does not
make its blocks neighbors of terrain blocks. Contacts, transfers, attachment and any cross-space
mechanisms require explicit semantic boundary operations. Local lighting likewise stays space-local
by default. Cross-space light transport, if supported later, is an explicit boundary mechanism,
not accidental neighbor lookup.

Entities need reference frames supporting moving platforms/elevators, walking inside ships,
rotating structures and crossing spaces. Contact does not permanently transfer entity ownership to
the contacted space. Combine relative entity movement with space linear and angular motion while
preserving authoritative entity identity. Persistence/network observations identify relevant reference
space. Reference frame, contact support and durable owner are distinct concepts.

## Presentation and geometry

Local section mesh × space transform = presented geometry. Pure motion does not rebuild unchanged
section meshes. Mesh/work identity needs space context, conceptually `SpaceId + local SectionPos`;
culling uses transformed bounds. Renderer knows no vehicle classes.

BG1's shared semantic shape contract feeds gameplay collision, selection and structure collider
aggregation. Render triangles do not automatically become physics geometry. Specialized compiled
representations are allowed when profiling supports them. Full-cube/AABB composition remains a
fast path; detailed visible geometry may use simpler collision/selection shapes.

## Optional composite physics and game policy

Thousands or millions of voxels do not imply one rigid body per block. Default: a voxel structure
contributes to an aggregated/composite physical body; solver-body count is independent of block
count. Articulated structures may use several bodies/spaces without making per-block bodies the base.

C2 definitions and BG1 shapes may supply mass, mass distribution, center of mass, inertia, collision
representation, drag-related data, force-producing capabilities and other typed physical properties.
Content edits dirty aggregate properties for incremental or otherwise bounded recomputation.

Mechanisms operate on forces, torques, contacts, thrust, drag, buoyancy, lift-like effects and other
force providers. Engine code does not know what an engine block, wing, wheel or rocket means and
does not classify car/ship/plane/rocket as fundamental physics types. A structure may combine ground,
flight, water and propulsion effects without changing engine type; the game/mod owns why forces exist.
No physics library, exact realism target or solver API is selected in this documentation pass.

Kinematic spaces are first-class translation, rotation and deterministic scripted/control motion.
Elevators, platforms, large doors, cranes, rotating mechanisms and deterministic transport consume
this model; a separate ElevatorSystem is not the spatial foundation.

## Hierarchy, interaction and lifecycle

A space may be relative to another: root world → ship → rotating turret, elevator or moving platform.
Parent-child transforms compose; child local voxel logic remains independent. Bound depth/resources
and reject cycles. Support independent spaces and evolve the same contracts toward space-vs-world
and space-vs-space collision, landing, temporary physical links, joints, docking and hierarchical
attachment; do not create unrelated one-off transport systems.

Game policy defines connectivity. Architecture permits evaluation and splitting a connected
structure into independently identified spaces, with transforms, motion and local content preserved.
Optional merge/connect operations use explicit identity/revision/ownership transitions. VS1 acceptance
owns foundational split/lifecycle capability before M5; a complete articulation/docking catalog and
all merge gameplay rules are deferred. A split must conserve content and relevant motion state;
failed/retried durable transitions must not lose or duplicate content. Detailed algorithms remain
implementation work, not pseudocode here.

## Persistence and durable transfer

VS1 defines versioned persistence for space identity, parent identity, transform, motion mode/state,
local voxel content, local entities/components as appropriate and structure revisions. Semantic
identities remain authoritative; runtime handles are not saved identities. Current root-column paths
are not sufficient as a universal multi-space namespace. Physical organization remains storage policy;
no backend or save-format change is made now. Existing worlds require explicit compatibility/migration
rules, and legacy root identity must be deterministic during conversion.

Accepted invariant (implemented for present pickups/migrations in D-052): source durable state may
not retire before the destination state needed for recovery is acknowledged. VS1 extends it to
cross-space transfers and split/merge ownership changes. Recovery must be revision-aware and, for
partial content transfers, quantity-aware and idempotent. Destination success precedes source cleanup;
terminal failed jobs retire obsolete transient snapshots but leave authoritative state retryable.
Parent/child publication and lifecycle recovery must not leave dangling durable references.

## Storage and lifetime evidence boundaries

S1's keep-current backend decision remains valid for its measured single-world workload. It did not
evaluate arbitrary moving spaces. VS1 and READY1 re-run [S1 trigger criteria](S1_PERSISTENCE_DECISION.md)
for space count, transform-only updates, local-column counts, split/merge, changing entity ownership
and many small structures. Reopen storage choice only when documented triggers fire; neither VS1
nor this document automatically reopens S1 or requires a database.

RSM1 proves current single-world lifetime only. Extend bounded ownership accounting to resident spaces,
local sections, mesh jobs, GPU resources, physics bodies, pending transforms, persistence jobs and
cross-space references. READY1 proves bounded representative moving-space routes including cancellation,
revisit and lifecycle failures; unsaved data cannot be discarded to force a plateau.

## Performance and acceptance invariants

- Moving an unchanged large space does not rewrite all voxels or remesh all sections.
- Static world path remains low overhead; dynamic costs belong to opted-in spaces.
- Physics-body count is independent of block count.
- Aggregate-property rebuilds are dirty/incremental or otherwise bounded.
- Hierarchy traversal is bounded and acyclic.
- Cross-space broadphase avoids global voxel×voxel comparisons.
- Identity, local logic and content quantities survive motion, split and interrupted persistence.
- Reference-frame movement, generalized shape contact and reopen behave coherently.

No numerical budgets are invented here. VS1 measures static/moving workloads and records results in
its report; READY1 checks them before a separate pass activates M5. [NETWORKING](NETWORKING.md)
requires transform/revision updates rather than N moved-block messages, space-aware interest and
reference-frame prediction. Downloaded mod adapters reuse these semantics within capability/quota
boundaries; no ABI stability is promised before implemented evidence.
