# RustCraft B1.7.3 — bootstrap repository

A performance-first Rust voxel sandbox whose default first-party gameplay is intentionally familiar
to Minecraft Beta 1.7.3 players without requiring bug-for-bug or tick-for-tick compatibility.

The repository is structured as a platform plus first-party gameplay modules. Human players, bots,
replays and tests share a semantic Agent/Controller boundary. Multiplayer servers define a content
profile so clients and bots can automatically resolve the resources/gameplay packages they need.

This bootstrap is intentionally small: it establishes contracts, crate boundaries, Codex workflow,
reference-source handling and the first headless implementation target rather than pretending the
whole game already exists.

## Start

```bash
just doctor
just refs-fetch          # optional but recommended for Beta/reference-heavy work
just refs-status
just bootstrap-check
```

Put local proprietary/reference material in the documented ignored paths:

```text
reference/minecraft-beta-1.7.3-src/
reference/assets/vanilla-b1.7.3/
# or reference/assets/vanilla-b1.7.3.zip
```

Public third-party references are declared in `reference/sources.json` and cloned into the ignored
`reference/external/` tree by `just refs-fetch`.

Then start Codex from the repository root and give it `prompts/00-bootstrap.md` (or simply instruct
it to read `AGENTS.md` and implement M0).

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
