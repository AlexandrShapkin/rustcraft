# R2 autonomous acceptance

Implementation is locally accepted; public Ubuntu/Windows CI is the publication gate.

## Recovery

Fresh writable clone: `/tmp/rustcraft-r2-recovered`, based on public
`e82d42d2a302c9f373b71fbe379ed28cd8877286`. No original interrupted R2 worktree was found.
The old read-only checkout and external recovery backup remain intact.

The canonical user ROADMAP was the only file recovered from the old filesystem.
Original and restored SHA256 both were
`9e4fc9dfa21ea9e3e5c83af960278374a753991b92479e11c57224798fe2cf78`
before R2 closeout. Direct filesystem comparison excluded older published-code omissions,
manual drift and untracked files already published or older than the public baseline.
No uncertain Rust files, generated captures, caches or proprietary assets were copied.

Baseline persistence blockers were fixed independently in
`a3a1b4dcc136bafe558cc49729e2662352e76260`.
See [durable transfer repair](ENTITY_TRANSFER_REPAIR.md) for protocols, component compatibility,
and disk-backed interruption matrix. No accidental persistence hunks were reverted.

## Implementation and evidence

Historical crops now live exclusively in the Minecraft importer. Inventory, hotbar, selector and
six player-preview parts resolve semantic regions. Renderer geometry uses whole-resource local UVs
and binds each resolved page in painter order. Existing GUI dimensions, slots and input policy remain.
The preview lifecycle also preserves geometry through background rebuilding and clears it on closure.
ARCH-002 is narrowed to the existing destination-layout/game-policy boundary.

Project-authored coordinate-pattern fixtures prove identical semantic pixels from historical sheets,
rearranged sheets and individual discovered files. Selector package override and warm/content-invalidated
cache checks pass. Explicit offscreen acceptance compares complete hotbar and inventory images across
legacy, rearranged, direct-file and nine-page layouts, with visible selector/head probes. Eight captures
passed on llvmpipe OpenGL; captures remain ignored build artifacts. Existing M3 HUD/control tests pass.
`just sample-game` passes its independent non-Minecraft dependency guard and rendered sandbox proof.
No proprietary historical pixels are required by tests or added to tracked/release assets.

Sequential local acceptance, two Cargo jobs with debug information disabled:

- focused persistence, resource, HUD and semantic fixture tests: passed;
- explicit offscreen semantic layout/page equivalence: passed;
- workspace format, all-target check, workspace tests and strict all-feature Clippy: passed;
- `just ci`: 323 tests passed, two explicit acceptance tests skipped, strict Clippy passed;
- `cargo machete`: no unused dependencies;
- `cargo audit`: no vulnerabilities; existing unmaintained paste, smartstring and ttf-parser warnings;
- release `publication-check`: passed licensing and asset-history policy.

No storage backend, generic checkpoint envelope, entity codec or frozen world generator changed.
The Minecraft receipt component necessarily writes v2 quantity/revision information and reads legacy v1.
No F1/A1/C2/BG1/DX2/RF1/READY1/M5 implementation was started.
