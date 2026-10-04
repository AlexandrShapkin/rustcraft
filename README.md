# RustCraft

RustCraft is an extensible, performance-oriented voxel engine/runtime written in Rust. Minecraft
Beta 1.7.3 is provided as a first-party game package and reference implementation; it is not the
identity or compatibility target of the engine.

The repository is structured as a platform plus first-party gameplay modules. Human players, bots,
replays and tests share a semantic Agent/Controller boundary. Multiplayer servers define a content
profile so clients and bots can automatically resolve the resources/gameplay packages they need.

The product is pre-1.0 and under active development. The default client/server composition currently
embeds `minecraft-b173`; `sandbox-test` proves that the generic engine, Game API, world, renderer and
input path work without that package. See [the release notes](CHANGELOG.md) and
[release guide](docs/RELEASE.md).

## Start

```bash
just doctor
just refs-fetch          # optional but recommended for Beta/reference-heavy work
just refs-status
just bootstrap-check
just version
```

Put local proprietary/reference material in the documented ignored paths:

```text
reference/minecraft-beta-1.7.3-src/
reference/assets/vanilla-b1.7.3/
# or reference/assets/vanilla-b1.7.3.zip
```

These assets are not distributed with RustCraft. To run the first-party Minecraft profile, supply
a compatible local `terrain.png` with `RUSTCRAFT_TERRAIN_TEXTURE=/path/to/terrain.png`.

Public third-party references are declared in `reference/sources.json` and cloned into the ignored
`reference/external/` tree by `just refs-fetch`.

For release/version/tag and platform archive details, see [the release guide](docs/RELEASE.md).

RustCraft source is available under either MIT or Apache-2.0, at your option; see
[LICENSE](LICENSE), [LICENSE-MIT](LICENSE-MIT), and [LICENSE-APACHE](LICENSE-APACHE). This does
not license Minecraft trademarks or proprietary game assets, third-party references, or user-supplied
assets. RustCraft is not affiliated with Mojang or Microsoft.

After the owner completes the GitHub repository rename, the canonical clone command is:

```sh
git clone https://github.com/AlexandrShapkin/rustcraft.git
```

## Main entry points

- `AGENTS.md` — compact repository instructions for Codex;
- `docs/PRODUCT.md` — product definition/non-goals;
- `docs/ARCHITECTURE.md` — engine/gameplay/platform boundaries;
- `docs/AGENTS_AND_BOTS.md` — native Agent/Controller/Bot API direction;
- `docs/CONTENT_SYSTEM.md` — server-defined content and automatic resolution;
- `docs/PERFORMANCE.md` — evidence-driven optimization rules;
- `docs/ROADMAP.md` — implementation sequence;
- `reference/SOURCES.md` — how to use the Beta/reference ecosystem;
- `.agents/skills/` — task-specific Codex workflows;
- `justfile` — stable project command surface.

## Reference sources

The bootstrap declares six public research sources: `mc173`, `BetrockPlusPlus`, reconstructed
`mc_b1.7.3_release`, `beta-wiki`, `nostalgia` mappings and `LibreProg` open assets. They are not
vendored into this archive. See `reference/SOURCES.md` and `docs/REFERENCE_POLICY.md`.

## Important product constraints

- familiar Beta-like voxel sandbox gameplay and visual character;
- no requirement for Java/network/save/mod compatibility;
- no visual-modernization mandate such as ray tracing or realistic fluids;
- performance improvements may redesign algorithms and data representation;
- first-party gameplay modules stay native Rust in hot paths;
- downloadable third-party executable content is sandboxed (planned WASM), never arbitrary native
  shared libraries;
- bots are native platform participants rather than simulated graphical clients;
- connecting clients/bots automatically resolve the server content they require;
- optimization claims must be measured.

Developer tools use F4 for discovery, held F3+1..9 for pages, `/` for commands and Backquote for
Rhai. Unicode text ships with bundled open fonts; no system-font installation is required.
See [debugging](docs/DEBUGGING.md) and [UX1 text/licenses](docs/UX1_REPORT.md).
