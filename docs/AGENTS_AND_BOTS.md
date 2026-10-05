# Agents, controllers and Bot API

Bots are a native platform feature.

## Controller model

A controllable player/entity receives semantic intent from a controller. Candidate controllers:

- local human;
- network player;
- native bot;
- sandboxed WASM bot;
- remote Bot API client;
- replay controller;
- deterministic test/script controller.

The universal boundary consumes movement, look direction, jump, crouch and generic primary or
secondary actions. A game package decides whether those actions mean mine, place, interact or
something else. `AgentIntent<G=()>` carries only generic movement/look/jump/crouch/primary/secondary fields and a
typed game payload. Explicit mod-api legacy_actions::MinecraftActions holds retained local
attack/use, hotbar, break/place, crafting and inventory conveniences. Generic Control scenarios
use the unit payload; normal Minecraft controllers use PlayerIntent. This is not a wire protocol. The simulation must
not depend directly on window-system events or device key codes.

## Bot model

Use `observe -> decide -> act`. Do not grant a bot an unrestricted mutable `World` merely because
it runs locally.

Observations may include self state, inventory, nearby blocks, raycast target, relevant entities,
time/weather and events. Actions may include movement, interaction, block actions, inventory,
crafting and communication where permitted.

The semantic Bot API must be versionable and should remain stable across internal ECS/chunk/runtime
rewrites.

Survival observations expose nearby dropped item stacks, the selected semantic item (including
tool durability), inventory slots and mining progress. Survival actions use the same intent path
as a human controller: bots select hotbar slots, hold/release breaking, place blocks, manipulate
inventory and request registered recipes. They do not receive direct world or inventory mutation.

## Mod-aware bots

Bots should be able to reason about unknown server-added content through semantic definitions:
block/item/entity IDs, tags, properties, preferred tools, recipes and interaction capabilities.
They should not require textures or audio to understand modded content.

Generic observations expose namespaced definitions and capabilities. Minecraft-specific helpers
may be layered above them; they do not define the universal Agent API.

## Headless stepping

Architecture should support controlled stepping for tests/replay/AI experiments:

`observe N -> submit intent -> advance -> observe N+1`

This also enables cheap load testing with large numbers of headless agents.

## DX1 scenario controller

An explicit temporary scenario lease overrides human/legacy diagnostic intent. Movement uses
AgentIntent; terminal completion/cancellation releases intent. Teleport is a separate privileged
action. The shared FixedControl gate aligns headless and graphical pause/step behavior.

## Current gaps and target A1/VS1 contracts

A1 migrates `PlaceIntent.block` to existing BlockKey and nearby dropped items to durable EntityId.
BotAction and ScriptedBot accept a typed game payload; Observation uses the same separation.
Minecraft inventory/selection/mining observation is explicit bot-api::legacy::MinecraftObservation.
Current positions have no space context.
A1 owns semantic external placement and durable-entity references, preserving distinct Game extension,
Control administration, Agent intent and Bot observe/act responsibilities rather than one giant API.
No new external contract may export dense handles without an explicit authoritative profile mapping.

VS1 adds stable VoxelSpaceId and local/reference-space positions where required. Bots must reason about
moving structures, entities and relative motion from semantic observations, not renderer-coordinate
scraping. References identify durable entity identity and relevant frame; contacting a space does not
imply permanent ownership. A1 versions the present Bot contract; VS1 space-context types remain future work.

## A1 durable item observation identity

Bot API version 4 exposes existing authoritative EntityId in each ItemEntityObservation. Movement,
observation reordering and column transfer/reopen do not assign new IDs. Identical stacks remain
distinguishable. Merge retains the recipient ID and retires consumed IDs; removal/pickup retires an
entity rather than reusing a list position. Current nearby_items reports the active loaded item list,
not a complete world census; unloaded items are absent and absence alone does not prove removal.
There is no new truncation in this adapter. Radius bounds block observations, not item coverage.
Bots gain no mutation authority from receiving an ID. This version change does not define a network
codec or change persisted entity identity/format.

A1 placement admission resolves existing BlockKey through a bound authoritative CompiledGameProfile,
then translates explicitly to retained local registry IDs. Missing profiles, unknown keys and missing
local mappings fail explicitly; no dense handle crosses this external boundary. Simulation records
rejected intent admission in last_action_error, and direct place_semantic returns Result. Target,
adjacency, held-item, collision and quantity checks remain shared with local controller placement.
AgentIntent is not a serialized network protocol. Retained historical world/storage consumers still
use explicit local compatibility adapters retained by A1. These do not qualify numeric IDs as
external identity.

Version history during A1: version 3 introduced durable EntityId; version 4 separates typed game
observation/action payloads from generic Bot contracts. A server must negotiate/validate a future
external codec deliberately; Rust AgentIntent/Simulation structs are not that codec.
