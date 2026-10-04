# Roadmap

Product release versions use pre-1.0 SemVer and are independent of milestone labels. `M4` does not
imply product version `0.4.0`; a milestone may span prereleases, and a release may contain work that
does not complete a milestone. See `docs/RELEASE.md` for the version source and tag policy.

## M0 — foundation / first headless vertical slice

Status: complete.

- typed world/chunk/block primitives;
- multi-chunk world container;
- block registry and first-party gameplay registration;
- flat-world module;
- semantic agent/controller intent;
- basic movement/AABB/voxel collision;
- block break/place path;
- initial Bot API observations/actions;
- initial content manifest/package/target/hash model;
- deterministic headless scenario;
- focused contract tests.

## M1 — local playable client

Status: manually accepted by the owner; bounded M1.1 verification/audit passed and the milestone is
closed. Do not reopen without a real regression. See `RENDER_DIAGNOSTICS.md`.

- renderer/window loop;
- input adapter -> agent intent;
- chunk meshing/culling;
- texture/resource loading;
- camera;
- local world streaming;
- basic save/load abstraction.

## M2 — playable sandbox foundation and F3 telemetry

Status: accepted and closed by the owner. Do not reopen without a real regression. Automated,
headless and hardware validation is recorded in `M2_VALIDATION.md`. M1.1 remains closed.

- 16 semantic block definitions, 15 item definitions and compact stored block states;
- authoritative inventory, nine-slot hotbar, selected-item placement and Bot API v2 observations;
- DDA target/face traversal, inventory-aware placement and gameplay breaking actions;
- crosshair, target outline, pixel hotbar/icons/counts and project-owned text;
- packed skylight/block light, incremental propagation and localized mesh invalidation;
- authored flat sandbox building area with glass, leaves and a temporary emissive cube;
- F3 frame/TPS/tick/world/mesh/process/renderer metrics and optional real GPU telemetry;
- bounded fixed-step catch-up and a repeatable `just bench-m2` CPU workload.

Survival acquisition, crafting, tools, drops, fluids, procedural terrain and day/night are not M2
features. No M3 implementation was started in this batch.

## M3 — survival/gameplay

Status: complete and manually accepted by the owner. Closed unless a concrete regression is
reported.

- survival modes, drops, pickup and inventory/crafting foundation;
- tools, timed mining and durability;
- repaired canonical block geometry and GUI presentation;
- permanent offscreen block/item/state diagnostics;
- compact semantic orientation foundation only (no directional-block gameplay).

Mobs, combat/health, furnaces, farming, weather and redstone are not implemented M3 features.
See `M3_8_VALIDATION.md` for historical repair evidence. Owner acceptance confirmed dropped block
orientation/animation, GUI items, inventory operations, held mining/cracks, unbreakable behavior
and input/cursor transitions.

### Architecture alignment gate before M4

Status: complete. M3 and R1.2 are accepted; M4 is functionally complete (hardware evidence conditional).

- public game-package registry, schedules, controlled mutation commands and `GameProfile` added;
- `minecraft-b173` established as the first-party Game API client and composition boundary;
- active first-party content IDs namespaced;
- independent `sandbox-test` profile validates world/render/input mechanisms without Minecraft;
- remaining legacy runtime/UI/Bot policy leaks recorded in `ARCHITECTURE_AUDIT.md` for incremental
  migration before those areas are extended.

## R1 — resource/render scalability

### R1.0 — runtime identity and profile compilation foundation

Status: complete; R1.1 and R1.2 are complete; M4 is functionally complete (hardware evidence conditional).

- owned validated semantic IDs with typed block/texture/package/resource keys;
- deterministic `GameProfile` compilation by profile package order plus lexical block-key order
  into dense runtime `BlockId` handles and indexed definitions;
- explicit profile default/empty voxel semantics, including nonzero-default engine/render tests;
- domain-separated BLAKE3 content hashes and canonical complete manifest digests;
- generic `TextureHandle`/normalized `AtlasRegion` renderer boundary;
- temporary client-side Beta 16x16 atlas adapter and explicitly supplied renderer resource paths;
- `minecraft-b173` registration without authored numeric IDs and updated independent
  `sandbox-test` compilation path.

Runtime handles are profile-local and are not persistence/network identities. The legacy M0-M3
runtime registry remains an explicit semantic-to-legacy compatibility adapter until affected gameplay policy slices
migrate.

### R1.1 — resource loading and atlas compilation

Status: complete. Full validation passed on 2026-10-02. R1.2 is complete; M4 is functionally complete (hardware evidence conditional).

- generic package discovery and deterministic later-package texture overrides;
- semantic texture registry and direct indexed voxel render registry;
- first-party Beta virtual crop importer outside generic engine/renderer code;
- bounded parallel PNG decode with aggregate unique-source allocation preflight;
- deterministic padded/extruded shelf packing with adaptive power-of-two page size;
- multi-page chunk, dropped-block and GUI/hotbar/cursor batching;
- BLAKE3 compiled cache with corruption rebuild, unique atomic temporary writes and diagnostics;
- independent textured sandbox package plus mod override proof;
- report, inspect, atlas-debug and 1,000-resource stress tooling;
- device-safe configured page maximum plus graceful renderer adapter-limit validation;
- nearest/uniform-linear page sampler propagation; mipmaps fixed at one level and anisotropy at one.

Atlas-safe mip chains, anisotropic filtering, texture compression, a stronger packer,
bindless/texture arrays, further draw-call reduction, greedy meshing, Hi-Z, indirect/GPU-driven
rendering and LOD are profile-triggered backlog items, not an active R1.3 milestone. Reopen only if
M4 discovers a concrete measured renderer blocker.

### R1.2 — structural renderer scalability

Status: complete. Full validation passed; measurements are recorded in `PERFORMANCE.md`.

- contiguous 18³ section snapshots with dense interior copies and world-looked-up one-cell halo;
- indexed voxel/light meshing and conservative frustum culling at submission;
- bounded immutable CPU meshing jobs with generation rejection, dirty coalescing and upload budget;
- deterministic atlas-page submission and capacity-aware per-section GPU buffers;
- permanent release render-scale, dirty-remesh and camera-motion diagnostics.

Greedy meshing and advanced GPU-driven paths remain profile-triggered backlog items, not follow-on
work in this milestone.

## M4 — world generation & persistence foundation

Status: functionally complete. Generic storage, generator contracts, first-party generation, headless round-trip,
and the first local-player residency/streaming slice are implemented. M4-001's synchronous startup
barrier is closed. Autonomous production-controller and actual-client travel now close M4-002 and
M4-010: a normal-speed route crosses straight, turn, diagonal, reverse and negative-coordinate
frontiers with nonzero Safe/Visible margin, bounded residency, persistence/revisit, continuous view
intent and no streaming-induced dropped ticks. M4-011 remains closed. M4-009 scheduler/correctness
acceptance is closed with sub-second warm p95 request-to-visible; hardware frame-performance remains
conditional because this pass exposed only llvmpipe/GL, not AMD/Vulkan. M4-003 is closed by
versioned dropped-item and world-global persistence, ordered recovery tests, entity save/evict/
reload travel coverage and two-slot clock checkpoints. M4-004 is closed by versioned overworld v2,
autonomous v1/v2 fresh/reused travel, two actual-client seed routes, full local validation and
[Ubuntu/Windows CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37135831666).
Only representative-hardware quantitative evidence remains conditional under M4-009. The older
manual water-presentation walkthrough is not claimed; the functional repair is covered by liquid
geometry/culling, compiled-medium boundary and distance-fog tests. That optional qualitative
follow-up is not a new functional blocker. M5 and later milestones remain inactive.

- generic chunk/section lifecycle and deterministic generation contracts;
- game-owned generation policy, with Beta-like terrain policy confined to `minecraft-b173`;
- versioned semantic chunk/world persistence and local filesystem storage;
- generic player-record envelopes with game-owned versioned durable player codecs;
- component-versioned player records with opaque unknown-component preservation and two-slot crash recovery;
- semantic item-stack persistence and startup neighborhoods centered on restored player position;
- chunk-v3 spatial entity envelopes with stable identity, game-owned codecs and v2 migration;
- two-slot world-global components with a durable paused-offline simulation clock;
- ordered pickup and cross-column entity recovery with bounded receipts/tombstones;
- bounded asynchronous generation/load/save with explicit dirty/save generations;
- client and headless create/generate/edit/save/reopen workflows;
- deterministic, order/worker-count-independent generation and persistence benchmarks.

Current baseline: `rustcraft-world` has versioned/checksummed semantic chunk and player-record files,
bounded load/generation/save worker pools, stale request/result and save rejection, compression
metrics, and a portable filesystem backend. Frozen Minecraft generation version 1 plus new-world
default version 2, semantic local-player
persistence and named client create/open are integrated. Static liquid presentation includes
compiled-medium underwater fog. Client startup now assembles its minimum safe neighborhood on a
cancellable background worker while the window remains responsive; the ready neighborhood is
centered on the restored player. Runtime residency uses a generic interest controller (diagnostic
default load radius 4, retain radius 5; configurable with `RUSTCRAFT_STREAM_RADIUS=3..12`), async disk load/generation, staged per-turn result application and resumable boundary lighting, an unavailable-column movement/raycast guard, save-before-evict, and renderer/mesh
removal on eviction. Newly arrived columns now use a bounded worker-based bulk initial-lighting
stage and publish voxel/light arrays atomically; only neighbor-boundary reconciliation and
incremental edits remain on the lighting path. Boundary reconciliation advances in repeated
32-unit slices. Critical apply, boundary, snapshot and mesh submission now receive ordered reserved
windows within the 2 ms default budget; near REQUIRED/VISIBLE sections precede PREFETCH sections,
and completed meshes are uploaded nearest-camera first. The canonical fixed-
region worldgen hash remains unchanged. Headless streaming stress and edited-column revisit tests
pass. Desired/Retained use predictable Chebyshev squares; a connected complete 3x3 Safe+Visible core
gates control, `SAFE => VISIBLE`, and eventual boundary-light work cannot redefine that frontier.
Hardware-specific M4-009 performance evidence remains conditional. Dropped-entity/world-time
persistence is accepted by `world-state-roundtrip`, codec/corruption/recovery tests and the
entity-bearing long-travel route. M4-004 now adds smooth climate, six meaningful derived biomes,
continental/rolling/hill terrain, contextual coasts and surfaces, curved caves, depth-bounded ores,
biome-aware trees and deterministic static water lakes. Exact version resolution keeps v1 worlds
v1, including missing-column expansion; no automatic upgrade or Java seed parity is promised.
First-time version-specific safe spawn handles ocean origins; saved players are never relocated.
Lava, springs and dungeons remain explicit later first-party content scope, not placeholder
features. M4-004's final autonomous acceptance and public CI are recorded in `docs/PERFORMANCE.md`.

The older multiplayer/content-resolution scope is moved to an inactive later roadmap item. Network
chunk streaming, procedural features in engine storage code, raw persisted `BlockId` values and
advanced renderer optimization are out of M4 scope.

## DX1 — Developer Control Plane, Debug/Test Tooling & Rhai Scripting (CLOSED)

Status: CLOSED.

Generic control and bounded Rhai sessions/scenarios now have shared headless/graphical composition.
Same-source smoke, actual console, cooperative reload/jobs/cancellation, automatic failure capture,
bounded lifecycle/overhead and the full regression matrix pass. [Ubuntu/Windows CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37156846278)
is green; see DX1_REPORT.md for canonical evidence. M4 remains complete with M4-009 conditional.

## Pre-M5 technical consolidation

Accepted planning baseline: public main `16c6823a739e83f830b6676bbe0390be769ca17e`,
2026-10-04; [closeout Ubuntu/Windows CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37157185442)
passed. M0–M4 and DX1 remain closed; M4-009 representative-hardware quantitative evidence remains
conditional. The audit remains the accepted historical plan. DUX1 is CLOSED with local and Ubuntu/Windows acceptance recorded in [DUX1_REPORT.md](DUX1_REPORT.md); C1 and later stages remain inactive.

The canonical [PRE_M5_AUDIT.md](PRE_M5_AUDIT.md) contains source evidence, severity/disposition,
ownership map, dependencies and executable stage contracts (goal, scope, invariants, measurements,
acceptance, deferrals and regression gates). Activate one named bounded sub-slice per future pass;
do not treat the sequence as authorization to implement everything automatically.

| Order | Stage | Bounded result / closure evidence |
| --- | --- | --- |
| 1 | DUX1 — In-game developer diagnostics (CLOSED) | Metadata/selector and demand-driven shared DX1 snapshots, then targeted chunk/EntityId inspection; graphical selection/toggle/cost proof. Existing console pages/overlays stay usable. |
| 2 | C1 — Runtime configuration plane | Shared typed registry, source/validation/change policy and safe existing operational controls; Control/Rhai/developer UI share effective readback. Structural settings remain immutable/restart-bound. |
| 3 | RSM1 — Residency and memory lifetime | Ownership ledger and repeated A→B→C→D→A/unique-exploration/stale-revisit tests; logical counts/bytes plateau, safe metadata retirement and bounded pressure. Driver VRAM need not return to startup. |
| 4 | P1 — Frame pacing and presentation | Separate authoritative/render/request/present-call cadence, refresh/provider evidence and distributions; bounded scheduling/interpolation/input change only after diagnosis. High average FPS is not acceptance. |
| 5 | S1 — Persistence architecture evaluation and scalability | Benchmark existing storage and justified candidate spikes first; ADR accepts backend limits/component/recovery boundaries or names a separate bounded migration. No automatic database adoption. |
| 6 | R2 — Semantic resource normalization | Active HUD/inventory/player subresources lose source-sheet dependence; retain game-owned Beta importer and generic compiler; synthetic rearranged/multipage proof. Block crops already normalized. |
| 7 | A1 — Network-facing ownership and API consolidation | A1.1 semantic/stable external references and trusted-source adapter contract; A1.2 behavior-preserving migration of replication-touched game policy through public Game API. No all-API merger or ECS rewrite. |
| 8 | DX2 — Repository and legacy workflow retirement | Inventory/equivalence before deleting drivers/aliases; stable just/test tiers/output paths/release policy coherent; preserve specialist correctness coverage. |
| 9 | READY1 — Pre-M5 readiness review | Linked closure/baseline evidence, explicit medium-risk waivers, functional regression and Ubuntu/Windows CI; activate M5 only in a separate pass. |

Dependency reasoning: DUX1 exposes existing diagnostic leverage without waiting for a settings UI;
C1 prevents later experiments from proliferating flags. RSM1 stabilizes workload ownership before
P1 timing; P1 separates presentation from the clock future prediction will consume. S1 decides
storage/recovery/component limits before network commitments. R2 removes active layout coupling
before A1 settles future-facing references; those two investigations can otherwise proceed
independently. DX2 retirement follows proven replacement equivalence. A1.1 semantic/provenance
rules apply to all new contracts immediately, even before its consolidation gate.

M5 entry requires closure of the audit's PM5-001–006 mandatory findings, completion or explicit
rationale/trigger for MEDIUM items, generic/game direction and headless server intact, stable semantic
external identities, consciously accepted persistence backend, bounded residency, understood pacing,
normalized active resources, controlled configuration, shared automatic/in-game diagnostics with
bounded cost, host-owned admin capability provenance, coherent regression/tooling/docs and green
platform CI. See the audit's exact checklist. This does not require speculative perfection or claim
M4-009 representative-hardware evidence has been obtained.

Deferred: general ECS/renderer rewrites, exotic M7 optimization, distributed storage, full WASM/mod
UI, polished production GUI/settings, full scripting debugger, unlimited commands, inactive legacy
asset classes and mechanical dependency updates. Milestone history and frozen M4 hashes stay intact.

## M5 — multiplayer + server content resolution (future, inactive)

- authoritative server;
- QUIC evaluation/transport;
- snapshots/deltas/interest management;
- prediction/reconciliation;
- manifest handshake;
- content cache/fetch/integrity;
- headless remote bot transport.

## M6 — third-party modding (future, inactive)

- WASM runtime;
- capability API;
- stable versioning;
- example mod;
- target-specific content;
- quotas/profiling.

## M7 — evidence-driven optimization campaign (future, inactive)

Evaluate data layouts, storage engines, SIMD, io_uring, allocators, PGO/BOLT and high-player-count
partitioning only against representative benchmarks. R1 resource/render workloads should supply
reusable evidence for this later campaign.
