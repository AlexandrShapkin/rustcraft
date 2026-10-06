# BG1 report

## Baseline and scope

Owner-authorized baseline `8af0f161e470b82943646da62d0053d15b3cc92a`, branch main,
clean at entry, in `/home/user/Projects/rustcraft`. The owner's skill-metadata commit is
preserved. No other checkout/worktree was used. BG1 implements generic static voxel geometry;
DX2, RF1, VS1, M5 and M6 were not started. C2's schema/codec and historical reports are preserved.

## Dependency map recorded before implementation

- Game API authored/compiled voxel definitions: Empty/FullCube collision, six material roles,
  scalar light opacity, C2 definition-local state and orientation.
- Render-profile: indexed definitions implement the renderer resolver and resolve BlockModel.
- Chunk meshing: six canonical quads, opaque/same-translucent whole-face neighbor culling;
  padded immutable snapshots retain neighbor states/light across section boundaries.
- Inspector, GUI and dropped items: canonical cube geometry through BlockModel.
- Engine-core collision: solid cells block actor AABBs; axis-ordered movement and bounded binary
  search are shared by players and dropped entities.
- Engine-core selection: DDA accepted a boolean occupied-cell target.
- Runtime: semantic profile binds retained registry IDs; placement uses adjacency/actor overlap.
- Lighting: game-authored scalar opacity through registry, independently of rendering.
- Sandbox: public Game API/content/resources/render-profile/runtime, initially cube-only content.

## Model/shape architecture and identity

Authored ModelKey/ShapeKey reuse NamespacedId and package ownership. Selected profile resources
compile in lexical semantic-key order into immutable shared catalogs, addressed by u32
ModelHandle/ShapeHandle. Handles are deterministic within a catalog and local to that profile;
adding an earlier semantic key shifts handles without changing restored semantic block/state,
selection behavior or the semantic fingerprint. Only referenced semantic geometry, including guaranteed coverage masks, contributes to
that fingerprint. There is no duplicate ID system or durable mesh/GPU handle.

GeometryBinding chooses independent render model, collision shape, selection shape, orthogonal
transform and light coverage. Checked C2 state predicates compile to shared indexed entries;
invalid/forbidden states fail deterministically. No per-voxel property map or geometry allocation:
BlockState remains 8 bytes, canonical variant u16; model/shape handles are 4 bytes. Each optional resolved state entry is 24 bytes on this build. Models share
compiled triangle arrays, shapes share compiled selection edges. Catalogs are bounded to 4096
models/shapes, geometry-bound schemas to 4096 state entries and 128 rules.

## Primitive, compound and static geometry

Engine-core owns bounded local [0,1] Box and real triangular-prism Wedge primitives, independent
of renderer internals. Compounds contain at most 32 primitives. Bounds must be finite and strictly
ordered. Box unions subdivide/remove fully hidden shared faces at compilation. Mixed box/wedge
unions remove a box patch only when all corners prove coverage; no midpoint-only deletion.
Wedge faces remain conservative where clipping would be required.

BoundedStaticMesh uses indexed vertices with UVs and material-face roles. Compilation checks
finite bounded positions/UVs, valid indices/roles, nondegenerate triangles and limits of 128
triangles/384 vertices. Authored general meshes and generated compound output have the same
triangle budget. Static surface shapes support exact selection; physical shapes use independent
closed primitive volumes. No streaming, runtime deformation or mesh mutation is introduced.

## Full-cube path and presentation

Compiled FullCube has no general triangle array. Existing terrain takes the six fixed canonical
quads, direct neighbor-face culling, original UV orientation, atlas/page batches, tint and lighting.
General emission borrows shared triangles and transforms positions/normals/material-facing data;
no game-ID or named-content-shape switch is present. Neighbor states in padded snapshots preserve
boundary behavior. BlockModel shares geometry with world, inspection, dropped-item and GUI paths.
Selection outlines use independent shape edges; cracks follow actual rendered geometry.

## Collision, selection and transforms

World AABB movement delegates local overlap queries to profile-resolved shapes; the established
axis order/binary search remains. Boxes and compounds block their actual volumes. Wedges use
convex plane/box separation under exact orthogonal rotations rather than stacked cubes or a
full-cell approximation. The actor slope test stops at the expected height 0.6.

Selection keeps voxel DDA and intersects each candidate's independent shape. Full cubes use
convex box intersection, compounds nearest primitive hits, meshes nearest triangle hits.
Hits retain cell, distance, world/local point and surface normal. Placement maps the dominant
normal axis to adjacent cell with deterministic X/Y/Z tie order, and actor overlap checks the
placed state's actual collision shape. A 1e-5 cell-entry tolerance prevents oblique boundary
rounding from skipping an otherwise valid hit; a successive-ground-removal regression covers it.

The existing compact proper orthogonal ModelRotation composes authored and C2-facing transforms.
Render/collision/selection/coverage share that transform. Six facing orientations have exact
inverse/compose tests. No moving spaces or arbitrary animated transforms are implemented.

## Coverage and light policy

Models compile per-face None/Partial/Full plus conservative 4x4 u16 boundary masks. A tile is
covered only when its whole rectangle is covered by box unions; wedges contribute only their
full rectangular boundary faces. Static meshes advertise no guaranteed coverage. General
boundary triangles are removed only when the neighbor mask covers their whole bounding rectangle.
Full cubes retain the cheapest whole-face path. Transparency/cutout remains separate from
geometric coverage. No per-pair polygon clipping is used.

The partial-neighbor tests preserve an exposed full-cube face beside a half-height box, remove
shared same-half surfaces across x=15/16 (60 indices), and preserve opposite-half surfaces
(72 indices). Sub-tile and irregular coverage can retain hidden surfaces; it cannot claim
unsupported complete coverage. Mixed/wedge interiors may retain conservative overdraw.

LightCoverage Full/Partial/None/Transmitting caps existing authored scalar attenuation at
15/8/0/1 respectively, preserving emission. Ordinary legacy blocks keep their authored lighting
unchanged. Existing flood/sky algorithms consume state-resolved scalar values; this is coarse
cell transport, not arbitrary-mesh GI. Powered sandbox content proves full-to-partial attenuation.

## State and independent proof content

Sandbox registers semantic models/shapes through public Game API only, independently of
minecraft-b173 and renderer-private APIs. Its compact proof set includes semantic FullCube,
lower/upper plate (half), straight/inner/outer box assembly (shape/facing), real 45-degree incline
(facing), connection-dependent multipart frame (connections), tetrahedron static mesh with
independent box collision/exact surface selection, and reactor whose powered state selects
cube versus multipart geometry. The existing Use policy also updates facing.

Tests exercise actual model, collision and selection consumers for these properties. All six
wedge rotations are checked at x=-17,-16,-1,0,15,16 and section y=16, including normals, visible
triangles, gap ray traversal, distance/local point, partial collision and rotated coverage.
The existing chunk codec roundtrip reopens powered state under reordered profiles; separate
catalog insertion proves local handles do not become persistent identity.

`just sample-game` creates `target/sample-game/bg1-geometry.png` using generated project textures,
12 proof objects, multiple atlas pages and backface culling. Visual inspection confirmed distinct
halves, compound forms, multipart frame, tetrahedron and four visible wedge rotations. Output
remains ignored. The complete `just render-test-all` suite passed using the existing generated
Minecraft test fixture (all required terrain/GUI/skin resources); the default resource location
was absent. These synthetic captures are not proprietary assets. Adapter was llvmpipe/GL;
this establishes geometry/presentation proof, not owner-GPU performance.

## Cube-heavy performance comparison

Existing release `just render-scale` is the workload. Baseline active source was restored from
8af0f161 for its build in this same checkout, then BG1 edits restored; original release binary was
retained for five paired, alternating before/after runs. Both binaries ran the same four existing
scenes through camera-motion checks. No new benchmark checkout or workload was invented.

The original async stress harness waited for all completions without draining its bounded ready
queue and timed out after scene collection. A narrow harness repair drains results with the
existing two-section/2 MiB poll budget while waiting. The complete BG1 command now passes:
108 dirty requests, 9 unique sections, 10 submitted/completed, 1 stale discard, 99 coalesced,
9 uploads, and zero pending/inflight jobs. Camera movement alone rebuilds no geometry.

Five-run timings below are milliseconds, median (min–max), on the shared container:

| Cube scene | Before extraction | After extraction | Before meshing | After meshing |
| --- | --- | --- | --- | --- |
| small | 1.803 (1.102–2.161) | 0.865 (0.793–1.733) | 2.265 (1.801–3.634) | 1.436 (1.319–2.392) |
| medium | 10.049 (8.500–16.304) | 8.052 (7.779–10.375) | 9.517 (7.008–11.582) | 7.332 (7.125–11.998) |
| large | 34.976 (31.799–37.764) | 30.954 (30.197–40.579) | 26.712 (24.236–27.997) | 27.018 (24.655–36.807) |
| exposed | 0.602 (0.569–0.733) | 0.624 (0.564–0.747) | 30.736 (29.167–32.318) | 30.005 (29.865–36.617) |

A second five-pair final-source run and a 20-pair unpinned control exposed substantial scheduling/
frequency variation: in the latter large meshing medians were 29.164 → 35.269 ms and exposed
36.230 → 37.120 ms. Those results were investigated rather than discarded. `nm`/normalized
`objdump` comparison of SyntheticTextures' compiled section mesher found identical instruction
sequences, 2623 bytes and 525 instructions in both binaries: general geometry hooks compile out
of that cube workload. Relative code/data addresses were normalized for this comparison.

The final controlled comparison pins both binaries to CPU 0, alternates order, and repeats 20
pairs using the identical existing four scenes. Hardware: AMD Ryzen 5 PRO 2500U, x86_64 Linux,
8 logical CPUs; rustc 1.98.0, release build, CARGO_BUILD_JOBS=2. No global CPU/frequency setting was
changed. Final-source results, milliseconds median (min–max):

| Cube scene | Before extraction | After extraction | Before meshing | After meshing |
| --- | --- | --- | --- | --- |
| small | 0.937 (0.776–1.618) | 0.899 (0.786–1.387) | 1.305 (1.060–2.876) | 1.451 (1.040–2.191) |
| medium | 9.997 (7.987–14.648) | 9.254 (7.886–12.588) | 9.579 (7.260–17.119) | 9.305 (7.814–12.467) |
| large | 35.517 (30.935–44.777) | 35.046 (31.840–47.861) | 34.211 (26.610–55.115) | 34.404 (29.794–47.395) |
| exposed | 0.692 (0.570–1.220) | 0.638 (0.582–1.363) | 36.287 (30.811–50.754) | 36.128 (32.666–46.853) |

The controlled comparison and equal compiled cube path show no stable material cube-workload
regression. Unpinned timing differences are retained above; improvements are not claimed. The full
final-source render-scale command also passes camera-motion, dirty stress and dirty-remesh checks.

All scene vertex/index/triangle counts, snapshot bytes, mesh bytes and page batches match baseline.
Large: 289 sections, 596224 vertices, 894336 indices, 15169032 snapshot bytes,
25041408 CPU mesh bytes and 289 page batches. Exposed: 9 sections, 442368 vertices,
663552 indices, 472392 snapshot bytes, 18579456 CPU mesh bytes and 18 page batches.
Ranges overlap and show no material cube meshing regression; no speedup or FPS claim is made.
Allocation counts were not instrumented. Observed cube payloads remain identical; shared catalogs
and indexed state tables add bounded profile memory, while scheduler worst-case result accounting
now permits general geometry's bounded triangle count. General geometry allocates no per-voxel
triangle arrays. The benchmark's synthetic resolver does not measure complete game-profile/GPU cost.

## Compatibility and validation

Persistence remains semantic block key plus canonical state. No world format, GPU/runtime-handle
serialization, Minecraft generation algorithm or frozen hash was changed. Wider tests retain
semantic codec roundtrips, profile reorder and canonical worldgen hash expectations.

Focused engine-core/Game API/render/render-profile/runtime/sandbox tests pass. Wider
`just sample-game`, `just smoke`, `just test`, `just ci` and configured `just render-test-all`
pass with sequential Cargo work and CARGO_BUILD_JOBS=2 (360 workspace tests passed, 2 skipped).
The final local CI rerun also includes the coverage-fingerprint regression. Publication is blocked
by the Git metadata mount, as recorded below; public platform acceptance has not occurred.

## Known deferred limitations

- Conservative 4x4 coverage and whole-triangle rejection; no arbitrary neighbor polygon clipping,
  full CSG or exact irregular hidden-surface elimination.
- Static mesh physical volumes use explicitly authored box/wedge proxies; no triangle rigid bodies.
- Existing AABB axis movement remains: no automatic stair stepping, continuous collision detection,
  arbitrary slope-following locomotion or rigid-body physics.
- Coarse scalar cell lighting, no precise partial-volume sunlight/GI.
- Native static content registration; no new model-file importer, dynamic mesh mutation or streaming.
- Bounded authored complexity/state tables and proper orthogonal rotations; no moving voxel spaces,
  reflection policy, CAD, skeletal animation, fluids, networking or transport expansion.

## Initial publication blocker (historical)

No implementation or closeout commit could be created in this session. `git add` failed with
`Unable to create '/home/user/Projects/rustcraft/.git/index.lock': Read-only file system`.
Source edits remain in the authorized worktree; skill files have no diff. The environment permits
source writes but not Git metadata writes. No filesystem restriction was bypassed.

Local HEAD remains `8af0f161e470b82943646da62d0053d15b3cc92a`. No push or BG1 public Ubuntu/Windows
CI exists, so this report is local implementation evidence, not accepted stage closeout. Issue #15
remains open without a completion comment; BG1 remains active, focus BG1, DX2 planned. The worktree
contains only this session's known BG1 changes and is intentionally not reported clean. Accepted
EVIDENCE_INDEX reconciliation and registry closeout await actual publication/both-platform CI.
DX2/RF1/VS1/M5/M6 were not started. Publication must resume in this same checkout with Git metadata
writable; implementation uses Refs #15, followed by both-platform acceptance, issue evidence/closure,
registry/docs closeout, non-force push and final CI. No auto-close keyword is authorized before that
acceptance.

## Recovery publication

The owner authorized a recovery/publish pass from this same worktree and exact public baseline
`8af0f161e470b82943646da62d0053d15b3cc92a`. The existing implementation was preserved without
reset, stash or source changes. Recovery checks confirmed the checkout/HEAD/remote baseline,
clean whitespace and synchronized documentation; the diff and this report were reviewed against
the BG1 contract and #15 residual acceptance. Recorded final-source local gates above remain the
local evidence; expensive render-scale/workspace/CI runs were not repeated without a source change.
Implementation publication precedes public Ubuntu/Windows acceptance and issue/registry closeout.
