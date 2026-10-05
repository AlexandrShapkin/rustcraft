# Roadmap

Product release versions use pre-1.0 SemVer and are independent of milestone labels. `M4` does not
imply product version `0.4.0`; a milestone may span prereleases, and a release may contain work that
does not complete a milestone. See `docs/RELEASE.md` for the version source and tag policy.

The project is no longer organized as a strict Beta 1.7.3 clone. `minecraft-b173` is the first-party
game package and compatibility/reference client of a generic voxel runtime. The architecture rule is:

> Engine mechanism; game policy.

The current pre-M5 sequence is:

`R2 -> F1 -> A1 -> C2 -> BG1 -> DX2 -> RF1 -> READY1 -> M5`

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

Status: next planned stage; not started.

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

## F1 — unified client path & real-hardware field validation

Status: planned; starts only after R2 closes.

Purpose: close gaps that synthetic/milestone acceptance paths can miss by validating the exact human
workflow and converging ordinary client startup onto one runtime path.

Principles:

- one `rustcraft-client` executable and one normal client runtime path;
- gameplay modes such as Survival/Creative are game-owned player/session state, not different client
  executables or fundamentally different startup paths;
- roles grant capabilities; capabilities authorize developer/admin actions;
- F3/F4 input routing exists in the normal client path and capability checks decide whether actions are
  available;
- `--devtools`-style trusted tooling must not create a separate gameplay/input implementation;
- specialist acceptance modes are automation harnesses around the same runtime, not alternate games;
- `rustcraft-server` remains a genuinely separate headless authority process.

Required field work:

- reproduce and repair the normal `just client-survival` F3+digit regression through the real
  `WindowEvent` path, not only direct shortcut-unit calls;
- converge normal developer gameplay onto a canonical `just client` workflow; keep compatibility
  aliases only temporarily until DX2 retirement;
- compare dev and release builds on the owner's available AMD Radeon Vega 8 / RADV / Vulkan path;
- rerun stationary/pan/walk presentation evidence on real hardware and diagnose remaining perceived
  camera/frame smoothness without reopening P1 unless a reproducible defect is found;
- correlate long frames with player/world checkpoint durability events;
- investigate high player-checkpoint `sync_ms` tails and remove only proven redundant durability work
  while preserving S1 crash/recovery guarantees;
- explicitly separate application frame timing, filesystem durability latency and physical display
  behavior.

F1 is corrective integration/field validation, not networking, remote authorization or a persistence
backend migration.

## A1 — external identity, policy boundary & trust preparation

Status: planned; starts only after F1 closes.

Owner: PM5-001, PM5-005 and PM5-011.

Goals:

- remove process-local numeric identities from future external Bot/Agent/network-facing contracts;
- use stable semantic IDs and stable entity identity at process boundaries;
- ensure observations/actions that need entity identity expose durable `EntityId` rather than transient
  collection/local indices;
- finish incremental migration of Minecraft gameplay policy out of generic runtime/mod-api surfaces;
- establish generic trust/provenance context for future server administration and remote commands;
- keep capability checks explicit and shared with Control;
- preserve headless/bot determinism and avoid designing the full M5 wire protocol prematurely.

A1 defines the boundary M5 will rely on; it is not M5 networking implementation.

## C2 — unified content definition & capability model

Status: planned; starts only after A1 closes.

Purpose: establish the common content-definition contract that later blocks, items, entities, fluids and
other content categories specialize instead of growing independent class hierarchies and engine switches.
Conceptually this is the shared content “superclass”; in Rust it should be expressed through typed
composition and profile compilation rather than inheritance.

Core model:

- every authored content object has stable semantic identity;
- common metadata is expressed through typed properties, tags, capabilities, handlers/components and
  semantic resource references;
- category-specific definitions add only the contracts that are truly category-specific;
- mutable per-instance state remains distinct from type-level properties and behavior;
- authored flexibility is compiled into validated/indexed runtime definitions with dense local handles
  and fast capability lookup;
- the hot path must not perform repeated string-key/property-map lookups for common behavior.

The semantic distinction is explicit:

- tags classify what a definition belongs to or can be queried as;
- properties provide typed definition data/parameters;
- capabilities declare which generic contracts the definition implements;
- handlers react to bounded semantic lifecycle/game events;
- state records compact mutable instance state such as orientation, shape, powered state or damage;
- resources reference semantic presentation/model/shape data and never runtime atlas/GPU handles.

Handler contract:

- use a bounded set of semantic events such as place, break, use, tick, neighbor change, contact and
  other events justified by active gameplay;
- handlers/systems/rules receive controlled context/capabilities and produce Commands/CommandBuffer
  effects instead of unrestricted `&mut World` access;
- first-party handlers may be native Rust; the same semantic boundary must remain compatible with
  future sandboxed WASM handlers without implementing M6 here;
- avoid a giant universal callback DSL or an untyped `HashMap<String, Value>` runtime architecture.

Required proof:

- migrate only enough existing block/item/entity/content definitions to prove the common contract across
  more than one category;
- preserve existing gameplay semantics and stable semantic IDs;
- demonstrate that a new ordinary content definition can be added primarily through definition data,
  properties/tags/capabilities/resources and optional handlers rather than switches across unrelated
  engine systems;
- keep persistence, rendering and future network identity independent from runtime-local dense handles;
- keep `minecraft-b173` policy in the game package and the common mechanism in generic crates;
- preserve headless operation and current public Game API direction.

C2 is architectural foundation, not a full mod SDK, full ECS rewrite, scripting language or broad
content migration campaign. It should create the smallest stable common contract needed by BG1 and
later M6 work.

## BG1 — generalized block geometry & model system

Status: planned; starts only after C2 closes.

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

## DX2 — tooling equivalence, workflow convergence & retirement

Status: planned; starts only after BG1 closes.

Owner: PM5-009 and obsolete developer workflow debt.

Goals:

- prove canonical `just` workflows exercise the same real runtime paths used by humans and CI;
- retire redundant milestone-only aliases and alternate launch paths after equivalence is demonstrated;
- converge client scenarios/acceptance onto Control/scenario orchestration rather than bespoke client
  variants where practical;
- retain specialist low-level diagnostics only where they test a unique contract;
- update tooling/docs so a contributor does not need historical milestone knowledge to run, debug or
  validate the project;
- keep public CI deterministic, bounded and free of private assets.

DX2 is cleanup and equivalence proof, not a feature milestone.

## RF1 — structural codebase refactor & code-graph optimization

Status: planned; mandatory before READY1.

Purpose: perform a comprehensive behavior-preserving structural refactor after the pre-M5 contracts
have stabilized, so code-graph search, ownership reasoning, future edits and automated maintenance are
materially cheaper before networking multiplies cross-cutting complexity.

RF1 is deliberately late in the pre-M5 sequence: R2/F1/A1/C2/BG1/DX2 may still change boundaries. The
refactor happens after those contracts settle and before READY1 freezes readiness.

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

## READY1 — final pre-M5 readiness gate

Status: planned; starts only after RF1 closes.

READY1 is a verification/freeze gate, not another implementation campaign.

Goals:

- run a fresh whole-repository architecture and risk audit against the post-RF1 tree;
- confirm no pre-M5 BLOCKER/HIGH risk remains without an explicit accepted deferral;
- verify the canonical client/server/tooling workflows from a clean checkout;
- verify public Ubuntu/Windows CI and representative local graphical/headless smoke;
- recheck stable semantic external identities, capability/provenance boundaries, resource identities,
  unified C2 content-definition contracts, BG1 model/shape identities, persistence compatibility and
  generator identities;
- verify no historical private/proprietary assets are required by public build/test/release paths;
- confirm M5 can add transport/replication without first restructuring client startup, code ownership,
  content-definition/model boundaries, persistence or resource identity again;
- produce a concise `READY1_REPORT.md` and exact M5 entry contract.

READY1 must not hide unfinished architecture work by renaming it "M5 follow-up". If the audit finds a
real blocker, create a bounded repair stage before M5 rather than starting networking around it.

## M5 — multiplayer + server content resolution (future, inactive)

Starts only after READY1 closes.

- authoritative server;
- QUIC evaluation/transport;
- connection/session identity and authenticated capability context;
- snapshots/deltas/interest management;
- local-player prediction/reconciliation;
- semantic manifest handshake;
- content cache/fetch/integrity;
- headless remote bot transport;
- bounded initial multiplayer scale consistent with the accepted S1 storage envelope.

M5 does not require Java protocol compatibility.

## M6 — third-party modding (future, inactive)

- WASM runtime for downloaded/untrusted executable mods;
- capability API built on the common C2 content/handler contracts;
- stable versioning;
- example mod;
- target-specific content and semantic block/item/entity/model extensions;
- quotas/profiling;
- namespaced/versioned durable component use through the host persistence contract rather than raw
  filesystem access.

## M7 — evidence-driven optimization campaign (future, inactive)

Evaluate data layouts, storage engines, SIMD, io_uring, allocators, PGO/BOLT and high-player-count
partitioning only against representative benchmarks. R1/RSM1/P1/S1 workloads should supply reusable
evidence for this later campaign.
