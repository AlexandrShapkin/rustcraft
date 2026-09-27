# Initial Codex prompt

Work in this repository as the primary implementation agent.

Read `AGENTS.md` first and then the documents it requires. Inspect the existing workspace before
changing architecture. Use the repo-local skills when relevant. Use `just` as the normal command
surface. If Serena and Context7 are available, use them where they materially help; do not block if
they are absent.

Run `just doctor` and `just refs-status` early. Reference repositories/assets are research inputs,
not production dependencies. If a required public reference is missing and network access is
available, `just refs-fetch` is the supported setup path. Do not build or execute reference projects
merely to inspect them.

Implement M0 from `docs/ROADMAP.md` as one coherent headless vertical slice. Do not attempt the
whole game and do not stop after creating interfaces.

The batch must leave the repository with real working paths for:

- typed block/world/chunk primitives and a multi-chunk world container;
- module-driven block registration and a flat-world first-party module;
- semantic Agent/Controller intent;
- player position/velocity/AABB, basic movement, gravity and voxel collision;
- explicit block break/place operations;
- initial semantic Bot API observations/actions plus a simple scripted/headless bot/controller;
- initial Content Manifest / package ID / hash / dependency / target (`client`, `server`, `bot`)
  foundations and deterministic target filtering/hash validation;
- a deterministic headless executable scenario proving module registration, world creation,
  movement/collision, observation/action, block break and block placement end-to-end.

Keep renderer, full QUIC networking, production WASM runtime, databases, io_uring, custom
allocators, explicit SIMD, distributed sharding and full Beta worldgen out of this batch unless a
tiny abstraction is necessary to avoid a bad dependency.

If implementation needs familiar Beta behavior, read `reference/SOURCES.md`, choose the narrowest
relevant reference and inspect only what answers the concrete question. Cross-check important
ambiguous behavior when useful. Do not port Java/C++/Rust reference structure and do not copy
reference code. The project contract, not historical bugs, defines the target behavior.

Add focused tests for coordinate conversion including negatives/boundaries, block indexing,
registration, collision/movement primitives, placement/removal, controller intent and basic content
target/hash behavior. Do not build a giant compatibility test framework.

Before finishing, run the applicable recipes, preferably:

- `just bootstrap-check`
- `just ci`
- `just smoke`

Fix blocking failures. Record unrelated non-blocking defects in `docs/DEFECTS.md` rather than
spending the whole batch on polish. Record durable architecture changes in `docs/DECISIONS.md`.
When a reference investigation produces a durable conclusion, record a concise project-authored
note under `reference/notes/` rather than pasting source material.

At the end report concisely:

1. what now actually works;
2. important architecture introduced/changed;
3. validation commands and results;
4. defects recorded;
5. measured performance information, if any;
6. reference sources consulted for behavior-sensitive work;
7. the next logical vertical slice.

Do the implementation autonomously. Do not ask for confirmation on routine engineering choices.
