# A1 report

Historical evidence for the recorded baseline; the registry owns current stage state.

## Baseline

Started from public `d2c21ea054650329c3dac9c59c4318b843170b5b` in the clean recovery clone
`/tmp/rustcraft-r2-recovered`. F1 was closed. Owner explicitly authorized A1, not C2 or M5.
The six live A1 issues were revalidated against source before implementation.

### Before: source/boundary map

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


## Scope completed

Stable external placement identity; durable Bot entity observations; typed game compatibility
payloads; semantic profile resolution for migrated consumers; bounded native work/recipe/drop
policy ownership; and admitted local principal/provenance/capability separation.
No networking, authentication, ECS or C2 composed-content implementation was introduced.

## Implementation

### After: identity and mutation path

`PlaceIntent` carries the existing `BlockKey`. `Simulation::place_semantic` requires a bound
`CompiledGameProfile`, resolves the semantic key to its compiled handle, and explicitly translates
back through semantic identity to the retained local registry. It never equates compiled and
historical handles. Unknown content, unavailable profiles and missing mappings return controlled
errors; rejected step admission is visible in Bot `last_action_error`. Existing target/adjacency,
held-item, collision, occupancy and quantity checks still perform the mutation. Local dense IDs
remain in simulation/storage/render adapters, not unqualified external placement contracts.
`AgentIntent` is not serialized as a future wire protocol.

Bot `ItemEntityObservation.id` is the existing authoritative `EntityId`. Movement, list reorder,
column migration and actual persisted reopen preserve identity. Identical stacks remain distinct;
merge keeps the recipient ID and retires consumed IDs; pickup/removal retires identity. The adapter
reports active loaded items, not a world census. Unloaded absence does not prove deletion; no new
truncation is introduced, and radius bounds blocks rather than item coverage. IDs grant no mutation.
Bot API version 3 introduced IDs; final version 4 deliberately versions typed game payloads.

### After: policy and compatibility boundary

`AgentIntent<G=()>` contains universal controller actions and a typed payload. Minecraft conveniences
are explicit `mod-api::legacy_actions::MinecraftActions`; Bot inventory/selection/mining data is
`bot-api::legacy::MinecraftObservation`. Control remains a distinct generic administration surface.
Client, Bot and headless producers use shared simulation admission; generic primary/secondary
intent also drives the independent sandbox game.

Minecraft composition registers semantic `WorkDefinition` and `RecipeDefinition` through public
native APIs. `minecraft-b173::policy` owns recipe ingredients/results, mining costs, preferred-tool
multipliers and drop rewards. Runtime resolves these once into local dense rules and uses public
`WorkProgress` and recipe matching. Historical recipe numeric constants/comparisons are removed.
Policy-rich old definitions live in `mod-api::legacy`, with source compatibility reexports.
No engine-to-Minecraft dependency or private package extension is added.

Untouched compatibility is explicit: legacy geometry/render/storage and frozen worldgen numeric
translation, inventory slot layout/2x2 transactions, tool-damage descriptors, drop movement/merge/
pickup conventions and existing durable transfer coordination. Minecraft's registration adapter
still reads authored legacy descriptors to supply the migrated semantic rules. This bounded A1
result does not claim all gameplay/container mechanisms have been generalized; C2 is not implemented.

### After: trust boundary

Control API version 2 adds validated `PrincipalId` and admitted `Provenance` with explanatory grant
origin/label. `Context` has no `Deserialize`; future external labels cannot mint admitted grants.
Trusted native callers explicitly compose local grants. Relabeling execution source preserves
principal and capabilities; `ServerAdmin`/`FutureChat` labels do not grant developer authority.
Mechanisms authorize capabilities. F1 local launch grants remain separate from gameplay mode.
CLI flags, Rhai availability, source names and process-local provenance are not authentication.
No remote principals, server role assignment, network sessions or wire protocol are implemented.
[D-061](DECISIONS.md#d-061--typed-game-intent-semantic-rule-admission-and-local-grant-provenance)
records the lasting contract.

## Evidence

- Reordered two-package compiled profiles resolve the same semantic placement despite changed
  dense handles; unavailable/unknown mappings reject without spending inventory.
- Reversed and renumbered Minecraft block/item registries preserve semantic empty state,
  log-to-planks output, mining speed, cobblestone reward, tool damage and semantic player roundtrip
  into the original registry.
- Bot movement/reorder/reactivation and merge/removal tests plus persisted column migration/reopen
  in either load order preserve existing IDs.
- Independent sandbox primary/secondary behavior and native work/recipe consumer have no runtime
  or Minecraft dependency. `just sample-game` checks this dependency boundary and produces an image.
- Production WindowEvent F3/F4/digit/focus/repeat capability tests remain green; F1 camera/input and
  P1 presentation policy are unchanged.
- Frozen worldgen hashes, semantic codecs, quantity-aware receipts, checkpoint durability and
  D-052 pickup/migration/failure ordering regressions remain green. Persistent formats are unchanged.

## Measurements

No performance improvement is claimed. Semantic policy resolution occurs at registration/setup,
not by replacing hot dense indexing with strings. A1 requires no representative GPU gate.
Sample-game used offscreen GL/llvmpipe solely for independent composition, not hardware performance.

## Tests

Focused placement/profile, Bot identity/controller, provenance grant/denial, independent-game,
renumbered Minecraft policy/codec, production WindowEvent and persistence/transfer tests passed.
`just smoke`, `just survival-scenario` and `just sample-game` passed sequentially.
Final implementation `just ci`: 340 nextest tests passed, two existing skips; formatting,
documentation/tooling and strict workspace all-target/all-feature Clippy passed.
All substantial Cargo commands used `CARGO_BUILD_JOBS=2` sequentially. Recovery-only debug/incremental
settings conserved disk without changing normal owner build profiles.

## Issues resolved

Final comments record distinct acceptance evidence and both-platform CI before completion:
[#8](https://github.com/AlexandrShapkin/rustcraft/issues/8),
[#10](https://github.com/AlexandrShapkin/rustcraft/issues/10),
[#11](https://github.com/AlexandrShapkin/rustcraft/issues/11),
[#12](https://github.com/AlexandrShapkin/rustcraft/issues/12),
[#13](https://github.com/AlexandrShapkin/rustcraft/issues/13),
[#14](https://github.com/AlexandrShapkin/rustcraft/issues/14).

## Issues remaining / waivers

No unresolved A1-owned issue or waiver. No new issue was created. Unrelated issues were untouched.

## Known limitations

Local provenance is not remote authentication. Explicit retained compatibility is listed above;
these local adapters do not define new universal external contracts. Future M5 wire protocols and
C2 composition remain separate work. No save-format migration or performance claim is made.

## Implementation SHA

- `6cfdf7f9b79ca45dd049e108028bd1d8745515c5`: semantic placement and durable Bot entity identity.
- `9f03fb56a7e609b4786e527c77876d819b878476`: typed game adapters, semantic policy and local provenance.

## CI

- [Identity batch CI 37375463910](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37375463910): Ubuntu success, Windows success.
- [Policy/trust batch CI 37377613595](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37377613595): Ubuntu success, Windows success.
- Closeout CI is discoverable by the closeout commit below; acceptance requires both platforms green.

## Closeout SHA

The commit titled `Record A1 acceptance` introducing this finalized report is the closeout identity;
resolve with `git log -1 --format=%H --grep='^Record A1 acceptance$'`.
Registry closes A1, advances focus to C2 and leaves C2 planned. C2 implementation was not started.
