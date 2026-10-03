# Architecture decisions

Keep entries short. Add a new entry when a choice changes a durable contract.

## D-001 — Familiar behavior, not exact Beta compatibility

Status: accepted.

Beta 1.7.3 is the gameplay/visual reference. Internal algorithms, timing implementation, protocol,
storage and historical bugs are not compatibility requirements.

## D-002 — Platform plus first-party gameplay modules

Status: accepted.

Generic engine/runtime code remains independent of Beta-specific gameplay. The default game is
assembled from native first-party gameplay modules.

## D-003 — Semantic controller boundary

Status: accepted.

Humans, network clients, bots, replays and tests feed semantic intent into the same simulation
boundary. Simulation code does not read device events directly.

## D-004 — Bots are first-class

Status: accepted.

The platform owns an Agent/Bot API. Bots should not need renderer emulation or reverse-engineered
client automation.

## D-005 — Server-defined content profiles

Status: accepted.

Clients and bots automatically resolve required server content. Packages have targets so headless
bots do not fetch graphical/audio-only resources.

## D-006 — Native first-party, sandboxed third-party executable modules

Status: accepted.

First-party gameplay runs as native Rust. Untrusted downloadable executable content is sandboxed;
servers must never cause automatic loading of arbitrary native libraries.

## D-007 — `just` is the canonical developer command surface

Status: accepted.

Cargo remains the build system. `just` provides stable human/agent recipes for checks, smoke runs,
diagnostics and optional tooling.

## D-008 — Advanced optimization is measurement-driven

Status: accepted.

SIMD, io_uring, custom allocators, NUMA, lock-free structures, special databases and similar
techniques require a representative workload and measurement before adoption.

## D-009 — M0 simulation uses a generic voxel world and semantic action path

Status: accepted.

The first headless slice keeps chunk storage, AABB collision and fixed-step movement in generic
engine/runtime code. First-party blocks and flat generation register through gameplay modules;
break/place requests arrive as semantic controller intent and are also exposed through the Bot API.

## D-010 — wgpu and winit are isolated to the graphical path

Status: accepted.

The render crate owns wgpu device/surface/pipeline resources and CPU mesh extraction. The client
owns the winit event loop and translates platform input into semantic intent; runtime and server
remain free of graphics dependencies.

## D-011 — Render presentation copies only dirty world sections

Status: accepted.

The client keeps a RenderWorld presentation cache. Initial chunks are extracted once, then runtime
dirty notifications update changed X/Z sections and their boundary neighbors. CPU face-culling
meshes and GPU buffers are chunk-level resources; greedy meshing is deferred.

## D-012 — Local development resources resolve through content paths

Status: accepted.

The client uses the content crate's local resolver for the ignored/development terrain atlas path.
The renderer receives a resolved path and does not encode Minecraft asset locations in engine APIs.

## D-013 — World coordinates use X/Z horizontal and Y up

Status: accepted.

World, collision, section origins, mesh vertices and first-person camera math use X and Z for
horizontal axes and positive Y for vertical height. The view is right-handed with camera forward
mapped to -Z. CPU matrix helpers construct and multiply rows; `P * V * I` is converted once to
four f32 columns for WGSL `matrix * vector`. A transpose alone cannot repair mixed row/column
constructors. See `docs/RENDER_DIAGNOSTICS.md` for the verified convention and staged evidence.

## D-014 — Material tint is separate from lighting

Status: accepted.

First-party texture resolution supplies linear RGB tint with a white default. The renderer applies
it separately from face brightness. The flat-world grass top uses a fixed authored tint for its
grayscale atlas tile; biome coloration remains outside M1. Diagnostic checker/stone scenes stay
white-tinted and unlit.

## D-015 — M2 content, state and inventory

Status: implemented.

Blocks and items use separate typed u32 handles and semantic names in registries. A stored
BlockState holds a block handle and reserved variant bits (currently all authored states use zero).
Definitions express full-cube/empty collision, material, face resources, breakability, optional
item/block correspondence, stack limits, capabilities, emission and independent light opacity.
First-party atlas coordinates stay in gameplay-blocks; renderer APIs receive resolved tiles/tints.

Simulation owns a fixed 36-slot inventory (Option<ItemStack>, u16 counts), including the selected
nine-slot hotbar index. Selection, wheel changes and use/break arrive through AgentIntent. Placement
validates the selected item, destination and solid-block/player intersection before consuming one
item. Explicit controller placement also validates the semantic ray's adjacent cell and block hint;
explicit breaking must match the ray target. Direct simulation action methods remain useful to
trusted headless scenarios; controllers do not receive mutable World access. Development stacks
are seeded by semantic names at composition time and can be replaced without changing inventory.

## D-016 — M2 lighting and mesh invalidation

Status: implemented.

World owns packed four-bit sky/block channels in contiguous section arrays. Runtime Lighting owns
contiguous direct-sky seeds per loaded section and column bounds. Initialization traverses loaded
columns plus a one-section vertical light margin. An edit recomputes direct seeds only in its X/Z
column, then runs a deduplicated FIFO relaxation queue. Opening/closing sky and adding/removing
emitters converge using attenuation of at least one for propagated edges; vertical direct sky
retains 15 through clear air. Column extension initializes only that newly expanded column.

Every changed voxel/light value marks its section and any face-adjacent sections needed for
neighbor samples. Presentation copies dirty sections and their light halos, and existing GPU mesh
entries are replaced. Empty geometry is not drawn. M2 uses max(sky, block), a small brightness
floor and directional face shading; smooth lighting/AO is deferred. The temporary full-cube lamp
proves emission and removal. No renderer writes authoritative light or block state.

Opaque, Cutout, Translucent and Invisible are content classifications. Opaque neighbors hide
faces; identical nonopaque neighbors suppress shared faces; different nonopaque materials retain
interfaces. M2's atlas glass uses binary alpha and the cutout shader path. Fractional alpha and
sorted translucent rendering remain explicitly deferred, as do more sophisticated leaf interiors.

## D-017 — HUD and diagnostics are presentation

Status: implemented.

The purpose-built HUD consumes a read-only snapshot of slots, selection and the simulation's
RayHit voxel. Project-authored 5x7 glyphs, nearest atlas icons and integer scale produce crisp UI.
F3 is client state, never AgentIntent. DebugMetricsSnapshot periodically gathers simulation,
renderer, mesh, process and optional hardware counters; drawing code reads no world or OS data.
Debug text geometry/uploads are cached until text or dimensions change. Crosshair, slot and outline
geometry use bounded reusable buffers. Outline edges are currently projected screen-space lines.

## D-018 — Measurement definitions and scheduler policy

Status: implemented.

Headless runtime metrics use fixed 240-sample histories, without per-sample allocation. Rolling FPS
is 1 / mean frame interval. 1% low is 1 / mean of the slowest ceil(N/100) intervals, unavailable
before 100 valid samples. Tick and mesh durations use bounded histories. Actual TPS and rebuilds/s
are completed counts divided by elapsed monotonic wall time, refreshed after at least one second;
they are not configured-rate constants. Displayed target TPS is 20. Simulation timings exclude
presentation extraction/meshing; mesh timings include generation/upload preparation.

FixedStepClock retains fractional elapsed time, runs at most five 50 ms ticks per event-loop turn,
and deliberately drops whole excess ticks after a longer stall. It never increases tick dt or
changes the subsequent rate. The catch-up flag identifies turns needing multiple ticks. This
bounds the work after suspend/drag/long frames instead of accumulating an unbounded backlog.

Linux process sampling runs every 500 ms using /proc/self/status (RSS, threads), /proc/self/stat
(process user+system ticks) and /proc/stat (aggregate ticks and logical CPU count). CPU 100% means
one logical core; RSS is the kernel's inexpensive approximate resident figure. No new dependency
is needed. Other OSes or unreadable samples return None/N/A. See the kernel's
[/proc documentation](https://www.kernel.org/doc/html/latest/filesystems/proc.html).

GPU frame duration uses optional wgpu TIMESTAMP_QUERY from world-pass beginning to HUD-pass end,
asynchronous readback and queue timestamp period. It excludes presentation wait and capture copy.
Hardware utilization/VRAM have a separate optional client provider. Exactly one DRM device must
match adapter PCI vendor/device IDs; otherwise N/A. The Linux provider reads supported
[amdgpu sysfs counters](https://docs.kernel.org/gpu/amdgpu/thermal.html) at 500 ms intervals.
These are whole-device figures, not process VRAM or utilization inferred from frame duration.
No NVML/vendor dependency is mandatory. Unsupported timestamp/hardware metrics remain N/A.

## D-019 — M3 survival state and telemetry labels

Status: implemented.

`GameMode` is authoritative simulation state. Development mode seeds the temporary loadout;
Survival starts empty and routes block breaking through drops, item entities, pickup and inventory.
Item entities are compact value records with semantic `ItemStack`, velocity, age and pickup delay;
the runtime performs bounded merge/pickup passes and exposes semantic observations. ItemStack damage
is mutable per stack for tools, while the item definition supplies category, tier, speed and maximum
durability. Recipes are semantic shaped/shapeless records in a registry; the small 2x2 player grid
supports the first progression recipes. Rendering receives item position snapshots only.

F3 labels adapter telemetry as `Device GPU busy` and `Device VRAM`; process CPU/RSS remain distinct.
Present mode is shown explicitly with `(VSYNC)` for FIFO. These values are whole-device where the
platform provider says so and remain N/A without a matching provider.

## D-020 — M3.1 interaction presentation

Status: implemented.

Continuous mining uses the existing semantic `AgentIntent.attack` field as a held state. The
client clears it on release, focus loss and inventory mode; Survival mining resets on release or
target change and advances only on fixed simulation ticks. Dropped items are extracted into a
world-space sprite buffer and rendered with depth testing, while the HUD remains screen-space.
Inventory is a read-only snapshot with semantic slot-operation calls. Crack feedback is a
transient world-space overlay generated from the target block and mining stage; it never rebuilds
the persistent chunk mesh.

## D-021 — Beta resource-backed M3.2 presentation

Status: implemented.

The player inventory uses the semantic `gui/inventory.png` resource and the researched 176×166
layout. Block items opt into a shared 3D/isometric presentation for GUI, hotbar, cursor and world
drop paths; sprite items retain a 2D path. Destroy stages resolve to terrain atlas tiles 240–249.
Reference behavior is recorded before implementation in the M3.2 feature note, while the content
profile remains responsible for resource and presentation selection.

## D-022 — M3.8 canonical presentation geometry and offscreen inspection

One immutable full-cube face definition owns position perimeter, UV correspondence, normals
and triangle indices. World, GUI and dropped-item transforms consume that geometry. The
atlas uses top-left image coordinates; R1.0 supersedes the old `tile_uv` API with normalized
`AtlasRegion` mapping while retaining that convention. GUI items
use the source-derived column-vector transform, a depth-cleared pass and back-face culling;
GUI projection is independent of the gameplay camera. Resolved `BlockModel` carries semantic
face textures/tints and model rotation for both production presentation and inspection.

`offscreen::Scene` renders clip-space vertices to its own readable color/depth attachments.
It never copies a swapchain image or requires a visible window. The client CLI is the
first-party content adapter; renderer code contains no Beta block IDs or local asset paths.
Generated charts work without assets. Real-texture cases use the existing local resource
resolver. Outputs are deterministic local PNGs, suitable for future toleranced image diffs;
no unaccepted golden image or proprietary image is checked into source control.

## D-023 — compact semantic orientation foundation

Retain the existing `BlockState { block: BlockId, variant: u16 }` (8 bytes including padding),
with typed `Facing`, `Axis` and `HorizontalRotation` accessors. Facing uses bits 0..2, axis
3..4, horizontal rotation 5..6; other bits are reserved/preserved. Zero means North (-Z),
Y axis, R0. Typed constructors produce equal values for equal settings; no state-ID registry,
per-voxel string/map, allocation or opaque Beta metadata interpretation is introduced.

A block definition declares `orientation_property` (None/Facing/Axis/HorizontalRotation)
and a `base_model_rotation`. Proper signed orthogonal matrices rotate around the cube center.
Composition is base * state (column vectors: state first). First-party blocks currently keep
None/identity, so placement/gameplay behavior does not change. State survives world storage
and presentation extraction. Meshing rotates the complete geometry and its neighbor-facing
normal while keeping UVs attached; `state_texture` is a separate resolver hook for content
that selects a different material by state. Metadata import mappings and directional-block
gameplay are deferred. No furnace/door/stair mechanics were added.

## D-024 — engine mechanism and game policy boundary

The dependency direction is platform → engine → `game-api` → game package → optional packages.
Engine crates contain reusable voxel/runtime mechanisms and never depend on `minecraft-b173`.
Minecraft Beta behavior and content are policy implemented by the first-party game package using
the same public registration mechanisms available to another native game or native mod.

The initial `game-api` is intentionally small: validated namespaced IDs, generic voxel
definitions, package registration, native fixed/update schedules, a controlled block-mutation
command buffer and profile composition. Static function systems and compact/indexed storage avoid
hot-path indirection. New capabilities and commands require a concrete consumer.

## D-025 — incremental Minecraft package migration

`minecraft-b173` is the composition boundary for the existing first-party block and flat-world
modules. Client and server depend on it rather than importing those modules directly, and validate
its public Game API registration at startup. The M0-M3 `runtime` and `mod-api` remain transitional:
they mix reusable mechanisms with Minecraft inventory, mining, drops and crafting policy. Those
areas migrate when next extended, preserving accepted behavior rather than forcing a rewrite.

## D-026 — GameProfile and non-Minecraft architecture test

`GameProfile` composes namespaced package, resource and system identities with the existing content
manifest. It is composition metadata, not a general package manager. `sandbox-test` is a tiny
non-Minecraft game that registers four generated/debug blocks, consumes generic controller intent,
runs native scheduled systems, applies a queued world command and renders through the shared
offscreen renderer. Its Just recipe rejects dependencies on Minecraft, legacy gameplay crates and
the mixed runtime, making it a permanent dependency-direction integration test.

Compact `BlockState { BlockId, variant }` remains the engine storage strategy. Beta metadata
mapping, Minecraft properties and gameplay meaning belong in the Minecraft package.

## D-027 — semantics and scalability outrank exact Beta fidelity

Status: accepted; current project direction.

Beta 1.7.3 remains the first-party gameplay/content reference, visual style baseline and source of
recognizable interaction contracts. It is not a pixel-identical, behavior-identical or
algorithmically identical compatibility target. Prior exact-fidelity M3 notes remain historical
evidence for a completed repair campaign; they do not set the default policy for future systems.

Engineering priority is: clear predictable semantics; performance and frame-time stability;
large-world/content/mod scalability; Game API cleanliness; overall Beta-like identity; exact
fidelity. A measured or well-justified optimization may accept moderate visual differences while
preserving semantic readability. Substantial choices compare runtime/frame-time, memory,
scalability and extensibility benefit with visual/semantic cost, complexity and maintenance.

## D-028 — semantic identity, profile-local handles and canonical digests

Status: implemented in R1.0.

`NamespacedId` is an owned, validated `Arc<str>` authoring/serialization identity. Typed wrappers
separate packages, resources, blocks and textures where category mixing is unsafe. Packages register
semantic `VoxelDefinition`s without choosing numeric IDs. `GameRegistry::compile` follows explicit
`GameProfile` package order and lexical `BlockKey` order inside each package, moves the declared default voxel to
the reserved first slot, then emits a dense vector plus semantic-key index. `BlockId` is a compact
compiled-profile handle and has no cross-profile persistence guarantee.

`GameProfile` uses typed `PackageId` and `ResourceId` collections. Package registration and block
registration failures are atomic. Duplicate semantic block keys are errors; R1.0 deliberately has
no definition override layering. Resource override precedence remains an R1.1 composition decision.

The declared world default is queried through `World`/`CompiledGameProfile`; generic extraction no
longer interprets numeric zero as Minecraft air. Retaining the default at handle zero is an explicit
compiled-profile implementation rule, not content meaning. `BlockState` remains 8 bytes and no new
global variant flags are introduced.

Content bytes and canonical manifests use BLAKE3 derive-key domain separation. Manifest encoding
uses fixed-width integers, length-prefixed strings, sorted packages and sorted/deduplicated
dependency/target sets, covering all current semantic descriptor fields.

Renderer texture inputs are `TextureHandle` plus normalized `AtlasRegion`. The renderer receives
resolved terrain/inventory/hotbar/player paths from composition rather than discovering Minecraft
paths. R1.0 supports one uploaded physical page; the client-side Beta 16x16 conversion is a temporary
R1.1 adapter.

## D-029 — R1.1 compiled resource pages and bounded loading

Status: implemented.

`ResourcePackage` owns generic semantic texture discovery. Later packages override earlier
providers deterministically by `ResourceId`; the first-party Beta crop importer is package policy,
not renderer behavior. Before its at-most-eight decode workers start, the compiler validates every
unique PNG header and the checked aggregate RGBA allocation budget. Deterministic shelf packing
uses padded edge extrusion and selects the smallest supported power-of-two page dimension that fits
within the configured maximum and page-count limit.

Compiled pages are BLAKE3 content-addressed by sources, providers, compiler version and the complete
packing/sampling policy. Corruption rebuilds; unique create-new temporary files plus atomic rename
make concurrent writers safe; optional write failure is visible but non-fatal. One sampler policy is
allowed per page. Nearest and uniform linear sampling propagate to GPU creation, while mipmaps,
anisotropy above one and mixed-sampler pages are deferred pending atlas-safe mip generation and
measurement.

`TextureHandle` survives geometry extraction. Chunks, dropped block items and GUI block items are
submitted in page batches, including models whose individual faces span pages. UI sheet roles and
cracks bind their resolved page. The compatibility page-zero chunk uploader explicitly promises a
single page; test/inspection helpers that flatten geometry assert the same contract. The remaining
page-zero HUD bind is never sampled because that pass contains procedural color geometry only.

## D-030 — R1.2 bounded asynchronous section meshing and submission

Status: implemented; complete.

The renderer snapshots dense 16³ section state/light interiors into an 18³ contiguous volume with
one world-sampled halo cell on each side. This preserves boundary culling and light queries while
keeping normal mesh traversal hash-free. Client meshing uses a bounded worker pool (1–32 workers),
owned immutable snapshots, monotonically increasing per-section generations, and one newest pending
snapshot per in-flight section. Dirty notifications coalesce; stale or removed-section results are
rejected both at completion and before GPU upload. Worker failure is logged and does not panic the
client. CPU meshing never creates GPU objects.

Completed valid meshes upload on the render thread under configurable per-frame section/byte
budgets. A single mesh larger than the byte budget is permitted as a one-item progress exception.
Opaque/cutout page batches sort by atlas page and then section; translucent ordering is unchanged.
Per-section GPU buffers use checked geometric capacity (256-byte minimum), reuse existing
allocations, and shrink only below one-quarter utilization. The policy deliberately retains some
temporary capacity to avoid remesh churn; it is an estimate, not a fragmentation/VRAM guarantee.

`just render-scale` and `just render-camera-motion` report reproducible CPU workloads and actual
submission behavior. On the recorded release workload, radius-8 extraction/mesh time changed from
376.396/110.148 ms before snapshots to 32.767/24.886 ms after dense interior copies and indexed
meshing. Three real renderer upload cycles reused 108 buffers with zero new allocations or
reallocations. Greedy meshing remains deferred: it would need atlas-safe repeating UV behavior and
compatible material/light merging, while R1.2 has not yet established representative hardware
geometry as the next bottleneck. Bindless, arrays, GPU culling, indirect drawing, Hi-Z and LOD are
also deferred pending page-switch/draw and hardware profiling.

## D-031 — M4 scope is world generation and persistence

Status: accepted for M4.

The older roadmap entry named M4 as multiplayer/server content resolution. The active M4 contract
is now world generation and persistence foundation, with no network chunk-streaming protocol.
Accordingly the old network/content work is retained but moved to an inactive later roadmap item.
Engine code owns generic lifecycle, deterministic generation mechanisms and storage; selected game
packages own terrain/biome/feature policy. Persisted blocks use semantic identity and never treat
profile-local `BlockId` numbers as durable IDs. Renderer optimization topics including greedy
meshing, Hi-Z, bindless, GPU-driven rendering, compression, mipmaps/AF and LOD are backlog items,
not an active R1.3 milestone; only a concrete measured M4 blocker can reopen renderer work.

## D-032 — M4 starts with semantic one-file-per-column persistence

Status: accepted for the initial M4 format.

Use portable explicit little-endian encoding, versioned metadata/chunk schemas, per-file BLAKE3
checksums, section-local semantic palettes and atomic one-file-per-column replacement. This keeps
random access and crash recovery simple while locally resident worlds are small and avoids premature
region-container complexity. Revisit region files when measured file-count/open latency becomes
material at tens of thousands of columns or when streaming patterns justify a container. Runtime
`BlockId` is never a persistent identity.

The current payload benchmark on the repository's AMD Ryzen 5 PRO 2500U, Linux x86_64, Cargo
release profile encoded four generated columns into 264,248 raw section-payload bytes and 10,163
stored bytes (3.8%, including framing/checksums) with safe Rust `flate2` Zlib fast compression. The
crate is already present in the dependency graph through PNG support, provides a portable safe Rust
backend, and representative payloads showed a substantial size reduction. Compression stays
version-tagged and is bypassed when output is not smaller.

## D-033 — M4 generation uses coordinate-derived policy streams

Status: accepted for Minecraft overworld generator versions 1 and 2.

The generic `ChunkGenerator` takes only seed and signed `ChunkPos`, returns completed sections, and
must be independent of scheduling order. Minecraft policy samples domain-separated coordinate
hashes for terrain, bedrock, caves, ores and tree origins; cross-column features enumerate a
neighborhood of deterministic origins and clip to the requested column. The target is coherent,
recognizable Beta terrain rather than Java seed parity. Generator semantic changes require a new
stored generator version or explicit migration; terrain must not silently change on regeneration.
Version 1 is permanently frozen by canonical hash
`e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6`.
Version 2 extends the contract with independent climate, terrain, surface, cave, ore, vegetation
and lake domains; no stage consumes a shared mutable stream.

## D-034 — Static liquids retain a distinct compiled presentation class

Status: accepted for M4 generated static liquids.

Do not infer interaction targeting or geometry from the historical block number. Authored
definitions carry targetability and a distinct liquid material through the compiled profile. The
renderer submits opaque/cutout geometry first, then section-coarsely sorts blended pages back to
front with depth writes disabled. Static source-liquid tops are lowered to 0.875 block units and
same-block internal faces are suppressed. Camera medium is resolved from compiled liquid
presentation metadata; a bounded linear underwater fog policy is game-owned and applied to generic
opaque/translucent world shading (Minecraft policy currently uses color `(0.20, 0.40, 0.62)`,
linear start 2 and end 18 world units). Swimming and fluid simulation remain separate gameplay
work.

## D-035 — Persistence compatibility follows the persisted schema, not full profile identity

Status: accepted for M4.

`CompiledGameProfile::semantic_fingerprint` remains useful diagnostic identity, but presentation
and gameplay descriptors are not a save-decoding contract. Metadata v2 stores a separate persisted
state schema version alongside game/profile family and generator identity. Existing chunk palettes
are validated by semantic key/variant resolution. Missing content and unsupported state schema are
explicit errors. Stored chunks remain loadable across generator upgrades; only absent chunks require
an exact generator ID/version match before generation. Metadata v1 is safely interpreted as the
existing semantic-key plus `u16` variant schema v1 and rewritten after successful chunk loading.

## D-036 — Generic player record envelope, game-owned durable schema

Status: accepted for M4 local-player persistence.

World storage owns a validated player ID and bounded, versioned, checksummed opaque component
envelope; it does not interpret payload semantics. Outer record v2 contains a monotonically
increasing revision and components sorted by semantic ID, each with its own schema version and
bounded payload. Minecraft codecs currently register `minecraft_b173:player/transform:v1`,
`minecraft_b173:player/inventory:v1`, and `minecraft_b173:player/game_mode:v1`. Inventory stores
all slots, selected hotbar, cursor and crafting grid; item identity is semantic text resolved at
load, never a runtime handle. Unknown components are preserved opaquely through save/reopen;
missing required or unsupported known components fail explicitly. The prior single-payload record
is loaded as `rustcraft:legacy-player-payload` and migrated on its next checkpoint.

Durable component changes increment revision; one background worker has at most one in-flight
checkpoint and one replaceable newest snapshot. The two-slot store overwrites the slot other than
the newest valid checkpoint, syncs file data and the containing directory where supported (Unix),
and validates each slot with BLAKE3. Windows still syncs checkpoint file data; directory-entry
sync is unsupported and is not claimed. Metadata/chunk replacement uses `atomicwrites`, whose
Windows implementation calls replace-existing/write-through `MoveFileExW`, instead of relying on
`std::fs::rename` overwrite semantics. An
interrupted slot write leaves the previous revision available; startup picks
the highest valid revision and reports fallback recovery. Checkpoint cadence defaults to two
seconds and is clamped to 1–2 seconds (`RUSTCRAFT_PLAYER_AUTOSAVE_SECONDS`); this bounds expected
recent-state loss to roughly that interval under normal scheduling, not arbitrary hardware cache
failure. Graceful shutdown waits for the latest dirty revision. First-ready chunks center on the
saved position. Velocity/contact reset, AABB/camera are derived, and mining/input/UI/render state
is transient. M4-003 adds pickup receipts as a fourth Minecraft-owned player component; D-042
defines the separate spatial and world-global domains.

## D-037 — Product identity and release versions are independent of game packages

Status: accepted.

The product/repository brand is RustCraft; `minecraft-b173` remains the first-party game package
identity. Product releases use the single root workspace SemVer, initially `0.1.0-alpha.1`
because there are no prior release tags. Content packages carry their own SemVer. Game API,
network protocol, world/chunk/player/resource schemas, and game-package versions remain independent
compatibility domains. Product version changes never implicitly migrate saves. Formal publication
uses the owner-selected `MIT OR Apache-2.0` project license. Local history was rewritten to remove
`reference/assets`; release automation verifies the license metadata and scans reachable objects. It
never pushes, renames a remote repository, tags, or publishes.

## D-038 — Local world residency is interest-driven and bounded

Status: accepted for the M4 local streaming slice.

The generic `WorldResidency` mechanism takes signed column interest, tracks request tokens/phases,
prioritizes by distance with a bounded recent-motion lookahead along a normalized 70% movement /
30% horizontal view-direction blend, applies a configurable load radius
and retain hysteresis, and proposes evictions without owning game generation policy. The motion
estimate measures displacement per fixed tick, tracks stable-heading duration and speed, clamps each
sampled delta to 8 blocks and speed to 32 blocks/s, resets duration on a heading change, decays
duration after movement stops, and caps lookahead at three columns. This bounded speed × time means a
short high-speed nudge has small influence, while sustained travel advances requests ahead. Current
client policy uses load radius 4 and retain radius 5; `RUSTCRAFT_STREAM_RADIUS=3..12` remains
available. Desired and retained membership use Chebyshev distance, producing complete square
neighborhoods. Its
shared stage score has dominant REQUIRED (3x3 safety core), VISIBLE (next inner ring), and PREFETCH
classes; motion lookahead only orders work inside those classes.
Lookahead defaults off: the final normal-speed A/B had identical 0.5-column minimum forward Safe
and Visible margins, while request-to-visible p95 was 1.30 s on versus 1.50 s off, within the
software-renderer/run variance and consistent with the earlier indistinguishable A/B. The mechanism
and `RUSTCRAFT_STREAM_LOOKAHEAD` override remain available for future hardware evidence.
Bounded workers own disk load/decode/semantic resolution and generation; results carry the
current request token and are discarded if interest was abandoned. Resident columns outside retain
radius are considered for eviction every simulation tick; dirty/save-in-flight columns remain pinned
until successful persistence. Initial-light jobs are separately bounded to eight queued/active/result
slots; after completion the client applies at most one column per event-loop callback, subject to
the shared callback work budget. Missing storage may enter generation, but
corrupt data never does. When persisted generator identity differs, stored columns remain loadable
while absent columns cannot be generated under the incompatible policy.

Unavailable columns block movement and ray traversal rather than masquerading as air. Dirty columns
remain resident until their current save generation completes successfully. Persistable
dropped-item entities now take a frozen atomic column snapshot and no longer pin terrain. Eviction
removes lighting/world state and invalidates render snapshots,
mesh jobs and resident GPU sections. Startup creates the window first, then uses a cancellable
background bootstrap to obtain a minimum safe neighborhood centered on the restored player. This is
not general multiplayer interest management or an unbounded travel protocol.

New columns use a bounded worker-based bulk initial-lighting stage before voxel publication.
Initial direct skylight is constructed by vertical scans and only shaded cells/emitters seed
relaxation. Complete light arrays are applied by ownership transfer, then resident-neighbor faces
are reconciled; only boundary reconciliation, removal cleanup and interactive edits use the
existing incremental path. One initial-light worker is the default, coordinated
with three mesh, one load and one generation worker for six compute-heavy workers on an
eight-logical-CPU host; save workers are separate and I/O-oriented. A headless sweep found only a modest throughput gain from 2–4 initial-light workers,
while per-column worker elapsed increased, so one remains the default. This policy is provisional
until the same route is measured on AMD Vega 8/Vulkan. Order tests compare A→B, B→A and simultaneous
availability, including a seam emitter, against synchronous lighting.

## D-039 — Streaming application yields to event processing

Status: accepted as a responsiveness safeguard; target-hardware acceptance remains open.

Async workers do not guarantee a responsive client if result application is monopolizing the
event-loop thread. Per callback, input/event dispatch runs before optional streaming work. The
initial implementation shared one 2 ms wall-time budget across result application, boundary light,
snapshot synchronization and mesh scheduling. That kept the event loop responsive but exposed
render-ready starvation: early work and 20 Hz fixed-tick servicing could defer later presentation
stages. See D-040 for the revised staged policy and M4-011 acceptance.

## D-040 — Reserve fair per-turn service for visible streaming stages

Status: accepted for M4-011 and the automatic streaming acceptance path.

Streaming service runs after fixed-step/input processing on each event-loop turn, not only on a
simulation tick. The existing total 2 ms soft budget is divided into ordered windows for critical
completed-result application, boundary lighting, snapshot synchronization and mesh submission.
Each stage has its own deadline, so an earlier stage cannot consume every later stage's allowance.
Mesh upload-ready results continue to receive a per-render allowance and prefer camera-forward
sections within the same urgency ring. Initial resident mesh jobs are seeded in urgency/distance
order rather than coordinate order. Locally complete initial-light columns may be presented before
cross-column boundary reconciliation; the later correction remains authoritative and invalidates
only light-changed sections. The bounded boundary queue is sized to 256 for the supported radius-6
sweep and is reprioritized before queued work starts. Queued lighting work outside retention
is cancellable and does not pin columns; active reconciliation still completes atomically. Within every expensive
content/presentation queue, urgency classes dominate lookahead:
REQUIRED is the 3x3 safety core, VISIBLE is the next inner ring, and PREFETCH is the remainder.
Age is diagnostic and must not promote prefetch above required/visible work. Stage skip, starvation,
skip, dependency-block, starvation and backlog-age counters plus a forward-facing F3 frontier state
expose scheduling behavior; waiting on a full prerequisite queue is reported as dependency blocking,
not scheduler starvation.
The individual snapshot copy or indivisible light operation can still exceed its soft slice; the
same-device frame and terrain-arrival measurements decide whether that work needs further
subdivision/offload.

## D-041 — Streaming area geometry and safety are explicit

Status: accepted for M4 local streaming.

Euclidean membership (29/49/81/113 columns at radii 3/4/5/6) made the old edge look irregular and
provided unequal cardinal and diagonal reach. Manhattan membership (25/41/61/85) makes a visible
diamond and the weakest cardinal margin. Chebyshev membership (49/81/121/169) costs explicit corner
columns but provides a predictable square and a simple guaranteed travel margin, so Desired and
Retained use Chebyshev distance. The selected normal default is load radius 4 and retain radius 5.

Authoritative residency no longer implies simulation availability. A complete centered 3x3 must
be both locally authoritative and render-ready before control starts. Thereafter Safe grows only
by complete 3x3 neighborhoods overlapping the existing connected Safe set. Visible is presentation
readiness and may lead Safe; Safe can never lead Visible. Retained is radius hysteresis plus explicit
persistence and active-light pins, none of which alter the player frontier. Boundary light
is eventual convergence and may remesh changed sections without gating collision or presentation.

## D-042 — Durable state is split into column, world-global and player domains

Status: accepted for M4-003.

Generic storage owns framing, bounds, semantic type/component IDs, revisions, checksums, atomic
writes and spatial association; the active game owns payload meaning and codecs. This avoids a
reflective ECS serializer and keeps Minecraft item/time policy outside engine storage.

Spatial records are embedded in chunk payload v3 rather than placed in a global entity file or
sidecar. Block mutation and a drop in the same column share one atomic snapshot,
save-before-evict needs one dirty generation, corruption is column-local, and future region
storage can relocate the complete column unit. Payload v2 remains readable as zero entities.
Minecraft's item codec uses a stable 128-bit EntityId, semantic entity type and ItemKey, and all
current authoritative item fields. Production IDs combine a per-session 64-bit namespace with a
monotonic 64-bit counter; deterministic tests inject the namespace. New sessions use new
namespaces, so a lagging global checkpoint cannot collide with existing IDs.

Cross-column motion uses destination-before-source persistence. The destination record carries a
tombstone naming the old source; stable ID plus entity revision selects one newest active copy in
either load order. Source cleanup follows destination success, then destination cleanup removes
the bounded tombstone. Merge is restricted to one owning column, so count conservation is atomic.

Player pickup is an ordered cross-domain transfer: inventory change and a bounded pickup receipt
are one player checkpoint, its successful completion authorizes source-column removal, and source
success authorizes receipt pruning. Receipts suppress stale source copies during recovery. The
guarantee is idempotent recovery without endlessly repeatable duplication or silent permanent
loss, not instantaneous atomic commit across two files.

World-global state is a separate two-slot `RCSTATE` v1 envelope of sorted independently versioned
opaque components. `minecraft_b173:world/clock` v1 stores simulation ticks and pauses offline.
Unknown components are preserved byte-for-byte; unsupported known schemas fail. One bounded
coalescing worker checkpoints at the player autosave cadence and flushes on graceful shutdown.
The unexpected-failure loss window is the latest successful checkpoint (normally about two
seconds); platform sync claims remain those of D-032/D-036. Lighting, streaming, rendering and
controller state remain derived or transient and are never serialized.

## D-043 — New worlds use versioned Beta-recognizable overworld v2

Status: accepted for M4-004.

Keep `minecraft_b173:overworld` version 1 as an exact supported implementation for all existing v1
worlds. Resolve saved generator ID/version through the Minecraft package; never substitute latest,
never auto-upgrade, and fail contextually when the exact implementation is unavailable. New worlds
persist version 2. Generator output does not change chunk payload version because materialized
semantic `BlockState` storage is unchanged.

V2 deliberately uses a smooth 2D continental/rolling/hill height model plus bounded 3D carving
rather than Beta's full historical density/RNG pipeline. Temperature and moisture derive a small
meaningful biome set: ocean, beach, plains, forest, desert and hills. Surface replacement,
curved/branching caves, clipped ore segments, simple oak trees and ellipsoid water lakes each use
fixed coordinate-origin halos. The design is Java-seed-incompatible but deterministic across
request order, worker count and negative coordinates.

First-time spawn selection is version-specific game policy: search a bounded dry coordinate grid,
center the startup core there, then require the highest local surface to provide stable support and
headroom. Existing players retain persisted positions. Legacy v1's authored origin sandbox is
allowed only when its two columns are already resident in that startup core: relocation must not
create a competing partial origin world and an unsafe fallback spawn. Existing worlds are never
redecorated. Biomes remain derived and unpersisted.
Water lakes are static source voxels. Lava is deferred until a semantic block/resource/medium/light
contract exists; dungeons are deferred until spawner, mob, chest and loot gameplay exists. These
omissions are preferable to placeholder content that would expand M4 into unrelated systems.

## D-044 — Developer control and Rhai adapters

Status: accepted; DX1 closed by local/headless/graphical and Ubuntu/Windows CI acceptance.

Game-neutral control owns registry/source/capability/snapshot/action/scenario contracts.
Rhai 1.26.1 is a leaf adapter with per-session scopes, bounded AST execution and no dynamic
filesystem imports. Native and scripted commands converge on semantic actions; first-party
registration stays in minecraft-b173. Scenarios build explicit Rust steps rather than fake
coroutines. Fixed pause leaves worker/event/render turns running. See SCRIPTING.md.

DX1 workers use one bounded thread and request generations; publication stays on the owner thread.
Scopes never cross consumer ownership. JobRef tracks real compile/bundle/PNG lifecycle, with bounded
terminal retention. GPU mapping stays in render ownership with nonblocking publication; CPU PNG
work stays in the tooling worker. This adds no GPU dependency to headless server or game policy to
control. Native REPL parsing is limited to a 4 KiB console line; file compilation is background work.
