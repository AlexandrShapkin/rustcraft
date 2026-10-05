# Architecture

## Direction

RustCraft is a universal voxel engine with an extensible Game API. `minecraft_b173` is the first
game package built on that API and the primary integration target; it is not part of the engine's
fundamental vocabulary.

Engineering decisions use this order: clear and predictable gameplay semantics; performance and
frame-time stability; scalability to large worlds/content/mod sets; extensibility and Game API
cleanliness; overall Minecraft/Beta-like visual identity; exact Beta fidelity. Beta 1.7.3 remains
the first-party content reference and style baseline, not an absolute compatibility target.

The durable rule is **engine provides mechanism; game provides policy**. If the Minecraft package
is removed, engine crates must still describe a coherent voxel runtime. Dependencies point upward:

```text
platform
   ↑
engine
   ↑
game_api
   ↑
minecraft_b173 or another game
   ↑
optional game packages/mods
```

The engine may own voxel storage, compact block state, transforms, collision primitives,
registries, resources, schedules, command application, serialization, networking infrastructure,
generic UI, and rendering. A game package owns the meaning of mining, drops, inventories,
crafting, recipes, progression, blocks, items, mobs, time, fluids, and other game rules.

## Crate roles

- `engine-core`: typed IDs, chunk/world storage, compact `BlockState`, geometry and collision
  mechanisms. It has no game-package dependency.
- `render`: generic meshes, materials, textures, transforms, cameras, layers and diagnostics. It
  consumes resolved presentation data and never branches on Minecraft content IDs.
- `content`: package manifests, targets and hashes.
- `game-api`: public native extension mechanisms: namespaced IDs, registries, package
  registration, schedules, profiles and controlled world commands. Native games and native mods
  use the same contract.
- `agent-api`: semantic controller intent shared by humans, bots, networks, replays and tests.
- `bot-api`: versioned observations/actions over semantic data and capabilities.
- `minecraft-b173`: first-party game package and incremental compatibility boundary around the
  existing M0-M3 gameplay modules.
- `runtime`, `mod-api`, and `gameplay-*`: transitional M0-M3 implementation. Their remaining
  mixed ownership is catalogued in `ARCHITECTURE_AUDIT.md`; new Minecraft policy must move toward
  `minecraft-b173` rather than deeper into generic runtime code.
- `client` and `server`: platform adapters and composition roots. Neither is a source of gameplay
  authority.
- `sandbox-test`: tiny non-Minecraft integration game proving that engine, renderer, world,
  Game API and semantic input work without the Minecraft package.

## Extension model

The API supplies a small set of extension mechanisms rather than a complete catalogue of future
mechanics:

- components/state hold compact data;
- native systems implement executable behavior in registered schedules;
- declarative definitions/rules describe content where that is sufficient;
- events state facts and commands request controlled mutations;
- queries expose data without leaking mutable implementation details;
- capabilities provide weakly coupled contracts;
- registries map namespaced semantic IDs to indexed runtime definitions.

Static/native systems and specialized storage are valid. Extensibility does not require dynamic
maps or trait objects in per-voxel hot paths. A genuinely new mechanism may extend the Game API or
engine plugin boundary; ordinary new content should not require an engine edit.

## Execution and mutation

The engine owns technical schedule stages such as input collection, `FixedUpdate`, `Update`,
command application, maintenance, extraction and rendering. Games register systems into those
stages. Systems request authoritative world changes through a bounded command buffer; the engine
applies commands at a controlled boundary. The current public proof implements `SetBlock`; new
command variants are added only for real use cases.

OS input becomes generic semantic actions before reaching a game. A game interprets
`PrimaryAction`, `SecondaryAction`, a target voxel and actor context. Minecraft may interpret these
as mining or placement, while another game can choose different behavior. Legacy M0-M3 intent
fields remain temporarily for behavior-preserving migration.

## Content and profiles

All extensible identities are namespaced (`minecraft_b173:block/stone`,
`sandbox_test:block/crystal`). A `GameProfile` composes package IDs, resource sets, registered
systems and a content manifest. It is composition metadata, not a package manager:

```text
Minecraft:       voxel_std + minecraft_b173
Minecraft + mod: voxel_std + minecraft_b173 + some_mod
Other game:      voxel_std + sandbox_test
```

The first-party Minecraft package receives no private gameplay shortcut. Existing M0-M3 paths
that predate `game-api` are explicit migration debt, not precedent for new systems.

Authored identity and runtime identity are separate. `NamespacedId` and typed keys such as
`BlockKey`/`TextureKey` are owned, validated semantic identities that survive process and package
ordering changes. `GameProfile` compilation assigns dense `BlockId` handles in explicit profile
package order and lexical `BlockKey` order within each package, with the profile's declared empty/default voxel
reserved first. Runtime handles are valid only for that compiled profile. Persistence and network
formats must serialize semantic keys, or carry an authoritative versioned profile mapping.

The compiled block registry provides O(1) `BlockId` lookup and a semantic-key index at loading,
diagnostic and serialization boundaries. Voxel hot storage contains only compact `BlockState`
values. Semantic strings never enter per-voxel storage or meshing lookup loops.

## Semantic invariants

Optimization may change pixels, update algorithms or distant detail, but it must not make core
state and interactions ambiguous. Grass remains recognizable as grass; directional state remains
readable; a lever/device relationship behaves predictably; mining/tool relationships remain
understandable; inventory operations remain deterministic; voxel water remains recognizably voxel
water. Game packages define these contracts and tests should assert them independently of a
particular rendering or simulation optimization.

## World and presentation

World hot storage retains compact `BlockState { block: BlockId, variant: u16 }` values. Typed
state accessors express facing, axis and rotation without strings, maps, allocation or an opaque
Beta metadata byte. Registered definitions resolve a state to collision and generic render
descriptors. Historical metadata conversion belongs in the Minecraft compatibility layer.

Texture identity follows the same boundary: authored definitions name a semantic `TextureKey`,
profile/resource compilation resolves it to a profile-local `TextureHandle` and an arbitrary
normalized `AtlasRegion`, and renderer code consumes only that physical region. Atlas regions do
not imply 16-pixel tiles or one page. R1.1 discovers generic namespaced PNG resources, applies
deterministic package-order overrides, imports the local Beta sheets as virtual cropped resources,
and compiles padded/extruded deterministic shelf atlases into a BLAKE3-addressed disk cache.
`minecraft_b173` owns the legacy sheet coordinates; generic content and renderer crates do not.

Physical atlas pages are first-class. Chunk meshes, dropped block models and GUI block models are
grouped by `TextureHandle`, so one model may reference several pages. Crack, container, HUD and player-preview quads bind their resolved pages explicitly;
background and selector or adjacent preview parts may use different pages. The sole production page-zero bind is
the procedural HUD color pass, whose shader branch never samples the bound texture. The offscreen
contract uses project-owned three-page resources to exercise chunk, dropped and GUI submission.

The compiler treats configured page dimensions as device-safe maxima and chooses the smallest
supported power-of-two square that deterministic shelf packing can satisfy within `max_pages`.
The client can clamp this maximum with `RUSTCRAFT_MAX_ATLAS_DIMENSION`; the renderer also validates
every compiled page against the actual adapter limit and fails gracefully if configuration and
device disagree. Adapter discovery still occurs after first-party compilation in the current
startup architecture.

Block selection uses world-space AABB line geometry in a dedicated line-list pipeline after world
geometry has populated depth. It uses a 0.002-unit expansion, black 0.55 alpha blending,
`LessEqual` depth testing and disabled depth writes. GPU clipping handles near-plane intersections
and scene depth hides rear edges; selection is not a screen-space HUD overlay. Interactive review
confirmed occlusion/near-plane behavior; the line opacity was raised slightly after that review for
clearer visibility.

`RendererResources` now carries compiled physical pages plus resolved semantic presentation roles
for the cropped inventory panel, separate hotbar/selector and six preview regions. Each quad
uses full local UVs, and ordered page ranges preserve painter order across arbitrary pages.
Historical physical crop coordinates live only in the Minecraft importer and historical tests.
The purpose-built inventory/hotbar layout and preview destination roles remain explicit ARCH-002
game-policy debt; R2 does not add a GUI framework or move gameplay authority.

Simulation owns authoritative state. Presentation extraction produces read-only generic render
data. Renderer and UI drawing never mutate gameplay. Beta reference work defines the observable
semantics and recognizable presentation of `minecraft_b173`; it does not define engine
architecture or require historically identical algorithms/output.

## Performance

Prefer indexed registries, contiguous chunk storage, batch processing, dirty queues and native
first-party systems. Profile measured workloads before adding indirection, concurrency or exotic
storage. ECS is useful for many entities but is not mandatory for chunks, networking or every
piece of game state.

Measured optimizations may introduce moderate visual differences when semantic readability stays
intact. Valid directions include mipmapping, anisotropic filtering, texture compression, LOD,
reduced distant update rates, simplified distant materials/transparency, aggressive culling,
dynamic quality, GPU-driven rendering, greedy meshing and optimized lighting. Evaluate resource
pack size, block-state cardinality, mod composition, view distance, entity counts and server scale,
not only the first-party Beta scene.

## R1.2 section rendering

Presentation extraction owns an immutable contiguous 18³ section snapshot: the 16³ interior is
copied from dense engine section arrays, while the one-voxel halo is sampled through world
accessors. Meshing workers receive owned snapshots and use indexed voxel/light reads only. The
client schedules one in-flight generation per section and coalesces later dirties to the newest
pending snapshot. Results whose generation is no longer authoritative (including removed sections)
are discarded before upload. GPU upload remains on the render/event thread and is bounded per frame
by section and byte budgets; one result larger than the byte budget may progress alone to avoid
starvation.

Resident meshes persist while outside the conservative camera frustum. Frustum visibility changes
only which page batches are submitted; it never dirties/rebuilds geometry. Opaque/cutout batches
are deterministically ordered by atlas page then section, while translucent geometry retains its
existing ordering policy. Per-section GPU vertex/index buffers grow geometrically and are reused
when capacity suffices, with a quarter-capacity shrink threshold. Debug metrics distinguish CPU
snapshot/mesh estimates, logical GPU buffer bytes, retained GPU capacity estimates, and CPU upload
submission time; none is presented as measured VRAM or transfer completion.

## M4 world foundation (functionally complete)

`engine-core` remains policy-free: `ChunkBuilder` creates unpublished dense sections, and
`World::publish_column` replaces a finished set of signed-Y sections at one publication boundary.
The new `rustcraft-world` crate owns generic generator/scheduler contracts, lifecycle tracking,
versioned storage, semantic palettes, persistence dirty generations and bounded save workers. It
does not depend on Minecraft block definitions. The flat generator and `sandbox-test` both compile
against the same generator contract.

`minecraft-b173::worldgen` owns two exact implementations of the first game policy under semantic
ID `minecraft_b173:overworld`. Version 1 is frozen for existing worlds and retains its canonical
output. Version 2 is the new-world default: a fixed 128-block overworld with sea level 64, smooth
temperature/moisture climate, derived ocean/beach/plains/forest/desert/hills classes, continental
height plus rolling/hill fields, contextual surface replacement, curved caves, bounded ore veins,
biome-aware trees and static water lakes. Biomes remain derived from seed/version/coordinates and
are not persisted. Both versions use a narrow legacy block-handle adapter while the simulation
still uses M0-M3 IDs.

Existing metadata resolves the exact `(generator ID, version)` through the Minecraft package before
any missing column is scheduled. Unknown IDs/versions are explicit compatibility errors; there is
no latest-version fallback and no automatic v1-to-v2 conversion. New metadata records v2. Every v2
random decision is domain-separated by seed, generator version, stage/feature identity, origin
coordinate and feature index. Bounded origin halos are clipped into each destination column, so
request order and worker completion cannot alter caves, veins, canopies or lakes. Generation
returns authoritative sections only; it does not build lighting, render snapshots or meshes.

Canonical v1 retains its historical handle-based lock, while v2 hashes semantic block keys and
variants: seed 731173, columns X/Z in `[-2,2)`, X-major/Z-minor order, sorted section Y, dense voxel
order, BLAKE3 over little-endian coordinates and `(u32 key byte length, UTF-8 key, u16 variant)`.
V1 is `e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6`;
v2 is `4e134fa137fa5477fc3d28c9a610afd14019b0d0304246e43cd9940deb4684e7`.
Caves evaluate a four-column halo: maximum tunnel travel 49 blocks plus radius below 3.3 requires
four, not two, origin columns. A wider-halo reference test guards against truncated border caves.
Ores, trees and lakes use one-column halos; all temporary work remains column/halo bounded.

First-time spawn selection is also Minecraft policy. It searches a bounded coordinate grid for a
dry v1/v2 surface, centers the initial 3x3 residency core there, then validates the highest local
voxel surface for stable support and headroom. Existing persisted players are never relocated and
the chosen position becomes ordinary player persistence. If the bounded startup neighborhood
contains no safe fresh spawn, opening fails explicitly instead of falling back to an underwater
player; existing terrain is preserved. The legacy new-v1-world sandbox is added only when both
of its origin columns already belong to the generated startup neighborhood. It must not create
partial columns at origin when a dry spawn was relocated elsewhere; a six-seed regression checks
that residency and valid spawn survive decoration. Existing worlds are never redecorated.
Authored diagnostic fixtures retain their independent startup policy. Generated water/lakes are static source
voxels; flowing fluids and swimming remain later gameplay. Lava and dungeons are deliberately
deferred because the current content profile lacks a complete lava medium/emission contract and
dungeons' spawner/chest/mob/loot semantics.

World files use `world.rcw` metadata and one signed-coordinate `.rcc` file per chunk column. The
container has an explicit world-format version, metadata schema version, chunk-payload version,
bounded lengths and BLAKE3 checksum. Chunk palettes persist semantic block keys and variants, never
`BlockId`. Metadata v2 distinguishes informational full-profile fingerprint, game/profile family,
persisted-state schema version and generator ID/version. Existing chunks are decoded by resolving
their semantic palette entries; a profile fingerprint change alone is not a rejection. Missing
semantic blocks and state-schema/family mismatches are explicit errors. Generator mismatch prevents
generation of absent chunks but does not prevent loading existing persisted chunks. Legacy metadata
v1 is treated as the known semantic-key + `u16` variant schema v1 and migrated to v2 only after
existing chunk palettes resolve successfully. Zlib/DEFLATE fast compression is selected only when
smaller; metadata/chunk writes use a cross-platform atomic replacement primitive, sync the completed
file, and sync the containing directory where supported. On Windows, directory sync is explicitly
unsupported/no-op: file contents are synced and replacement uses the platform replace-existing,
write-through operation, but RustCraft does not claim directory-entry power-loss durability there.
`WorldStorage::flush` completes no background jobs itself; workers must be joined/drained by their
owner, while flush syncs affected directory entries on platforms that support it.

The client supports `--world NAME` (default `default`) and `RUSTCRAFT_WORLD_SEED`; compatible
columns load, missing initial columns generate, and simulation mutations enter a separate
persistence-dirty queue. Save jobs are bounded and shutdown waits for active writes before flushing
remaining dirty columns. Player records use an outer v2 envelope (`player_id`, monotonic revision,
component count, checksum) and sorted independently versioned semantic components. The generic
storage layer bounds and preserves opaque component payloads; games own component IDs and codecs.
Minecraft currently registers transform, inventory, game-mode and pickup-receipt v1 components. Inventory
contains semantic ItemKey/count/durability stacks in all slots, cursor stack and crafting grid;
new components need only a game codec/registration, not a generic format or path change. Unknown
components are retained opaquely on load/save; missing required components or unsupported known
versions are compatibility errors. The old single-payload record is exposed as a legacy component
and migrated on the next checkpoint.

The client observes serialized durable state each fixed tick and increments a revision only when
components change. A single background worker holds at most one in-flight and one coalesced newest
snapshot. Checkpoints are scheduled every two seconds by default
(`RUSTCRAFT_PLAYER_AUTOSAVE_SECONDS`, clamped to 1–2); sync work never runs on movement ticks.
Storage alternates two per-player slots, writing the slot other than the newest valid revision,
syncing file data and (on Unix) the containing directory. BLAKE3 validation selects the newest
valid revision and falls back with a recovery diagnostic if one slot is corrupt. This targets at
most about two seconds of progress loss on an unexpected failure, subject to scheduling and
filesystem behavior; it is not a promise against hardware/controller caches that ignore sync.
Graceful shutdown submits and waits for the latest dirty revision and reports failure rather than
claiming success. Missing player records in older worlds remain first-time-player migration. The
startup 3x3 neighborhood is centered on the restored player chunk. Position/orientation, mode,
inventory/crafting/cursor and bounded pickup receipts are durable; player velocity and
grounded/contact state reset, bounds/camera are derived, and mining/input/UI/render state is
transient.

Chunk payload v3 extends the atomic `.rcc` column snapshot with bounded generic spatial records
and transfer tombstones. Each record has a stable 128-bit `EntityId`, monotonic entity revision,
semantic entity type, independently versioned game-owned payload, and its containing column as
spatial owner. Minecraft's first codec is `minecraft_b173:entity/item` v1: semantic ItemKey,
count, damage, position, velocity, age and pickup delay. Runtime numeric item handles and renderer
state are absent. Payload v2 loads as valid voxel data with zero entities and rewrites only on a
natural save. Unknown entity types, unsupported known schemas, missing items and corrupt entity
data are contextual compatibility errors, never an empty/regenerated chunk.

Spatial entities activate only after terrain publication. Spawn, merge, pickup/despawn, periodic
motion checkpoints and boundary crossing dirty their persistence domains. Eviction freezes the
entity column, captures its latest voxel+entity generation, waits for that exact save, then removes
simulation and terrain residency; dropped items therefore no longer pin terrain. A cross-column
move saves the destination first with a stable-ID tombstone naming the old source. Revision
deduplication makes either load order deterministic. Only after destination success is the source
rewritten, and only after source success is the tombstone pruned. Merge remains column-local so
survivor and consumed participant share one atomic snapshot.

Pickup crosses player and column files. Inventory plus a bounded `(EntityId, source-column)`
receipt is one player checkpoint; only its successful revision permits source-column deletion.
The receipt suppresses a stale source entity after recovery and is pruned after source absence is
durable. This is ordered idempotent recovery, not multi-file atomicity: failure may roll back to
the latest complete checkpoint, but repeated reopen/pickup cannot manufacture items and an
acknowledged item is not silently discarded.

World-global state uses generic `world-state.{0,1}.rcs` v1 checkpoints: monotonic revision, sorted
semantic/versioned opaque components, strict bounds and BLAKE3 checksum. Minecraft owns
`minecraft_b173:world/clock` v1, the fixed simulation-tick counter. It pauses while closed and
resumes exactly; no wall-clock catch-up, weather or day/night system is invented. Unknown global
components are retained opaquely; unsupported known versions fail compatibly. One background
coalescing worker retains at most one in-flight and one newest snapshot, checkpoints every two
seconds by default (`RUSTCRAFT_WORLD_AUTOSAVE_SECONDS`), and flushes the newest revision on graceful
shutdown. Unexpected failure can lose progress since the latest successful approximately
two-second checkpoint; graceful shutdown targets zero acknowledged durable-state loss, within the
platform sync contract above.

The current persistence matrix is:

| State | Policy | Domain / durable identity |
| --- | --- | --- |
| Voxel `BlockState` | durable | column; semantic BlockKey + variant |
| Dropped `ItemEntity` stack/transform/velocity/age/delay | durable | column; EntityId + semantic entity type/ItemKey |
| Local player transform/mode/inventory/cursor/crafting | durable | player ID + semantic components |
| Pickup recovery receipts | durable until acknowledged/pruned | player component + EntityId/source column |
| Simulation world time | durable | global `minecraft_b173:world/clock` component |
| Seed/generator/profile compatibility | durable metadata | `world.rcw` |
| Lighting | derived/rebuilt from voxels | none |
| Item bob/orientation and renderer extraction | derived | none |
| Player velocity/contact, mining progress, input intent | transient | none |
| Desired/Safe/Visible/Retained sets, queues and lifecycle tokens | transient | none |
| RenderWorld, meshes, GPU/atlas state and telemetry | derived/transient | none |
| Registries, recipes and crafting definitions | active profile definitions | not world snapshots |

Local chunk availability is managed by generic
`WorldResidency`: an interest center maps to signed chunk coordinates, a configurable load radius
(default 4, `RUSTCRAFT_STREAM_RADIUS=3..12`) and a retain radius one column larger (default 5).
Both use Chebyshev distance, so their boundaries are complete predictable squares rather than
lattice circles whose cardinal and diagonal reach differ.
Requests are distance-first, then biased by a bounded speed × sustained-travel-time horizon toward
a normalized 70% movement / 30% horizontal camera-direction blend. Movement controls bias magnitude;
looking around alone does not increase it or erase priority along actual travel.
The horizon is bounded to 12 seconds/three columns, per-tick displacement is clamped to 8 blocks,
and a heading change resets the duration so a short nudge cannot reprioritize the whole area.
Bounded load/generation workers return immutable results with request tokens. A bounded
`InitialLightingScheduler` builds full initial light arrays from owned voxel columns before their
atomic voxel/light publication; the simulation thread validates current interest and applies at
most one completed column in a callback under its shared wall-time budget. Only resident-neighbor
boundary reconciliation, eviction cleanup and incremental edits use the separate lighting queue.
Worker-built local lighting is immediately sufficient for first presentation; boundary
reconciliation remains authoritative and later dirties only sections whose light actually changed.
Thus a column does not wait behind unrelated boundary jobs before its first mesh can be built. The
bounded integration queue holds up to 256 columns (enough for the supported radius-6 sweep plus
transitions); queued work is reprioritized by current residency urgency while active work finishes.
Obsolete queued lighting work is cancelled at eviction and does not pin residency. Active
work finishes rather than leaving a partial authoritative update.
Initial-light jobs prioritize distance/forward bias with aging before worker start, coalesce stale
results by request token and do not preempt active work. One worker is the default, coordinated
with mesh/load/generation workers under a six-compute-heavy-worker client budget on an
eight-logical-CPU host; persistence workers are separate and I/O-oriented. Direct skylight is built by vertical scans; only shaded cells and actual emitters seed
relaxation. Initial local-light presentation is published once; completed boundary correction
invalidates only sections whose light changed, rather than rebuilding the whole column.
Missing storage is eligible for generation, while corrupt/incompatible data remains an explicit failure.
Unavailable columns are not treated as walkable air by collision or ray queries. Resident columns
outside the retain radius become eviction candidates on each fixed tick. Dirty columns stay resident
until the matching persistence generation is saved. Dropped-item columns use the same frozen
save-before-evict protocol and are not permanently pinned.
Eviction removes world/light state, render snapshots, and renderer/mesher residency. Initial
world assembly happens on a cancellable bootstrap worker after window creation, with a minimum safe
neighborhood centered on restored player position. After startup, input/event dispatch precedes
optional streaming work. The client retains a 2 ms default wall-clock streaming budget per event
turn (`RUSTCRAFT_STREAM_MAIN_BUDGET_MS`, clamped 0.25–8 ms), but reserves separate ordered windows
for critical completed-result application, boundary lighting, snapshot synchronization and mesh
scheduling. Streaming service runs after fixed-step/input processing on every event-loop turn,
rather than only on 20 Hz simulation ticks. Each expensive stage checks its own deadline, so a busy
earlier queue cannot consume every later stage's allowance. Urgency is end-to-end: the 3x3
REQUIRED safety core outranks the VISIBLE inner ring, which outranks PREFETCH; lookahead orders work
within a class but cannot promote distant prefetch above nearby terrain. Dirty section sets and
initial resident mesh scheduling are coalesced and drained by current urgency/distance; completed
mesh uploads prefer camera-forward sections within the same urgency ring. Boundary-lighting advances
in resumable 32-unit quanta; one
vertical-ray scan may remain indivisible. F3 exposes stage budget skips, consecutive turns without
service, dependency-block turns, backlog age and the nearest forward frontier lifecycle. These are soft budgets: one
indivisible operation can exceed its window and still requires profiling. The F3 summary is rebuilt
once per second. Radius 3 is a temporary diagnostic default, not a final view-distance decision.
This is local single-player residency, not a network interest protocol or general multiplayer
streaming system.

The four horizontal area sets are deliberately distinct:

- `Desired(c, Rload) = { p | max(|p.x-c.x|, |p.z-c.z|) <= Rload }` is request policy.
- `Visible` contains authoritative locally-lit columns whose current presentation has every
  expected resident section mesh available. Visible speculative terrain may extend outside Safe.
- `Safe` is the connected union of complete 3x3 Visible neighborhoods overlapping the existing
  Safe set, seeded by the complete 3x3 neighborhood around the restored player. Collision,
  movement, targeting, placement and breaking may use only Safe columns. Consequently
  `Safe => Visible`; the only transition occurs atomically inside result application.
- `Retained(c, Rretain, pins)` is the Chebyshev radius-5 square plus explicit active-light and
  dirty/save-in-flight pins. Persistable dropped items use save-before-evict instead. Pins extend storage lifetime only; they never
  redefine Safe or Visible.

Initial lighting supplies local authoritative voxel/light readiness. Cross-border reconciliation
is eventual convergence, not a safety prerequisite: it dirties only changed sections and never
revokes an already Safe column. The urgency order is current column, immediate Safe core, visible
travel area, directional prefetch, then outer Desired. Movement direction is the primary prediction
signal when optional lookahead is enabled; camera direction only breaks ties within the same
urgency class. Lookahead defaults off because the controlled route showed no useful margin or
latency benefit.

The game resolves camera medium through the compiled voxel material and liquid surface metadata;
it supplies the water fog policy. Generic world shaders apply distance-dependent fog to opaque and
translucent geometry without Minecraft block-ID checks. This presentation state does not affect
authoritative voxels or persistence.

## DX1 control tooling (accepted)

`control` owns game-neutral semantic control; `scripting-rhai` is a leaf adapter.
`minecraft-b173::control` owns first-party registration and the legacy simulation adapter.
Client/server compose the same scenario program. See [SCRIPTING.md](SCRIPTING.md).

DX1 async work has one bounded tooling worker (16 queued requests/replies), per-script request
generations and owner-thread atomic AST/handler publication. JobRef is used for compile/reload,
bundle writes and PNG writes, with cooperative scenario WaitJob. Render owns async GPU mapping;
workers receive owned bytes, never simulation/renderer borrows. See SCRIPTING.md for the audit.

## Post-DX1 consolidation boundary

The current ownership/normal dependency map and rechecked transitional debt are in
[PRE_M5_AUDIT.md](PRE_M5_AUDIT.md). D-045 requires bounded pre-M5 consolidation, not a general
rewrite. Engine provides mechanism; game provides policy. Native runtime commands/dense handles
are local contracts and must not be serialized unchanged as network schemas. M5 remains inactive.

## C1 operational configuration

The composition-owned rustcraft-config leaf supplies typed semantic policy, validation and requested/effective acknowledgement. Control, Rhai, native consumers and developer Settings share it; startup sources resolve once and native loops use typed fields. See [CONFIGURATION.md](CONFIGURATION.md) and [C1_REPORT.md](C1_REPORT.md). Engine mechanism remains independent of game policy; server remains headless.

UX1 text: `content::fonts` resolves bounded semantic role stacks through ordered startup package
composition; RendererResources supplies them to generic `render::text`. cosmic-text shapes/rasterizes
an explicitly populated bundled-only database. The renderer owns bounded CPU glyph generations and
one reusable GPU composite page. HUD/debug/console share this path; no game-policy/font-atlas leakage.

## Residency lifetime accounting

RSM1 extends existing shared diagnostics with scalar ownership accounting; inactive consumers do not
scan resident state. Current mesh tokens are globally non-reused and removed from the current-section
map on eviction. Admission bounds submitted-unconsumed plus ready payloads, allowing nonblocking
result sends and clean shutdown. Entity durable-owner metadata retires only after existing recovery
references permit it. Lighting rejects propagation into physically evicted columns and retires obsolete
cleanup on revisit. These mechanisms preserve game policy, Safe residency and save-before-evict.
RustCraft-owned GPU payload/capacity differs from allocator/driver physical residency.
See [RSM1_REPORT.md](RSM1_REPORT.md) and decision D-049.

## Client presentation

P1 keeps fixed 20 TPS authority and Control/Bot snapshots separate from transient client transforms.
Two position samples interpolate using the fixed remainder; local orientation previews only pending
human look and rebases when the tick consumes it. No visual write-back, persistence or server clock.
Discontinuities snap. Shared Presentation diagnostics contain bounded scalars, not frame resources.
See [P1_REPORT.md](P1_REPORT.md), decision D-050 and the conditional display/provider limitations.

## Persistence physical evaluation

S1 preserves the column/checkpoint backend and logical formats while measuring physical scaling.
Storage clones share fixed scalar IO totals; queue waits and current-only dirty timestamps feed
existing demanded Persistence diagnostics. No game receives raw paths or candidate backend access.
The benchmark-only split model lives in world examples/dev dependencies, not server composition.
Future backend capabilities must make durable acknowledgement, transient generations, revision
idempotency and ordered recovery explicit; component schemas remain separate from physical layout.
See [S1_PERSISTENCE_DECISION.md](S1_PERSISTENCE_DECISION.md).
