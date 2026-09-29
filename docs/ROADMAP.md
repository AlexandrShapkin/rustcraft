# Roadmap

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

Status: complete. M3 is also accepted; M4 has not started.

- public game-package registry, schedules, controlled mutation commands and `GameProfile` added;
- `minecraft-b173` established as the first-party Game API client and composition boundary;
- active first-party content IDs namespaced;
- independent `sandbox-test` profile validates world/render/input mechanisms without Minecraft;
- remaining legacy runtime/UI/Bot policy leaks recorded in `ARCHITECTURE_AUDIT.md` for incremental
  migration before those areas are extended.

## R1 — resource/render scalability

Status: planned; implementation has not started.

R1 evaluates resource and renderer paths for large packs, many block states/mods, long view
distances and many entities. Exact Beta pixels are not a constraint. Candidates include
mipmapping, anisotropic filtering, texture compression, resource batching/caching, greedy meshing,
GPU-driven submission, stronger culling, LOD, reduced distant animation rates, simplified distant
materials/transparency and dynamic quality.

Every substantial choice must preserve semantic readability and compare measured or justified
frame-time, memory, scalability and extensibility benefits against visual/semantic cost,
complexity and maintenance. R1 must not embed `minecraft_b173` policy in the generic renderer.

## M4 — multiplayer + server content resolution

- authoritative server;
- QUIC evaluation/transport;
- snapshots/deltas/interest management;
- prediction/reconciliation;
- manifest handshake;
- content cache/fetch/integrity;
- headless remote bot transport.

## M5 — third-party modding

- WASM runtime;
- capability API;
- stable versioning;
- example mod;
- target-specific content;
- quotas/profiling.

## M6 — evidence-driven optimization campaign

Evaluate data layouts, storage engines, SIMD, io_uring, allocators, PGO/BOLT and high-player-count
partitioning only against representative benchmarks. R1 resource/render workloads should supply
reusable evidence for this later campaign.
