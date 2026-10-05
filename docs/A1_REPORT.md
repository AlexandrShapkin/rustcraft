# A1 report — active implementation

Starting public baseline: d2c21ea054650329c3dac9c59c4318b843170b5b.
A1 active, focus A1; C2 remains planned. No networking or save-format change.

## Source/boundary map

- agent-api AgentIntent/PlaceIntent -> runtime Simulation::step -> target/held-item/collision checks
  -> dense world mutation. PlaceIntent currently leaks BlockId; compatibility fields mix game actions.
- bot-api BotAction/ScriptedBot -> Controller -> same Simulation; Simulation::observe translates
  legacy registry content names but discards ItemEntity.id. BOT_API_VERSION starts at 2.
- runtime survival ItemEntity carries engine-core EntityId and persistence_revision; runtime transfer
  receipts/column snapshots and minecraft-b173 semantic persistence codecs already preserve that ID.
- game-api GameRegistry -> CompiledGameProfile maps existing BlockKey to profile-local BlockId.
  minecraft-b173 registers VoxelDefinition through public GamePackage, but flat/world/runtime adapters
  deliberately translate semantic keys back to retained historical registry handles.
- mod-api BlockDefinition/ItemDefinition contain generic geometry/light descriptors mixed with
  hardness/tool/drop/placeable policy. gameplay-blocks defines Minecraft values; runtime inventory,
  survival recipe defaults, mining, drop/pickup and step consume these directly.
- minecraft-b173 owns first-party composition and semantic save adapters, depends on runtime through
  public APIs. Runtime does not depend on minecraft-b173; that direction must remain intact.
- Control Context carries source/capability labels; F1 LocalSession composes local grants separately
  from game mode. No current source label is authenticated provenance.
- sandbox-test composes independent GameProfile/CommandBuffer/Schedule and generic primary intent
  without minecraft-b173. It is the independent-game proof path for generalized mechanisms.

Native source navigation was sufficient after CodeGraph reported no recovery-clone index.

## Pending acceptance

Issues #8, #10, #11, #12, #13, #14 revalidated open. Each requires its own proof/CI before closure.
Bounded slices will preserve internal dense IDs, frozen worldgen and semantic persistence/transfer
ordering. This progress report is not stage acceptance.

## Identity slice implementation

PlaceIntent now uses existing BlockKey; a bound CompiledGameProfile resolves semantic identity
before explicit translation to retained historical runtime IDs. Reordered two-package profiles
request the same content; missing/unavailable mappings and unknown keys fail without inventory loss.
Bot API version 3 exposes existing durable EntityId. Tests cover movement/reorder/reactivation,
merge/removal semantics, and observations after actual persisted migration/reopen in either order.
Local intent owners now clone retained requests or extract timing flags before moving owned intent.
No whole-AgentIntent wire serialization or persistent format change.
