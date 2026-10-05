# Architecture alignment audit

The table below is historical M0-M3 alignment evidence, not the current next-stage plan.
The post-DX1 [PRE_M5_AUDIT.md](PRE_M5_AUDIT.md) rechecks every ARCH finding against public main
16c6823a and supersedes this document's stale R1.0 one-page/temporary-atlas actions. R1.1/R1.2
already resolve those foundations. R2 has now removed historical HUD/skin source-layout coupling;
runtime policy, controller/definition compatibility and destination-layout policy remain migration debt.
The current post-R2 source-to-plan table below and ROADMAP supersede historical next-step actions.

This audit covers the implemented M0-M3 tree at the architecture-alignment pass. It distinguishes
actual dependency/policy leaks from directory naming. The alignment is incremental so accepted
gameplay and presentation behavior is preserved.

| Subsystem | Current owner | Correct target owner | Status | Action |
|---|---|---|---|---|
| Chunk/world voxel storage | `engine-core` | engine | OK | Keep compact, game-neutral storage. |
| `BlockState` orientation bits | `engine-core` | engine | OK | Retain compact typed state; put Beta metadata adapters in `minecraft-b173`. |
| Ray cast, AABB and collision math | `engine-core` | engine | OK | Keep policy-free primitives. |
| Mesh/material/texture/offscreen rendering | `render` | engine/render + tooling | R1.0 foundation fixed | Renderer accepts `TextureHandle`/arbitrary `AtlasRegion` and resolved paths; one-page upload and Beta adapter remain for R1.1. |
| Block definition registry | `mod-api` plus `gameplay-blocks` | `game-api` definitions plus game policy | Partial | `VoxelDefinition` is semantic and numeric-ID-free; compiled registry is indexed. Migrate policy-rich legacy consumers incrementally. |
| Namespaced content identity | mixed legacy strings | `game-api`/content | R1.0 fixed foundation | Owned validated common identity plus typed keys support runtime-loaded content; legacy APIs remain migration debt. |
| Package/system composition | implicit startup modules | `GameProfile` + public package registration | R1.0 fixed foundation | Typed package/resource keys and canonical package-plus-lexical-block ordering produce profile-local dense handles and a declared default state. |
| Engine schedule and commands | direct `Simulation::step` mutation | engine/Game API mechanisms | Partial | Added native registered schedules and `CommandBuffer::SetBlock`; migrate real systems when touched. |
| Movement/collision loop | `runtime` | engine runtime mechanism | Mixed | Extract only when the next engine consumer needs it; do not add Minecraft policy to this portion. |
| Inventory and `ItemStack` | `runtime`/`mod-api` | generic container primitives where useful; Minecraft policy in game package | Policy leak | Record migration; preserve M3 behavior. Define capabilities from demonstrated non-Minecraft needs before extraction. |
| Mining/hardness/tool tiers | `runtime`/`mod-api` | `minecraft-b173` | Policy leak | Mandatory migration before materially extending mining. Do not add more mining policy to generic runtime. |
| Drops/item entities/pickup | `runtime` | generic entity/command mechanisms plus `minecraft-b173` policy | Policy leak | Split spawn/motion mechanism from drop/pickup rules when this area next changes. |
| Recipes/crafting | `runtime` | `minecraft-b173` through public Game API | Policy leak | Move recipe definitions and matching system behind the game package before adding more crafting. |
| First-party blocks/flat world | `gameplay-*` | `minecraft-b173` | Partial/fixed boundary | Aggregated and re-exported only through `minecraft-b173`; clients no longer depend on gameplay crates directly. |
| Semantic input/controller | `agent-api`, client, runtime | platform/Agent API + game systems | Partial | Added `primary_action`/`secondary_action`; legacy Minecraft aliases remain for behavior-preserving migration. |
| Bot observations/actions | `bot-api`, runtime | generic Agent/Bot API + game-specific convenience layer | Partial | Semantic IDs/capabilities exist; crafting/mining/inventory convenience fields remain an explicit compatibility exception. |
| Beta HUD/inventory presentation | `render::hud` and client | generic UI renderer + `minecraft-b173` UI construction | Narrowed by R2 | Semantic subresources remove physical sheet coupling; destination layout and slot policy remain organizational debt. |
| F3/process/GPU telemetry | client/render tooling | client/tooling | OK | No gameplay authority or headless dependency. |
| Headless server | server + runtime | platform composition root | Partial | Remains GPU/window independent; currently composes legacy runtime behind validated Minecraft profile. |
| Non-Minecraft integration | absent before this pass | architectural test game | Fixed | Added `sandbox-test` and `just sample-game`; dependency guard rejects Minecraft/runtime crates. |

## Dependency findings

`engine-core` has no crate dependencies. `render` depends on `engine-core` and graphics libraries,
not game packages. `game-api` depends only on `content` and `engine-core`. No engine-to-Minecraft
Cargo edge was found.

The concrete violation was at the orchestration boundary: client/server directly imported
`gameplay-blocks` and `gameplay-flat-world`, while `runtime` presents Minecraft survival policy as
if it were universal runtime behavior. Client/server now depend on the `minecraft-b173` package
boundary. The runtime split is not safe to complete in a bounded behavior-preserving pass and is
tracked above.

## Safe next-step rule

New engine mechanisms may land in engine/Game API crates. New Minecraft rules land in
`minecraft-b173` and register through the public extension surface. Before modifying a legacy
mixed subsystem, first migrate the portion needed by that change; do not deepen the leak. This is
safe for incremental local work only if that rule is enforced. Before M5, the stricter A1 identity/
replication-touched policy gate in PRE_M5_AUDIT.md also applies.

## R1.0 numeric-identity audit

Repository-wide searches classify the remaining zero/historical IDs as follows:

- `GameRegistry::compile` deliberately assigns the declared profile default to handle zero. Code
  obtains that handle from `CompiledGameProfile::default_state`; zero carries no Minecraft meaning.
- `RenderWorld::default` uses zero only as an explicit standalone diagnostic sentinel. Extraction
  from a real `World` copies its declared default, including a tested nonzero default.
- raw zero checks and `BlockId(0)` values in `engine-core`, `render`, `runtime`, lighting,
  survival and flat-world sources are inside isolated unit/diagnostic tests with local registries.
- `AIR.id`, `STONE.id`, `GRASS.id` and other first-party constants occur in Minecraft client/server,
  benchmarks, tests and the accepted M0-M3 compatibility runtime. They are game-specific legacy
  policy, not generic engine semantics.
- `minecraft-b173::legacy_block_id` validates semantic compiled presence and translates to the
  historical registry only for accepted M0-M3 consumers. Canonical compiled handles are not
  required to equal those historical numbers.

No generic production renderer/world path treats numeric zero as Minecraft air, and no authored
Game API definition selects a runtime `BlockId`.

## Current source-to-plan gaps (post-R2 `5eb1770`)

Read-only source inspection; planned limitations are not automatically functional defects.
R2 semantic source-region/page resolution is implemented. D-052 pickup/partial/migration ordering and
failed-snapshot retirement are implemented, independently of R2. Historical acceptance reports remain
unchanged; the original PRE_M5 table describes its older baseline.

| Current assumption / source anchor | Gap against accepted target | Owner | Required invariant after migration |
| --- | --- | --- | --- |
| client `window_event`: dev_key gated by devtools; just client-survival/dev-client | Normal F3 chords and tooling admission use different routing | F1 / DX2 | One normal path; semantic action then capability check; aliases retire only with equivalent coverage |
| runtime GameMode Development/Survival; Control Context source/capability strings | Gameplay state/local grant composition is not authenticated session roles | F1 / A1 / M5 | Independent gameplay mode/principal/roles/grants; host authenticates and authorizes |
| agent-api PlaceIntent.block: BlockId; BotAction wraps AgentIntent | Profile-local ID cannot be unchanged external identity | A1 | Semantic stable identity or explicit authoritative profile mapping |
| bot-api ItemEntityObservation lacks EntityId | Collection positions cannot identify durable entities | A1 | Stable EntityId in observations needing durable references |
| mod-api BlockDefinition/ItemDefinition; runtime inventory/mining/crafting | Separate policy-rich legacy families and mixed ownership | A1 / C2 | Public composed definitions; game policy outside generic mechanisms |
| game-api VoxelDefinition/CompiledVoxelDefinition | Semantic voxel foundation lacks full category/state/handler model | C2 | Typed composition and indexed runtime capabilities, no untyped hot-path maps |
| engine-core BlockState.variant and orientation bit accessors | Global fixed bit conventions do not scale per definition | C2 | Deterministic block-local canonical schemas with compact encoding and semantic compatibility |
| game-api CollisionDescriptor Empty/FullCube; core World::collides and runtime solid/AABB checks | No generalized collision/selection shape contract | BG1 | Independent configured shapes, composed-AABB fast path and coherent selection |
| render geometry::FACES, cube inspection/meshing; six face resources | Full-square neighbor culling cannot describe partial/general models | BG1 | Semantic models and full/partial/no coverage; cube optimized common case |
| engine-core orientation::ModelRotation signed orthogonal matrices | Cube-orientation foundation is not arbitrary authored geometry | BG1 | Deterministic model/state transforms without named stair/wedge engine classes |
| core raycast::cast tests targetable voxel cells | Ray hits cell entry, not configurable non-full selection shape | BG1 | Traversal plus actual selection-shape intersection |
| World.sections/lights keyed by root SectionPos | One grid is not multiple independently movable local grids | VS1 | Stable space identity, local storage/relationships and cheap static path |
| BlockPos/WorldCommand SetBlock lack space context | Coordinates ambiguous across grids | VS1 | Explicit space-local addressing and intentional root adapters |
| render section snapshots/jobs/GPU keys use root SectionPos | Motion cannot be independent from local mesh identity | VS1 | Space+local-section identity; transformed bounds; no motion-only remesh |
| world WorldStorage chunks/x.z.rcc; column owner/tombstones | Root namespace cannot universally own many structures | VS1 | Versioned space/parent/motion/local-content durability; ordered transfer |
| runtime ItemEntity/root Vec3 and entity column ownership | No entity reference-frame contract | VS1 | Relative/linear/angular motion; frame distinct from contact/durable ownership |
| sandbox-test registers generated voxels through public APIs | Independent proof exists, but not all future shape/space categories | DX2 / READY1 | Project-owned diagnostic content proves generic contracts without Minecraft |
| client/server specialist CLI paths; numerous just recipes | Coverage equivalence not proven | DX2 | Same product composition via Control/scenarios; preserve unique assertions |
| Large composition/service modules across client/world/render/runtime/control/Rhai | LOC alone does not establish ownership/change locality | RF1 | Measured graph/hotspots, clearer ownership and concise CODE_MAP |

Current normal crate edges remain inward; no generic engine-to-Minecraft edge appears in manifests.
Game API uses content/core; render-profile bridges content/game/core to rendering; server excludes
rendering. Control administers semantic commands, Agent feeds intent, Bot observes/acts, and native
Game commands mutate through CommandBuffer. Do not merge these responsibilities merely to shorten
paths. minecraft-b173 still wraps the transitional runtime; sandbox-test avoids it deliberately.

Source size inventory (lines, including tests; not a refactor verdict): client main 7,398; world lib
4,452; render lib 3,836; scripting-rhai lib 2,514; runtime lib 2,023; server main 1,952; control lib
1,524. ClientApp composes input/fixed/presentation/residency/persistence/diagnostics; Simulation mixes
mechanisms/game policy; world/render libraries combine codecs/schedulers/backend or GPU/geometry.
RF1 must measure module/type/function connectivity, repeated Git change hotspots, cycles and adapters
before choosing decompositions; no call-graph measurement or benchmark result is claimed here.

Source navigation: engine-core lib/orientation/raycast; game-api and mod-api lib; runtime lib;
render geometry/inspection/lib/hud; render-profile lib; content resources; Minecraft lib and durable
codecs; agent/bot/control lib; scripting-rhai lib/worker; client main/devtools/developer_input; server
main; world lib; sandbox-test main; justfile. Current deferred cube/global-space limits belong to
BG1/VS1, not new defect IDs. [VOXEL_SPACES](VOXEL_SPACES.md) owns complete spatial direction.
