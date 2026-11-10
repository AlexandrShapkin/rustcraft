# Reference source map

Reference material exists to answer concrete behavioral, format, protocol and presentation
questions. It is evidence, not project architecture and not a source tree to translate.

## Core sources

### `theorzr/mc173`

Path after `just refs-fetch`: `reference/external/mc173/`

Use for:

- independent Rust interpretations of Beta 1.7.3 world/chunk/entity/block behavior;
- implementation ideas that are idiomatic in Rust;
- server/protocol research;
- identifying behavior worth verifying elsewhere.

Do not treat its architecture as this project's architecture. In particular, this project has
stronger modularity, content-system and Bot API requirements.

### `OfficialPixelBrush/BetrockPlusPlus`

Path: `reference/external/BetrockPlusPlus/`

Use for:

- independent C++ client/server behavior;
- protocol and packet interpretation;
- legacy world/NBT/region handling;
- renderer/resource interpretation;
- performance and portability implementation ideas.

Do not copy AGPL implementation code into this project.

### `jacobo-mc/mc_b1.7.3_release`

Path: `reference/external/mc_b1.7.3_release/`

Use as a historical-behavior source when the question is "what did Beta 1.7.3 actually do?".
Useful for constants, block/item/entity semantics, recipes, timings and presentation conventions.

Treat reconstructed game code/assets as reference-only. Do not mechanically port it and do not
redistribute its game material as part of this project.

## Supplemental sources

### `OfficialPixelBrush/beta-wiki`

Path: `reference/external/beta-wiki/`

Use for protocol and technical-specification lookup. Verify ambiguous claims against an independent
implementation or historical source when the detail matters.

### `McHistory/nostalgia`

Path: `reference/external/nostalgia/`

Use mappings to make decompiled legacy source easier to navigate. Mappings explain names; they do
not define this project's API.

### `OfficialPixelBrush/LibreProg`

Path: `reference/external/LibreProg/`

Use as a redistributable/open visual and audio reference compatible with Beta 1.7.3, especially
when an asset is needed in tests/examples without using Mojang-owned assets.

## Local references

### Decompiled/reference Beta source

Preferred local path:

`reference/minecraft-beta-1.7.3-src/`

Use narrowly for behavioral archaeology. Never base the engine architecture on the Java class
hierarchy.

### Vanilla Beta 1.7.3 texture pack

Preferred local paths:

- `reference/assets/vanilla-b1.7.3/`
- `reference/assets/vanilla-b1.7.3.zip`

These paths are ignored local owner data beneath the discovered source root; assets are not tracked
source and do not require a particular historical checkout. Supply them separately from the source
checkout; never commit proprietary files.

Use as the visual baseline for texture coordinates, atlas layout, animation conventions, alpha
behavior and near-field appearance. Keep it local and untracked.

## Evidence priority

There is no rule saying historical implementation behavior must be reproduced. The product and
`docs/GAMEPLAY_CONTRACT.md` define the target.

When researching a familiar mechanic:

1. identify the observable behavior the project actually wants;
2. inspect the historical/reconstructed source if exact historical behavior helps;
3. cross-check nontrivial details with an independent implementation or technical reference;
4. write original Rust that fits this project's architecture;
5. document any deliberate behavioral divergence that affects players or mod/API contracts.

When sources disagree, investigate the disagreement. Do not choose a source solely because it is
written in Rust or looks cleaner.

## Engineering references reviewed 2026-11-10

These are design observations and suggested validation techniques, not a change
of product direction or a mandate to port another engine. Existing Issues and
ROADMAP.md continue to own actionable work.

- **Luanti:** [engine structure](https://docs.luanti.org/for-engine-devs/structure/)
  separates client/server work and background map generation/mesh work. Application
  to [#17](https://github.com/AlexandrShapkin/rustcraft/issues/17): measure queue wait,
  mesh CPU time and upload/frame time separately before choosing an optimization;
  check generation identity before accepting stale background results. This is an
  engineering inference, not evidence of a current RustCraft defect.
- **Veloren:** [project architecture](https://book.veloren.net/contributors/developers/codebase-structure.html)
  separates headless client/server libraries from frontends. Keep RustCraft's
  existing semantic controllers and Game API independently testable without a
  graphical client. Its [ECS discussion](https://book.veloren.net/contributors/developers/ecs.html)
  motivates measuring batching/cache locality for item scaling in
  [#7](https://github.com/AlexandrShapkin/rustcraft/issues/7), not an ECS rewrite.
- **Horizon:** interpreted here as [horizon-rust/horizon](https://github.com/horizon-rust/horizon)
  because no URL was supplied. Its [contribution guide](https://github.com/horizon-rust/horizon/blob/main/CONTRIBUTING.md)
  uses pinned Java-oracle/Rust-probe comparisons for identical inputs. For
  [#5](https://github.com/AlexandrShapkin/rustcraft/issues/5) and
  [#6](https://github.com/AlexandrShapkin/rustcraft/issues/6), use deterministic
  transaction fixtures, compare semantic outcomes and item conservation, including
  invalid actions. Record reference revision, input seed and expected mechanic.
  Horizon targets a different Minecraft version; its parity goal does not override
  RustCraft's Beta reference or engine-mechanism/game-policy boundary.

Licensing: Luanti source is [LGPL-2.1-or-later](https://github.com/luanti-org/luanti/blob/master/LICENSE.txt),
Veloren is [GPL-3.0-or-later](https://gitlab.com/veloren/veloren/-/blob/master/LICENSE),
and Horizon is [MIT](https://github.com/horizon-rust/horizon/blob/main/LICENSE).
Use architectural lessons only in this change; no source or assets were copied.
A source license does not license third-party game assets or oracle fixtures.
Keep proprietary references and owner-owned assets ignored and backed up separately.
