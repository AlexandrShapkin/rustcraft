# Roadmap

Product release versions use pre-1.0 SemVer and are independent of milestone labels. `M4` does not
imply product version `0.4.0`; a milestone may span prereleases, and a release may contain work that
does not complete a milestone. See `docs/RELEASE.md` for the version source and tag policy.

The project is no longer organized as a strict Beta 1.7.3 clone. `minecraft-b173` is the first-party
game package and compatibility/reference client of a generic voxel runtime. The architecture rule is:

> Engine mechanism; game policy.

Mutable order/state/focus belongs to [stages.toml](stages.toml); detailed future scope belongs to
[stage contracts](stages/INDEX.md). The generated view below is navigation, not a second owner.

<!-- BEGIN GENERATED STAGE PLAN -->

Current focus: **BG1**. Execution order and state are generated from [stages.toml](stages.toml).

| Stage | State | Contract |
| --- | --- | --- |
| F1 | closed | [unified client path & real-hardware field validation](stages/F1.md) |
| A1 | closed | [external identity, policy boundary & trust preparation](stages/A1.md) |
| C2 | closed | [unified content definition & capability model](stages/C2.md) |
| WF1 | closed | [workspace & workflow normalization](stages/WF1.md) |
| BG1 | active | [generalized block geometry & model system](stages/BG1.md) |
| DX2 | planned | [tooling equivalence, workflow convergence & retirement](stages/DX2.md) |
| RF1 | planned | [structural codebase refactor & code-graph optimization](stages/RF1.md) |
| VS1 | planned | [voxel spaces, kinematics & optional composite physics](stages/VS1.md) |
| READY1 | planned | [final pre-M5 readiness gate](stages/READY1.md) |
| M5 | inactive | [multiplayer + server content resolution](stages/M5.md) |
| M6 | inactive | [third-party modding](stages/M6.md) |
| M7 | inactive | [evidence-driven optimization campaign](stages/M7.md) |

<!-- END GENERATED STAGE PLAN -->

Closed stages are not reopened without a reproducible regression. Later stages may update shared
infrastructure, but must preserve the acceptance evidence and contracts of earlier stages.

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

Status: complete.

- public game-package registry, schedules, controlled mutation commands and `GameProfile` added;
- `minecraft-b173` established as the first-party Game API client and composition boundary;
- active first-party content IDs namespaced;
- independent `sandbox-test` profile validates world/render/input mechanisms without Minecraft;
- remaining legacy runtime/UI/Bot policy leaks recorded in `ARCHITECTURE_AUDIT.md` for incremental
  migration before those areas are extended.

## R1 — resource/render scalability

### R1.0 — runtime identity and profile compilation foundation

Status: complete.

- owned validated semantic IDs with typed block/texture/package/resource keys;
- deterministic `GameProfile` compilation by profile package order plus lexical block-key order
  into dense runtime `BlockId` handles and indexed definitions;
- explicit profile default/empty voxel semantics, including nonzero-default engine/render tests;
- domain-separated BLAKE3 content hashes and canonical complete manifest digests;
- generic `TextureHandle`/normalized `AtlasRegion` renderer boundary;
- temporary client-side Beta 16x16 atlas adapter and explicitly supplied renderer resource paths;
- `minecraft-b173` registration without authored numeric IDs and updated independent
  `sandbox-test` compilation path.

Runtime handles are profile-local and are not persistence/network identities. Historical runtime
registry compatibility remains a migration concern only where an active subsystem still depends on it.

### R1.1 — resource loading and atlas compilation

Status: complete.

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
representative profiling identifies a concrete blocker.

### R1.2 — structural renderer scalability

Status: complete. Measurements are recorded in `PERFORMANCE.md`.

- contiguous 18³ section snapshots with dense interior copies and world-looked-up one-cell halo;
- indexed voxel/light meshing and conservative frustum culling at submission;
- bounded immutable CPU meshing jobs with generation rejection, dirty coalescing and upload budget;
- deterministic atlas-page submission and capacity-aware per-section GPU buffers;
- permanent release render-scale, dirty-remesh and camera-motion diagnostics.

Greedy meshing and advanced GPU-driven paths remain profile-triggered backlog items, not follow-on
work in this milestone.

## M4 — world generation & persistence foundation

Status: complete. Representative-hardware quantitative evidence remains conditional where the
available acceptance machine cannot expose the owner's exact GPU/display stack, but no functional
M4 blocker remains.

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
- deterministic, order/worker-count-independent generation and persistence benchmarks;
- dynamic residency with explicit Desired/Retained/Safe/Visible semantics and save-before-evict;
- bounded worker-based bulk initial lighting plus incremental/boundary reconciliation;
- unavailable-column movement/raycast/place/break guards;
- deterministic v1 compatibility and v2 semantic world generation.

Frozen generation identities/hashes and accepted save-format behavior remain compatibility contracts.
Network chunk streaming, Java save/protocol compatibility and broad renderer optimization are not M4
scope.

## DX1 — Developer Control Plane, Debug/Test Tooling & Rhai Scripting

Status: complete and closed.

- shared `RustCraft Control API` for Queries, Commands, Events, Assertions, Jobs, Captures,
  Capabilities and Diagnostics;
- trusted local Rhai runtime/session model with bounded execution and no raw filesystem/network/process access;
- shared command registry for developer console, scenarios, automation and future administration;
- same-source headless and graphical scenarios;
- pause/step/resume, captures and bounded failure bundles;
- explicit developer tools without auto-executing scripts during ordinary startup.

See `DX1_REPORT.md` for canonical evidence.

# Pre-M5 hardening programme

These stages exist to remove known architectural and operational risks before networking makes them
expensive compatibility constraints.

## DUX1 — in-game developer diagnostics

Status: complete and closed.

- semantic diagnostic page/overlay registry with owner/help/availability/cost metadata;
- shared demand-driven providers for DUX, Control, Rhai, scenarios and captures;
- selected chunk/entity inspection with stable semantic identity and persistence/ownership state;
- bounded F3/F4 diagnostic UI and overlays;
- inactive providers perform no collection work.

See `DUX1_REPORT.md`.

## C1 — runtime configuration plane

Status: complete and closed.

- one typed configuration registry and deterministic source precedence;
- requested/effective values and atomic batches;
- runtime policy classes such as NextTick, NextFrame, Reconfigure and RestartRequired;
- native streaming, meshing, lighting, autosave and presentation-related configuration readback;
- persisted configuration remains separate from world saves;
- Control, console, Rhai and Settings share one configuration authority.

See `C1_REPORT.md` and `CONFIGURATION.md`.

## UX1 — developer controls & Unicode text

Status: complete and closed.

- held F3 chord model and semantic debug-page shortcuts;
- command/Rhai console modes, discoverable help and completion;
- grapheme-aware UTF-8 editing and IME event path;
- generic Unicode shaping/layout through bundled resources;
- semantic font roles and deterministic bundled fallback without required system-font discovery;
- bounded glyph/text caches;
- old hard-coded 5×7 text renderer removed.

Known field gap: the normal human startup path has not yet been proven to expose the same developer
input route as the UX1 acceptance harness. This is owned by F1 rather than reopening UX1.

See `UX1_REPORT.md`.

## RSM1 — residency & memory lifetime

Status: complete and closed. PM5-002 resolved.

- shared world -> simulation -> render -> meshing -> GPU lifetime ledger;
- repeated-route, unique-exploration, stalled-consumer and remove/revisit acceptance;
- mesh generation metadata no longer scales with all historically visited sections;
- submitted/completed/ready mesh pressure has explicit bounded ownership;
- stale asynchronous results cannot resurrect revisited render state;
- entity and lighting lifetime leaks found by the campaign were repaired;
- logical GPU ownership/capacity is distinguished from allocator/driver high-water and process RSS.

The owner's historical AMD VRAM growth was not attributable from the software-renderer provider; it
remains a field-validation question, not an open RSM1 correctness blocker.

See `RSM1_REPORT.md`.

## P1 — frame pacing & presentation

Status: complete and closed. PM5-003 resolved.

- explicit authoritative and presentation clocks;
- fixed 20 TPS simulation preserved;
- client-only previous/current translation interpolation;
- pending local mouse look preview with exact authority rebase and no double application;
- teleport/load, pause/step/resume, focus, resize and catch-up rebasing;
- shared bounded Presentation diagnostics;
- frame interval, duplicate-transform, state-age and application input-to-camera evidence.

P1 proved that the major visible low-Hz effect came from repeated 20 Hz camera/player transforms and
mouse look waiting for fixed-tick consumption. It improved visual transform smoothness and
input-camera responsiveness, but did not establish an average-FPS or physical scanout-cadence
improvement. Owner AMD/Vulkan/display validation remains an F1 field task.

See `P1_REPORT.md`.

## S1 — persistence architecture evaluation & scalability

Status: complete and closed. PM5-004 resolved. No S1.3 migration is required for bounded initial M5.

- current checksummed whole-column `.rcc` backend benchmarked at 10k and supplemental 50k scale;
- small voxel and entity-only write amplification measured explicitly;
- queue sustainability, file-count scaling, read/reopen, churn and durability envelope measured;
- crash/recovery ordering, checkpoint fallback and cross-column entity recovery retained;
- split terrain/entity physical layout evaluated as an isolated candidate and rejected for initial M5;
- future durable extension state defined as required game core plus bounded optional namespaced/versioned components;
- backend paths remain host-owned and replaceable behind a durability-aware capability boundary.

Current backend acceptance is bounded, not an unlimited-player guarantee. Reconsideration is triggered
by measured backlog/dirty-age growth, operationally unacceptable entity amplification, or file/read/
backup/shutdown costs outside the benchmarked M5 envelope.

See `S1_REPORT.md` and `S1_PERSISTENCE_DECISION.md`.

## R2 — semantic resource normalization

Status: accepted; implementation and Ubuntu/Windows CI green.

Evidence: implementation `f3909b7b94aa8c86eae2c98588384976ea296225`; independent persistence
repair `a3a1b4dcc136bafe558cc49729e2662352e76260`. See [R2 acceptance report](R2_REPORT.md)
and [implementation CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37325633913).
Semantic layout/page, cache, sandbox and release-policy acceptance passed; ARCH-002 is narrowed
to the remaining destination-layout/game-policy boundary.

Owner: PM5-006 and remaining active historical source-sheet layout coupling.

Goals:

- active HUD, inventory and player presentation consume semantic subresources instead of historical
  whole-sheet runtime contracts;
- Beta sheet dimensions/crop coordinates are confined to `minecraft-b173` legacy import mapping and
  historical/reference tests;
- generic renderer consumes resolved semantic `AtlasRegion`/presentation descriptors and remains
  independent of `gui.png`, `inventory.png`, `char.png`, source coordinates and original atlas layout;
- semantic resources render identically when imported from rearranged sheets, individual files,
  package overrides or different compiled atlas pages;
- accepted M3 hotbar/inventory/player-preview behavior remains visually and interactively unchanged;
- public tests and release artifacts require no proprietary historical pixels;
- ARCH-002 is closed or narrowed to an explicit remaining game-policy boundary.

R2 must not become a general GUI framework, a full resource-pack UX, or a renderer optimization
campaign.

## Future implementation contracts

See the generated plan above and [stage index](stages/INDEX.md). Completed sections here and linked
reports preserve historical acceptance; later changes require concrete regression evidence. Maintenance
batches do not enter the product sequence. To change planning, edit the registry/contract and run
`just docs-sync`, then `just docs-check`.
