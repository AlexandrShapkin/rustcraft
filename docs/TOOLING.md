# Tooling

`just` is the repository command surface. It is intentionally a command runner, not a build system;
Cargo remains the Rust build system.

Run `just doctor` to see what is installed. Optional tools must improve a real workflow; Codex
should not fail merely because an optional utility is absent.

## Repository command surface

Common recipes:

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
