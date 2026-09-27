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
