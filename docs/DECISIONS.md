# Architecture decisions

Keep entries short. Add a new entry when a choice changes a durable contract.

## D-001 — Familiar behavior, not exact Beta compatibility

Status: accepted.

Beta 1.7.3 is the gameplay/visual reference. Internal algorithms, timing implementation, protocol,
storage and historical bugs are not compatibility requirements.

## D-002 — Platform plus first-party gameplay modules

Status: accepted.

Generic engine/runtime code remains independent of Beta-specific gameplay. The default game is
assembled from native first-party gameplay modules.

## D-003 — Semantic controller boundary

Status: accepted.

Humans, network clients, bots, replays and tests feed semantic intent into the same simulation
boundary. Simulation code does not read device events directly.

## D-004 — Bots are first-class

Status: accepted.

The platform owns an Agent/Bot API. Bots should not need renderer emulation or reverse-engineered
client automation.

## D-005 — Server-defined content profiles

Status: accepted.

Clients and bots automatically resolve required server content. Packages have targets so headless
bots do not fetch graphical/audio-only resources.

## D-006 — Native first-party, sandboxed third-party executable modules

Status: accepted.

First-party gameplay runs as native Rust. Untrusted downloadable executable content is sandboxed;
servers must never cause automatic loading of arbitrary native libraries.

## D-007 — `just` is the canonical developer command surface

Status: accepted.

Cargo remains the build system. `just` provides stable human/agent recipes for checks, smoke runs,
diagnostics and optional tooling.

## D-008 — Advanced optimization is measurement-driven

Status: accepted.

SIMD, io_uring, custom allocators, NUMA, lock-free structures, special databases and similar
techniques require a representative workload and measurement before adoption.
