# Beta 1.7.3 reference policy

The project intentionally maintains a research perimeter under `reference/`. Reference material is
used to understand familiar behavior, formats, constants and visual conventions; it does not define
production architecture.

Read `reference/SOURCES.md` for the source map and use `just refs-status` to see what is locally
available.

## Source classes

Historical/reconstructed material is best for questions such as "what did Beta 1.7.3 actually do?".
Independent reimplementations are useful for cross-checking interpretation and seeing alternative
implementations. Technical wikis/mappings are navigation aids. Vanilla assets are a visual baseline.
Libre/open assets are preferred for redistributable tests/examples.

The product contract still wins: this project is not bug-for-bug compatible and may deliberately
replace historical behavior with cleaner explicit mechanics while retaining familiar gameplay.

## Rules

Use reference material narrowly to answer concrete questions such as movement behavior, block
interactions, crafting rules, protocol details, region/NBT conventions or texture-atlas behavior.
Then write original idiomatic Rust for this architecture.

Do not:

- mechanically translate Java/C++/Rust reference implementations class/module by class/module;
- copy substantial source fragments into production code;
- inherit another project's architecture merely because it already implements Beta behavior;
- make the Java class hierarchy/API structure the engine architecture;
- copy AGPL reference implementation code into this project's source;
- commit local proprietary game assets/reference source by default;
- package Mojang-owned vanilla assets into bootstrap archives/releases by default.

When a durable finding matters, record the question, sources consulted, conclusion and deliberate
project divergence (if any) under `reference/notes/`.

## Local paths

Optional decompiled/reference source:

`reference/minecraft-beta-1.7.3-src/`

Optional vanilla/default texture pack:

`reference/assets/vanilla-b1.7.3/`

or:

`reference/assets/vanilla-b1.7.3.zip`

Public reference clones live under `reference/external/` and are managed by the `just refs-*`
recipes. They are git-ignored rather than vendored.
