---
name: reference-study
description: Study reference implementations, specifications and assets before implementing player-observable Beta-like RustCraft behavior, extracting semantic invariants without porting reference architecture.
---

# Feature reference study

Use this skill before implementing any player-observable Beta-like behavior: controls, gameplay,
inventory, crafting, item entities, HUD/UI, block visuals, mining, entities, world generation,
weather, audio or animation. It is unnecessary for purely internal storage, CI, serialization,
transport, logging or profiler work.

1. Read `reference/SOURCES.md` and `docs/REFERENCE_POLICY.md`.
2. Run `just refs-status`.
3. Define the concrete question before opening a reference tree.
4. Choose the narrowest useful source class:
   - historical/reconstructed source for original observed behavior/constants;
   - `mc173` or `BetrockPlusPlus` to cross-check independent interpretations;
   - `beta-wiki` for protocol/spec lookup;
   - `nostalgia` for legacy-name/mapping help;
   - vanilla texture pack for visual baseline;
   - `LibreProg` for redistributable/open asset examples.
5. Cross-check important ambiguous behavior rather than trusting one implementation blindly.
6. Extract semantic invariants and implementation-independent expectations; identify details safe
   to change and useful optimization opportunities. Do not port the reference architecture.
7. Choose original idiomatic Rust behavior using the current project priority order. Exact
   reproduction is optional when a meaningful measured/justified benefit supports a deviation.
8. Before production edits, record `reference/notes/features/<feature>.md` with sources and locked
   revisions, original classes/methods/assets, reference behavior, semantic invariants,
   implementation-independent expectations, details safe to change, optimization opportunities,
   chosen RustCraft behavior, intentional deviations and acceptance criteria.
9. If the conclusion is durable or affects future work, record it in project documentation too.

Never copy substantial third-party/reference code into production source. Never automatically
execute/build downloaded reference repositories merely to inspect them.
