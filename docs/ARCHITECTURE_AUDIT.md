# Architecture alignment audit

This audit covers the implemented M0-M3 tree at the architecture-alignment pass. It distinguishes
actual dependency/policy leaks from directory naming. The alignment is incremental so accepted
gameplay and presentation behavior is preserved.

| Subsystem | Current owner | Correct target owner | Status | Action |
|---|---|---|---|---|
| Chunk/world voxel storage | `engine-core` | engine | OK | Keep compact, game-neutral storage. |
| `BlockState` orientation bits | `engine-core` | engine | OK | Retain compact typed state; put Beta metadata adapters in `minecraft-b173`. |
| Ray cast, AABB and collision math | `engine-core` | engine | OK | Keep policy-free primitives. |
| Mesh/material/texture/offscreen rendering | `render` | engine/render + tooling | OK | Renderer has no Minecraft IDs; content resolves descriptors before rendering. |
| Block definition registry | `mod-api` plus `gameplay-blocks` | `game-api` definitions plus game policy | Partial | Added generic `VoxelDefinition`/`GameRegistry`; migrate the legacy policy-rich registry without an M3 rewrite. |
| Namespaced content identity | mixed legacy strings | `game-api`/content | Fixed for active first-party definitions | Added validated `ContentId`; first-party IDs now use `minecraft_b173:`. |
| Package/system composition | implicit startup modules | `GameProfile` + public package registration | Fixed foundation | Added `GameProfile`, `GamePackage`, `Schedule` and profile validation at client/server composition roots. |
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
safe for the next milestone only if that rule is enforced.
