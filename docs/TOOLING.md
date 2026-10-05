# Tooling

`just` is the repository command surface. It is intentionally a command runner, not a build system;
Cargo remains the Rust build system.

Run `just doctor` to see what is installed. Optional tools must improve a real workflow; Codex
should not fail merely because an optional utility is absent.

## Repository command surface

Common recipes:

- `just version` — print the authoritative product version;
- `just release-build` / `just release-check` — stage and validate local client/server archives;

Documentation and release helpers use Python standard library only. Python 3.11+ (tomllib) is
required for normal documentation/CI validation; direct Cargo builds remain independent of Python.
- `just doctor` — inspect the local toolchain;
- `just bootstrap-check` — baseline repo validation;
- `just ci` — offline docs tests/check, then formatting/check/tests/clippy;
- `just smoke` — headless vertical-slice smoke scenario;
- `just refs-status` — inspect local reference sources/revisions;
- `just refs-fetch` — fetch declared public references;
- `just refs-update` — fast-forward clean reference clones;
- `just refs-lock` — record exact reference commits.

When a new repeatable project workflow appears, prefer a `just` recipe over teaching Codex a long
one-off command sequence in prompts.

## Baseline

Expected:

- `git`;
- Python 3.11+ (`python3`; Windows CI uses `python`);
- Rust toolchain (`rustc`, `cargo`, `rustfmt`, `clippy`);
- `just`;
- `rg`;
- `jq`.

Already useful on the owner's system and safe for Codex to use when available:

- `fd` for file discovery;
- `xh` for HTTP diagnostics;
- `yq` (go-yq) for structured-data shell automation where suitable;
- `delta` for readable diffs;
- `doggo` for DNS diagnostics;
- `websocat` for socket/WebSocket experiments;
- `wl-copy`/`wl-paste` for local clipboard workflows;
- `hyperfine` for command-level benchmarks.

Use ordinary tools (`rg`, `git grep`, `jq`, `find`, shell) when they are the shortest reliable path.
Do not reach for an MCP just to prove it exists.

## High-value Rust additions (optional)

Adopt when useful, not as mandatory bootstrap dependencies:

- `cargo-nextest`: faster/cleaner test execution on larger workspaces;
- `cargo-audit`: RustSec vulnerability checks;
- `cargo-deny`: license/source/advisory policy once the dependency graph becomes real;
- `cargo-machete`: catch unused dependencies;
- `cargo-semver-checks`: protect stable Mod/Bot/public SDK APIs once releases exist;
- `cargo-llvm-cov`: coverage investigations;
- `cargo-bloat`: binary-size attribution;
- `samply` or `cargo-flamegraph`/`perf`: CPU profiling;
- `cargo-fuzz`: parser/protocol/content fuzzing when those attack surfaces exist;
- `cargo-mutants`: targeted mutation testing for critical pure logic when normal tests are mature;
- `sccache`: build-cache option if compile time becomes material;
- `mold`/`lld`: linker experiments if linking becomes a bottleneck;
- `wasm-tools` / a Wasmtime CLI: inspect and validate WASM when the third-party mod runtime becomes
  real.

Do not install all optional tools preemptively. Add a tool when its workflow becomes relevant and
then expose the common invocation through `just`.

## Reference tooling

Public research repositories are declared in `reference/sources.json`. `scripts/references.sh`
uses only baseline `git` + `jq` and intentionally does not build or execute third-party reference
projects.

The reference lock file is informational/reproducibility metadata. `just refs-lock` records the
exact commits currently being consulted so a research conclusion can later be reproduced.

## MCPs / Codex

Keep machine-specific MCP launch configuration at user scope unless the repo genuinely needs a
portable project-local server.

When available:

- Serena: symbol-aware navigation/refactoring and repository understanding;
- Context7: current third-party library/API documentation.

Do not hard-code credentials in `.codex/config.toml` or the repository.

## Offscreen block presentation inspector (M3.8)

`just render-test SCENE [options]` writes deterministic PNGs and scene manifests beneath
ignored `target/render-tests/`, overwriting that scene's previous generated output. It needs
a wgpu adapter (software GL/llvmpipe works), but no visible window or readable window surface.
A missing GPU/asset fails explicitly; normal CPU tests never require either.

Examples:

```sh
just render-test face-neg-z --mode corners
just render-test face-neg-z --mode uv --wireframe
just render-test dropped-bookshelf --rear
just render-test gui-bookshelf --native
just render-test cube-log --axis x
just render-test cube-bookshelf --facing east --base-rotation 90
just render-test gui-grid
just render-test-all
just fidelity-m3
```

Faces: `face-pos-x`, `face-neg-x`, `face-pos-y`, `face-neg-y`, `face-pos-z`, `face-neg-z`.
Representations: `cube-NAME`, `dropped-NAME`, `gui-NAME`, `inventory-NAME`, `hotbar-NAME`,
`cursor-NAME`; NAME is a first-party semantic name such as bookshelf/log/grass. The last three
are isolated item representations, not screenshots of an entire interactive screen.
Modes: `solid`, `corners`, generated `uv`, local-resource `atlas` (default for blocks).
`--wireframe` adds triangle edges, vertex marks, normals and face labels; `--no-cull` is
explicitly diagnostic. `--rear` exposes the opposite sides; `--native` uses a 16-pixel GUI
region rather than the enlarged inspection view. `--item-age TICKS` and `--count N` select
deterministic dropped-stack presentation. `--facing north|east|south|west|up|down`,
`--axis x|y|z`, `--rotation 0|90|180|270` and `--base-rotation` exercise the orientation
foundation even on currently nondirectional first-party blocks; these do not modify gameplay
content. Combined diagnostic controls compose base*facing*axis*horizontal.

`just render-test-all` and `just fidelity-m3` run the complete offscreen suite, including rear
and native views. The older CPU-only recipe is now `just render-unit-test`. The reusable
`inspection::BlockModel::resolve(BlockState, resolver)` API exposes content/state selection
separately from GPU rendering. No texture-pack editor or golden-image acceptance is implied.

## Non-Minecraft architecture integration game

`just sample-game` builds and runs the `sandbox-test` profile. The recipe first checks its normal
Cargo dependency tree and fails if it contains `minecraft-b173`, either legacy gameplay crate, or
the mixed M0-M3 runtime. The game registers four `sandbox_test:` voxel definitions through
`game-api`, consumes generic controller intent, queues a `SetBlock` command, and writes a generated
offscreen image to `target/sample-game/sandbox-test.png`. It uses no proprietary resources and is
an architecture test rather than a second product.

## DX1 developer commands

`just dev-client`, `just script-check [PATH]`, `just scenario-headless [PATH]`,
`just scenario-client [PATH]`, `just dx-smoke`, `just script-bench`.
Both scenario recipes default to scripts/scenarios/dx_smoke.rhai. See [DEBUGGING.md](DEBUGGING.md).

DX1 closure diagnostics: `just dx-test`, `just dx-console`, `just dx-overhead`.
Overhead writes ignored target/dx-overhead.json. `dx_responsive.rhai` plus the explicit
`--dx-abort-after-frames 100` client flag proves cooperative job/capture/abort with frame progress.

## Planned pre-M5 workflow consolidation

[PRE_M5_AUDIT.md](PRE_M5_AUDIT.md) classifies current recipes and test tiers. DX2 inventories
callers/assertions/artifacts and proves scenario equivalence before retirement. No recipe is dead
merely because DX1 exists; preserve frozen M4 hashes and specialist correctness gates. DUX1 adds
discovery over existing diagnostics; C1 shares effective settings across Control/Rhai/in-game tools.
Current execution state is in the [registry](stages.toml). DUX1 adds `just dux-test` (shared and client focus/cache tests) and `just dux-client` (release real-surface acceptance with an isolated fixture world). See [DUX1_REPORT.md](DUX1_REPORT.md). No specialist recipe is retired.

## C1 workflows

`just config-test` runs registry/control/native application tests; `just config-smoke` runs headless acceptance. Client/server `--config-report` gives non-graphical readback; `--set-config KEY=VALUE` and `--config-file PATH` select startup sources. `rustcraft-client --c1-acceptance` exercises real Settings/boundaries/captures in a disposable world. Existing DX/DUX and specialist recipes remain. See [CONFIGURATION.md](CONFIGURATION.md) and [C1_REPORT.md](C1_REPORT.md).

## UX1 acceptance

`just ux-test` runs focused input/editor/font/resource/cache checks. `just ux-client` runs the
real disposable graphical acceptance (F3/hotbar, F4, slash/Backquote console, multilingual text,
scale/reset/captures). Existing dx/dux/C1 workflows remain. Fonts are embedded; development and
release runs do not look up repository font paths or system fonts. Release staging includes
FONT_RESOURCES with all upstream font license texts. See [UX1_REPORT.md](UX1_REPORT.md).

## Residency lifetime acceptance

- `just rsm1-test`: focused meshing/runtime/shared-ledger tests, including pressure and late results.
- `just rsm1-client`: explicit developer graphical campaign with disposable state and C1 workload
  overrides; 5 bounded route cycles, unique exploration, return, final drain and capture. Requires
  the normal graphical resource/backend setup. Scalar evidence is ignored `target/rsm1/*/samples.json`;
  captures use the existing DX capture workflow. No generated evidence is a repository resource.

These complement `world-stream-bench`, `world-travel-test` and `render-camera-motion`, whose gates
remain intact. They are correctness/acceptance workflows; any DX2 retirement requires equivalent
coverage. See [RSM1_REPORT.md](RSM1_REPORT.md).

P1 workflows: `just p1-test` checks bounded clocks/visual state and shared diagnostics;
`just p1-client` runs six matched 20-second phases on a real 640x360 surface in a disposable world,
then exact pause/step/resume/teleport and capture acceptance. Set existing C1 overrides explicitly
if changing workloads; compare identical settings. Bounded results live under ignored `target/p1/`,
captures under `target/captures/`. This is a milestone acceptance alias for later DX2 review, not a
physical input-to-photon/scanout profiler. `dx-overhead` includes Presentation active/inactive modes.

S1 workflows: `just s1-test` runs bounded physical-publication, unknown-component, transfer/reopen
and isolated split-candidate correctness. `just s1-bench 10000 100 OUTPUT_DIRECTORY` runs release
current/split comparisons sequentially plus auxiliary delete/recovery/shutdown/accounting probes.
Benchmark directories are disposable and outputs belong under ignored `target/s1/`; never commit
stores, temporary files or operation logs. A 50k/one-cycle current-only diagnostic is
`cargo run --release -p rustcraft-world --example s1 -- 50000 1 target/s1/large.json`.
Candidate remains examples/dev-dependencies only, with no normal backend switch or new format.
These are S1 acceptance aliases for later DX2 review; existing persistence workflows remain.
See [S1_REPORT.md](S1_REPORT.md) and [S1_PERSISTENCE_DECISION.md](S1_PERSISTENCE_DECISION.md).

## Target DX2 command surface and RF1 navigation

Conceptual long-term core: `just client`, `just server`, `just test`, `just ci`. This is target
workflow, not a claim that all four recipes already exist with final semantics. Current survival,
devtools and specialist acceptance recipes remain until F1 path unification and DX2 proven-equivalent
coverage permit retirement. Scenarios should drive the same client/server composition through Control
and semantic actions; no alternate product variants. DX2 also owns project-authored diagnostic content
through the public Game API; READY1 checks independent engine acceptance without Minecraft assets.

RF1 begins with measured crate/module graphs, largest files/types/functions, highly connected modules,
change hotspots, adapters, cycles (if any) and duplicate policy/mechanism paths. It produces
`docs/CODE_MAP.md`, a concise current ownership/navigation map for mutation, presentation, persistence,
content compilation, control and the later space path. No stale symbol dump or hundreds of tiny files.
See [ARCHITECTURE_AUDIT](ARCHITECTURE_AUDIT.md) for today's source anchors and [ROADMAP](ROADMAP.md)
for execution gates. This docs-only pass does not perform RF1 or create its post-refactor code map.

## Canonical GitHub tracker queries

GitHub Issues is live defect/debt state; [DEFECTS](DEFECTS.md) is policy/history/navigation only.
These tracking commands do not affect offline build/test workflows:

```sh
gh issue list --repo AlexandrShapkin/rustcraft --state open --limit 200
gh issue list --repo AlexandrShapkin/rustcraft --state closed --limit 200
gh issue list --repo AlexandrShapkin/rustcraft --state open --label severity:P1
gh issue list --repo AlexandrShapkin/rustcraft --state open --label stage:F1
gh issue view 18 --repo AlexandrShapkin/rustcraft
```

Search both states before creation. On work/closeout follow [WORKFLOW](WORKFLOW.md), using a file for
multiline bodies/comments. After required acceptance and CI, post the evidence comment before closure:

```sh
gh issue comment 18 --repo AlexandrShapkin/rustcraft --body-file /tmp/issue-resolution.md
gh issue close 18 --repo AlexandrShapkin/rustcraft --reason completed
```

Examples are not authorization to close F1 during another pass. For deliberate no-work conclusions use
`--reason "not planned"` (API state reason not_planned); for duplicates use `--duplicate-of NUMBER`
with a canonical ticket. Do not fabricate completion evidence or close on migration alone.

## Documentation navigation commands

Python 3.11+ is a baseline tooling requirement, checked by `just doctor`. No pip packages are needed.
Public Ubuntu/Windows CI installs Python and runs the same offline tests/check before Rust compilation.

```sh
just docs-check                      # offline, deterministic, mutation-free integrity
just docs-test                       # Python stdlib unittest
just docs-sync                       # only generated index and ROADMAP marker block
just codex-context                   # registry focus; optional GH enrichment with timeout
just codex-context F1 --offline       # compact local pointers, no network
just stage-new C3 "Some new stage" BG1  # insert after BG1; edit scope/navigation afterward
just project-tree                    # current tracked paths from Git, no snapshot file
```

`stage-new ID TITLE AFTER` creates a planned contract/registry entry and synchronizes navigation. It
creates no commit, label or authorization. Manual TOML edits remain supported; run sync/check afterward.
Focus/state helpers are intentionally omitted: edit those few registry fields explicitly, preserving
at most one active stage and its focus. Closing a stage does not remove it from the sequence.

Override the just Python command on systems without `python3`: `just --set python python docs-check`.
Direct portable equivalents are `python scripts/docs.py check` and
`python -m unittest discover -s scripts/tests -p 'test_docs.py'`. Local context remains usable when GH
is absent, unauthenticated or offline; full issue reconciliation still requires current online state.
Source anchors are navigation hints checked for existence/case, refreshed after structural changes.
See [INDEX](INDEX.md) for document ownership and templates, not a full tree dump.

## F1 normal client and accepted hardware evidence

`just client [ARGS…]` is canonical normal human/developer startup. `just client-release [ARGS…]`
uses the same runtime in the release profile. `--survival` selects game-owned player state;
`--devtools` explicitly grants trusted local tooling; `--player` demonstrates denied selector
operations. These arguments are independent. `client-survival` and `dev-client` delegate to `client`;
specialist scenarios/acceptance aliases remain harnesses until DX2 proves retirement equivalence.
Owner normal-speed release play is accepted; use `just client-release` for that play profile.
Both automated profiles measured successfully; no unprovided comparative speedup is claimed.

`just f1-input-window-test` runs deterministic production input-router regression without display/GPU.
`just f1-test` runs client, scripting and storage tests sequentially. `just f1-client [both|dev|release]`
produces isolated AMD/RADV/Vulkan field evidence under ignored `target/f1/`; see
[DEBUGGING](DEBUGGING.md#f1-routing-and-field-acceptance). It preserves normal build
profiles; local quota-constrained test symbol overrides are not owner field measurements.

`just f1-manual` records 45 seconds of owner-controlled normal release gameplay in disposable
F1 state, with the same AMD/RADV/Vulkan gate. `just f1-client` now also retains fast_pan and
walk_fast_pan evidence; existing four-phase evidence remains valid. See DEBUGGING for capture steps.
