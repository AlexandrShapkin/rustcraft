# Architecture

## Layers

### `engine-core`
Generic low-level primitives: typed IDs, coordinates, chunk/world storage foundations, geometric
primitives, deterministic helpers and other Minecraft-agnostic data structures.

It must not know about diamond, creepers, recipes, redstone, furnaces or Beta-specific block names.

### `mod-api`
Semantic registration/contracts for gameplay definitions and systems. It should encourage bulk or
scheduled work, not one extension-boundary crossing per block per tick.

### `agent-api`
Small semantic control contract between simulation and controllers. The simulation consumes
intent; it does not consume keyboard or mouse events.

### `bot-api`
Versionable observation/action concepts exposed to trusted native bots, sandboxed bots and future
remote bots. It builds on semantic IDs rather than internal pointers.

### `content`
Content manifest/package/target/hash concepts. This becomes the foundation for server-defined
content resolution, cache verification and target-specific downloads.

### `runtime`
Startup/orchestration, registries, module ordering, schedules, world lifecycle and integration
between the generic engine and first-party gameplay modules.

### `gameplay-*`
Native first-party modules that assemble the default game. These may use direct native execution,
generics/static dispatch and compact indexed registries.

### `client`
Eventually owns windowing, rendering, input and audio adapters. Input is translated into agent
intent before entering simulation.

### `server`
Authoritative/headless host. It must not require GPU, window or audio.

## Data and execution

Prefer typed numeric IDs, contiguous storage, explicit dirty queues, batched work, compact block
states and partitioned ownership. ECS is appropriate for many entities but is not a mandatory
universal abstraction for chunks, networking or storage.

Simulation and rendering are independent. Use fixed-step logic where it simplifies gameplay and
incremental/event-driven work where it avoids waste without changing expected behavior.

Do not introduce distributed sharding or microservices until a clean single-process server exists
and measurement shows a need.
