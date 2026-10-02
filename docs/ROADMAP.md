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

Status: complete. M3 and R1.2 are accepted; M4 is active.

- public game-package registry, schedules, controlled mutation commands and `GameProfile` added;
- `minecraft-b173` established as the first-party Game API client and composition boundary;
- active first-party content IDs namespaced;
- independent `sandbox-test` profile validates world/render/input mechanisms without Minecraft;
- remaining legacy runtime/UI/Bot policy leaks recorded in `ARCHITECTURE_AUDIT.md` for incremental
  migration before those areas are extended.

## R1 — resource/render scalability

### R1.0 — runtime identity and profile compilation foundation

Status: complete; R1.1 and R1.2 are complete; M4 is active.

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

Status: complete. Full validation passed on 2026-10-02. R1.2 is complete; M4 is active.

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

Status: active; generic storage, generator contracts, first-party generation and headless
round-trip are under implementation. The normal client startup path is integrated but not yet fully
non-blocking, and the full validation matrix remains outstanding.

- generic chunk/section lifecycle and deterministic generation contracts;
- game-owned generation policy, with Beta-like terrain policy confined to `minecraft-b173`;
- versioned semantic chunk/world persistence and local filesystem storage;
- generic player-record envelopes with game-owned versioned durable player codecs;
- component-versioned player records with opaque unknown-component preservation and two-slot crash recovery;
- semantic item-stack persistence and startup neighborhoods centered on restored player position;
- bounded asynchronous generation/load/save with explicit dirty/save generations;
- client and headless create/generate/edit/save/reopen workflows;
- deterministic, order/worker-count-independent generation and persistence benchmarks.

Current baseline: `rustcraft-world` has versioned/checksummed semantic chunk and player-record files,
bounded generation/save worker pools, stale generation/save rejection, compression metrics and a
portable filesystem backend. Minecraft generation version 1, semantic local-player persistence and
named client create/open are integrated. Static liquid presentation includes compiled-medium
underwater fog. Initial client startup still blocks while the nearby area is assembled; runtime
generation as the player moves, eviction, a bounded asynchronous load pool, interactive acceptance
and complete M4 validation remain required.

The older multiplayer/content-resolution scope is moved to an inactive later roadmap item. Network
chunk streaming, procedural features in engine storage code, raw persisted `BlockId` values and
advanced renderer optimization are out of M4 scope.

## M5 — multiplayer + server content resolution (future, inactive)

- authoritative server;
- QUIC evaluation/transport;
- snapshots/deltas/interest management;
- prediction/reconciliation;
- manifest handshake;
- content cache/fetch/integrity;
- headless remote bot transport.

## M6 — third-party modding

- WASM runtime;
- capability API;
- stable versioning;
- example mod;
- target-specific content;
- quotas/profiling.

## M7 — evidence-driven optimization campaign

Evaluate data layouts, storage engines, SIMD, io_uring, allocators, PGO/BOLT and high-player-count
partitioning only against representative benchmarks. R1 resource/render workloads should supply
reusable evidence for this later campaign.
