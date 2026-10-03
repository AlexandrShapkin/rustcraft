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
