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

The simulation should consume concepts such as movement, look direction, jump, crouch, attack,
use/interact, break/place, inventory actions and crafting actions. It must not depend directly on
window-system events or device key codes.

## Bot model

Use `observe -> decide -> act`. Do not grant a bot an unrestricted mutable `World` merely because
it runs locally.

Observations may include self state, inventory, nearby blocks, raycast target, relevant entities,
time/weather and events. Actions may include movement, interaction, block actions, inventory,
crafting and communication where permitted.

The semantic Bot API must be versionable and should remain stable across internal ECS/chunk/runtime
rewrites.

## Mod-aware bots

Bots should be able to reason about unknown server-added content through semantic definitions:
block/item/entity IDs, tags, properties, preferred tools, recipes and interaction capabilities.
They should not require textures or audio to understand modded content.

## Headless stepping

Architecture should support controlled stepping for tests/replay/AI experiments:

`observe N -> submit intent -> advance -> observe N+1`

This also enables cheap load testing with large numbers of headless agents.
