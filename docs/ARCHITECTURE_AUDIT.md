# Architecture alignment audit

The table below is historical M0-M3 alignment evidence, not the current next-stage plan.
The post-DX1 [PRE_M5_AUDIT.md](PRE_M5_AUDIT.md) rechecks every ARCH finding against public main
16c6823a and supersedes this document's stale R1.0 one-page/temporary-atlas actions. R1.1/R1.2
already resolve those foundations. Runtime policy, controller/definition compatibility and active
HUD/skin layout debt remain current; A1/R2 bound their pre-M5 closure.

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
| Beta HUD/inventory presentation | `render::hud` and client | generic UI renderer + `minecraft-b173` UI construction | Organizational debt | Preserve accepted M3 fidelity; move Beta layout/policy when UI is next extended. |
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
