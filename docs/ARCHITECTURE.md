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
