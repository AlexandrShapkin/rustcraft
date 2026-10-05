# Product definition

RustCraft is an extensible, performance-oriented voxel engine/runtime in Rust. Minecraft Beta
1.7.3 is a first-party game package and reference implementation, not the identity of the engine.
The default client/server composition still embeds that package during this transition.

The product is pre-1.0. The first planned formal version was `0.1.0-alpha.1`, selected as a clean
starting point because the repository had no prior release tags or dependable milestone-to-version
history. `0.1.0-alpha.2` is the next release candidate after public CI portability fixes; no stable
API or completed M4 scope is implied. Milestones describe implementation scope, not release
numbers.

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

Keep the old game's baseline readability and style. Do not make realism the objective. Mipmapping,
filtering, compression, LOD, culling, batching, dynamic quality, modern GPU paths and larger view
distance are valid optimizations. Moderate visual differences are acceptable for meaningful
performance, frame-time, memory, scalability or extensibility gains when gameplay remains
recognizable and semantically clear.

Not baseline goals: ray tracing, global illumination, volumetric effects, realistic fluids,
global per-block rigid-body physics (every voxel an independent body), ecosystem simulation,
seasons, GOAP-heavy or neural mob AI. Realism for realism’s sake remains outside the baseline.
Optional composite physics for selected voxel spaces/structures is now an explicit planned platform
capability, preserving the performance-first static path; see [VOXEL_SPACES](VOXEL_SPACES.md).

## Platform direction

The executable is a platform. The default Beta-like game is first-party content/modules. A server
may define a different content profile. Clients and bots should join without manual loader/mod-pack
installation steps.
