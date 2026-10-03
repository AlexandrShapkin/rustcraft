# Tooling

`just` is the repository command surface. It is intentionally a command runner, not a build system;
Cargo remains the Rust build system.

Run `just doctor` to see what is installed. Optional tools must improve a real workflow; Codex
should not fail merely because an optional utility is absent.

## Repository command surface

Common recipes:

- `just version` — print the authoritative product version;
- `just release-build` / `just release-check` — stage and validate local client/server archives;

The release helpers use Python 3's standard library for archive staging and inspection. Python is
optional for the ordinary Rust bootstrap but required for release commands.
- `just doctor` — inspect the local toolchain;
- `just bootstrap-check` — baseline repo validation;
- `just ci` — formatting/check/tests/clippy;
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
