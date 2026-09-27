# Product definition

## What this is

A Rust voxel sandbox that feels recognizably like Minecraft Beta 1.7.3 while being built as a
modern platform rather than a compatibility layer for the Java implementation.

Core loop:

`explore -> mine -> collect -> craft -> build -> survive -> automate -> explore`

Expected familiar systems eventually include blocks, terrain, inventory, tools, crafting,
lighting, fluids, health, mobs, farming, furnaces, simple redstone-like automation and
multiplayer.

## What compatibility means

Compatibility is behavioral and experiential, not internal. A lever should power a nearby lamp;
a door should respond to power; water should spread in a familiar block-based way; mining,
placing, movement and collisions should feel expected.

Accidental update-order behavior, Java object structure, packet format, save format, exact seed
compatibility and bug-dependent contraptions are not requirements.

## Visual direction

Keep the old game's baseline readability and style. Do not make realism the objective. Distant LOD,
better culling, batching, modern GPU APIs and larger view distance are valid optimizations if the
ordinary gameplay image does not become worse.

Not baseline goals: ray tracing, global illumination, volumetric effects, realistic fluids,
rigid-body block physics, ecosystem simulation, seasons, GOAP-heavy or neural mob AI.

## Platform direction

The executable is a platform. The default Beta-like game is first-party content/modules. A server
may define a different content profile. Clients and bots should join without manual loader/mod-pack
installation steps.
