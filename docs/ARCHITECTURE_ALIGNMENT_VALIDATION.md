# Architecture alignment validation

This bounded pass establishes the engine/Game API/game-package direction without starting M4 or
rewriting M3. The owner subsequently completed separate M3 manual acceptance.

## Dependency evidence

- `rustcraft-engine-core` has no normal Cargo dependencies.
- `rustcraft-game-api` has normal dependencies only on `rustcraft-content` and
  `rustcraft-engine-core`.
- `rustcraft-render` has no dependency on Minecraft, legacy gameplay crates or `runtime`.
- `rustcraft-sandbox-test` has no dependency on `rustcraft-minecraft-b173`,
  `rustcraft-gameplay-blocks`, `rustcraft-gameplay-flat-world` or `rustcraft-runtime`.
- client and server import first-party definitions through `rustcraft-minecraft-b173`, not the
  two legacy gameplay crates directly.

`just sample-game` enforces its dependency condition before execution. The run registered four
`sandbox_test:` blocks and two native systems, converted a generic primary action into a queued
`SetBlock`, applied it at the command boundary and rendered the result with the shared offscreen
renderer to `target/sample-game/sandbox-test.png`.

## Validation results

- `just bootstrap-check`: pass.
- `just ci`: pass, including strict Clippy with `-D warnings`.
- `just smoke`: pass; semantic Bot break/place scenario remained intact.
- `just survival-scenario`: pass; drop pickup and log-to-planks crafting remained intact.
- `just sample-game`: pass on llvmpipe/OpenGL offscreen rendering.
- `just render-test-all`: pass; deterministic M3 images regenerated.
- `just fidelity-m3`: pass; deterministic M3 fidelity suite regenerated.
- `timeout 5s just client-diag`: client initialized the triangle diagnostic on AMD RADV/Vulkan,
  loaded local GUI/player resources and reported three vertices/three indices; timeout exit 124 is
  the intentional stop for the interactive loop.

The full workspace check/test suite contains 77 unit tests. Normal CPU tests do not require a GPU.
Mesa's failed Zink probe is followed by successful llvmpipe adapter selection and is not a test
failure.

## Remaining migration boundary

The legacy M0-M3 `runtime` and `mod-api` still mix reusable mechanisms with Minecraft policy. Beta
UI construction also remains beside generic HUD rendering. These are recorded in
`ARCHITECTURE_AUDIT.md` and `DEFECTS.md`; future work must migrate the touched slice before adding
policy, rather than attempting a risky all-at-once rewrite.
