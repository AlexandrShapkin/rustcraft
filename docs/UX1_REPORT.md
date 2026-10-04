# UX1 — Developer controls and Unicode text

Status: **ACTIVE; local acceptance passed, public implementation CI pending**. Initial public main:
`a3554807a20dc4fff6d9878cb018659c23b1a7b2`.
Continuation/public UX1 starting baseline and C1 closeout:
`585de94b666257ddb5e4f2a902429db84be010ca`, Ubuntu and Windows green in
[CI 37182596219](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37182596219).
DUX1/C1 remain CLOSED. The original dirty mounted checkout is preserved; publication uses its
clean writable clone. No version/tag/release or RSM1/later-stage work.

## Input and console

A held F3 state tracks pressed/used, acts on release for bare Overview, consumes digits including
repeats and resolves semantic metadata to pages. 1 Streaming, 2 World, 3 Entities, 4 Lighting,
5 Meshing, 6 Renderer, 7 Persistence, 8 Scripts, 9 Settings. F4 selector remains. Focus loss,
Escape, console/selector/inventory focus clear held state. Normal digits still feed semantic hotbar
intent. Graphical scenario actions exercise the same native routing function used by winit.

Slash opens a prefilled COMMAND prompt; Backquote opens empty RHAI. Compact hints explain leading
slash, REPL, help, completion, history and closing. Existing registry supplies contextual help,
usage/capability/aliases and bounded command/subcommand/setting-key completion. Errors retain the
console and suggest help/usage. No capability expansion or parser bypass.

unicode-segmentation extended grapheme boundaries drive editing; insertion/deletion normalize
boundaries after clusters join. IME commit/preedit uses winit; focus enables/disables IME. Printable
text uses event.text, not physical key translation. Separate shaped caret supports fallback/bidi
cluster positions. Real desktop IME sessions are not exercised by platform CI; state paths are tested.

## Fonts, ownership and coverage

Generic `content::fonts::FontResources` resolves semantic `rustcraft:font/ui`, `/debug`, `/mono`
roles (same stack initially), ordered package overrides and provider ownership. RendererResources
carries resolved resources on profile load. Font-family replacement is startup-only; no OS install,
file-path string in UI or hot cache replacement. Sandbox independently renders non-ASCII semantic
text, and a package override test changes rendered pixels without renderer changes.

cosmic-text 0.19 (MIT OR Apache-2.0), default features disabled; std/swash enabled. Its fontdb,
HarfRust, unicode-bidi and swash provide fallback/shaping/bidi/rasterization. Unicode segmentation
and unicode-script are MIT/Apache-2.0; ttf-parser uses the same licenses. Glyphon 0.9/0.10 require
wgpu25/28, so UX1 keeps the existing wgpu26 and uses CPU rasterization with the generic HUD GPU path.
FontSystem starts from Database::new, fixed en-US locale and an explicit fallback list. It never
calls load_system_fonts or FontSystem::new. Tests assert exactly seven supplied faces.

Primary: unmodified Fusion Pixel 12px Mono latin, upstream 2026.09.25; pixel geometry at integer
12px and snapped positions, nearest composite sampling. Noto fallbacks prioritize coverage.
All fonts use SIL OFL1.1; licenses/copyright/RFN notices are next to each file. Fusion constituent
Ark Pixel/Cubic11/Galmuri notices also ship. No outlines or reserved font names were modified.
Source, pinned commits and licenses: [bundled notices](../crates/content/fonts/README.md).
Candidate cairopixel.zip was not present in the available workspace; selection was evidence-based.

| Face | Bytes | Glyphs | Unicode cmap entries |
|---|---:|---:|---:|
| Fusion Pixel | 4,928,652 | 36,385 | 36,980 |
| Noto Sans | 2,049,096 | 4,515 | 3,094 |
| Noto Sans Math | 1,015,396 | 5,130 | 3,033 |
| Noto Sans Arabic | 844,676 | 1,711 | 1,561 |
| Noto Sans Hebrew | 112,640 | 470 | 464 |
| Noto Sans Devanagari | 641,944 | 1,117 | 551 |
| Noto Emoji | 1,982,596 | 1,891 | 1,489 |

Total font bytes 11,575,000 (~11.04 MiB), embedded in normal/release binaries. This bounded size
buys broad self-contained coverage; release staging also includes resources/licenses. cmap counts
include mappings, not a claim of unique covered Unicode space or every Unicode glyph.
Required Latin/extended Latin/Cyrillic/Ukrainian/Greek samples and debug math symbols have cmap
coverage. Arabic/Hebrew/Devanagari/Chinese/Japanese/Korean/emoji samples use bundled faces. Missing
unassigned/private codepoints use the chosen face's deterministic .notdef/tofu, never ASCII '?'.
This is valid UTF-8 handling with bundled Unicode coverage, not universal glyph coverage.

## Text/cache/scale

Normal HUD counts, DUX/debug pages, Settings and console all emit generic shaped text runs.
The hand-written 5×7 table and uppercase conversion are removed. Specialist offscreen labels use
this same shaper/rasterizer converted to their existing geometry path, not a parallel font system.

CPU glyph cache: at most 2,048 images and 16 MiB payload, generation reset/replacement at caps;
no old-generation retention. After 64 KiB of changed input, the font/shaping/support database
caches are rebuilt too, including unsupported-glyph workloads; bundled face data remains shared. Current layout/input is bounded to 64 runs/16 KiB. One fixed 2048²
RGBA8 GPU composite page (16 MiB) and six vertices; current CPU surface <=16 MiB. Resize updates
clipping/UVs without creating another GPU page. Text beyond capacity is clipped. Same text/size/
caret avoids reshaping/rasterization/upload. Metrics include glyphs, CPU/surface bytes, fixed GPU
capacity/pages, rebuilds, layout/raster times and missing glyphs. This is text lifetime only, not RSM1.

C1 `rustcraft:ui/font_scale`, Float 0.5..3 default1, NextFrame, graphical availability, persist
allowed; Control/Rhai/Settings/native readback share it. Effective pixel size rounds to integers;
minimum/maximum/invalid/reset and deferred publication tests protect the old effective value.

## Acceptance and limitations

Focused input/editor/no-system-font/coverage/replacement/cache tests pass. Optimized soak
processed 8,192 distinct bundled CJK codepoints twice plus six changed 13,000-byte inputs:
eight cache rebuilds, final one glyph/30 bytes, with every update asserting the 2,048/16 MiB
bounds. The fixed GPU composite page remains one/16 MiB through captures and resize.

Final local validation: fmt, workspace/all-target check, workspace tests, strict all-target/all-feature
Clippy, `just ci` (285 passed, one optional skipped), dx-test, dx-console, dx-overhead, sample-game,
ux-test, C1 headless, C1 graphical and DUX1 graphical regression passed. Frozen M4 hashes were not
changed. World/persistence mechanics were not changed; long historical world campaigns were not
repeated. A loaded concurrent run hit an existing Rhai wall-clock limit; the final quiet bounded
four-thread CI run passed without changing script limits.

Real native Wayland/OpenGL surface uses llvmpipe LLVM23.1.1; owner AMD was inaccessible. This is
correctness evidence, not representative GPU performance. Earlier full regression receipts:
UX1 `target/test-runs/scenario/12-1791098141383117398/result.json`, DUX1
`69-1791098152454235080`, C1 `126-1791098187289388618` (same scenario directory).
Captures under `target/captures/ux1_*` cover Overview, held-chord Streaming, selector, command
entry/help/completion/error, Rhai result 42, multilingual fallback and Settings scale. Generated
captures/worlds are ignored and not committed. Actual resized surface was 1280×662. Screenshot
review found enlarged text needed a matching developer-panel background; sizing now invalidates
with scale and is covered by the HUD cache test (56 passed, one optional ignored) and
post-fix graphical receipt `target/test-runs/scenario/472-1791098904599135608/result.json` (pass).
All seven required capture variants were inspected; enlarged text stays on its background and
default pixel text remains crisp. Post-fix Unicode capture reports 167 glyphs/8,624 CPU glyph
bytes, one replacement, one fixed GPU page and CPU enqueue 2,775 µs; scale capture reports
239 glyphs/14,349 CPU glyph bytes, no replacement and enqueue 588 µs. These enqueue times are
CPU observations from software-surface execution. The authoritative scale key is
`rustcraft:ui/font_scale` (shared HUD/developer text), not a separate dev-only setting.

Optimized CPU text update evidence (single diagnostic samples, not universal thresholds):

| Case | Changed update µs | Unchanged µs | Layout µs | Raster µs |
|---|---:|---:|---:|---:|
| No text | 204 | 2.79 | 184 | 2 |
| Static page | 1,607 | 3.28 | 104 | 349 |
| Idle console | 172 | 2.37 | 39 | 110 |
| Typing console | 149 | 2.44 | 39 | 88 |
| First multilingual | 5,316 | 7.19 | 1,646 | 2,379 |
| Warm multilingual change | 2,089 | 3.35 | 320 | 662 |

Upload metrics measure CPU queue enqueue, never physical display scanout or GPU execution.
DX service overhead, including mean/p50/p95/p99/max and provider counts, is recorded in
[PERFORMANCE](PERFORMANCE.md). Disabled/Overview collect zero heavy domains; one page collects
only its domain, so DUX1 demand suppression remains intact.

Bundled-font-only tests assert seven explicit database faces and sample coverage; fontconfig is
absent from the dependency feature tree. Server normal dependency tree contains no render/wgpu/
winit. Independent sandbox renders semantic Ukrainian/Greek text with zero replacements.
Package override changes pixels for the same role. Release binary graphical acceptance uses
embedded fonts, and temporary release staging contains all seven files and required notices.
`cargo machete` found no unused dependencies. `cargo audit` reports no known vulnerabilities;
existing unmaintained warnings remain paste RUSTSEC-2024-0436, smartstring RUSTSEC-2026-0249 and
ttf-parser RUSTSEC-2026-0192. No mechanical dependency updates were made.

Known limits: no complete Unicode glyph guarantee, no full rich-text/editor/chat UI, no selection/
clipboard or real per-platform IME automation, no hot font-resource reload. RTL shaping/bidi is
library-provided; console logical grapheme navigation is not a full visual bidi editor. Large
views/long lines clip at the bounded viewport; not an unlimited scrollable layout. Further broad
GUI, R2 texture normalization, P1 timing and RSM1 residency work remain separate and inactive.

Final quiet post-panel regressions: console `14-1791134245341815073`, DUX1
`150-1791134297162608896`, and C1 207-1791134321682530234 all pass.
Final fmt/check/workspace-test/strict-Clippy and all required just workflows pass.
Diagnostic overhead counts remain zero for inactive domains; the final high-page run records
a 59.66 ms outlier in PERFORMANCE.md rather than discarding it.
