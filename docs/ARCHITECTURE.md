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
grouped by `TextureHandle`, so one model may reference several pages. Crack, container, HUD-sheet
and player-skin passes bind their resolved page explicitly. The sole production page-zero bind is
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
for the inventory background, HUD sheet and player skin. Those role names remain transitional M3
UI ownership debt, but paths and Beta atlas coordinates no longer enter the generic renderer.

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

## M4 world foundation (in progress)

`engine-core` remains policy-free: `ChunkBuilder` creates unpublished dense sections, and
`World::publish_column` replaces a finished set of signed-Y sections at one publication boundary.
The new `rustcraft-world` crate owns generic generator/scheduler contracts, lifecycle tracking,
versioned storage, semantic palettes, persistence dirty generations and bounded save workers. It
does not depend on Minecraft block definitions. The flat generator and `sandbox-test` both compile
against the same generator contract.

`minecraft-b173::worldgen` currently supplies the first game policy: a fixed 128-block overworld,
sea-level water, surface/bedrock rules, coordinate-seeded cave/ore/tree features and a narrow legacy
block-handle adapter while the simulation still uses M0-M3 IDs. Feature origins are sampled from a
deterministic neighborhood halo and clipped to each output column. Generation returns authoritative
sections only; it does not build render snapshots or meshes.

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
smaller; all writes use create-new unique temporary files, file sync, atomic rename and
parent-directory sync.

The client supports `--world NAME` (default `default`) and `RUSTCRAFT_WORLD_SEED`; compatible
columns load, missing initial columns generate, and simulation mutations enter a separate
persistence-dirty queue. Save jobs are bounded and shutdown waits for active writes before flushing
remaining dirty columns. Player records use an outer v2 envelope (`player_id`, monotonic revision,
component count, checksum) and sorted independently versioned semantic components. The generic
storage layer bounds and preserves opaque component payloads; games own component IDs and codecs.
Minecraft currently registers transform, inventory, and game-mode v1 components. Inventory
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
inventory/crafting/cursor are durable; velocity and grounded/contact state reset, bounds/camera
are derived, and mining/input/UI/render state is transient. Lighting is recomputed; dropped
entities and world time remain unpersisted. Initial client loading still has a synchronous startup
barrier while worker results and local file reads are assembled; moving that barrier into a loading
state remains outstanding.

The game resolves camera medium through the compiled voxel material and liquid surface metadata;
it supplies the water fog policy. Generic world shaders apply distance-dependent fog to opaque and
translucent geometry without Minecraft block-ID checks. This presentation state does not affect
authoritative voxels or persistence.
