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
something else. Legacy M0-M3 break/place/inventory/crafting fields remain temporarily for
compatibility and are game conveniences, not permanent universal semantics. The simulation must
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
BotAction still embeds transitional AgentIntent while its game-convenience adapter migration is active.
Current positions have no space context.
A1 owns semantic external placement and durable-entity references, preserving distinct Game extension,
Control administration, Agent intent and Bot observe/act responsibilities rather than one giant API.
No new external contract may export dense handles without an explicit authoritative profile mapping.

VS1 adds stable VoxelSpaceId and local/reference-space positions where required. Bots must reason about
moving structures, entities and relative motion from semantic observations, not renderer-coordinate
scraping. References identify durable entity identity and relevant frame; contacting a space does not
imply permanent ownership. No Bot API version bump or new types are implemented in this docs pass.

## A1 durable item observation identity

Bot API version 3 exposes existing authoritative EntityId in each ItemEntityObservation. Movement,
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
use an explicit local compatibility registry; their active migration is tracked separately in A1.
