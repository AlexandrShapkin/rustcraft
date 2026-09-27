# Reference study

Use this skill when implementation depends on Beta 1.7.3 behavior, protocol, legacy file formats,
asset conventions or the interpretation of historical code.

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
6. Extract the behavioral/format conclusion; do not port the reference architecture.
7. Implement original idiomatic Rust that follows this repository's contracts.
8. If the conclusion is durable or affects future work, record a short project-authored note under
   `reference/notes/` with sources consulted and any deliberate divergence.

Never copy substantial third-party/reference code into production source. Never automatically
execute/build downloaded reference repositories merely to inspect them.
