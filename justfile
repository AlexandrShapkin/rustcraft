set shell := ["bash", "-euc"]

# Override with --set python python on Windows installations without python3.
python := "python3"

# Show the available project commands.
default:
    @just --list --unsorted

# Report installed baseline/optional developer tools.
doctor:
    ./scripts/doctor.sh

# Show local/reference-source availability and exact revisions.
refs-status:
    ./scripts/references.sh status

# Fetch every enabled public reference repository.
refs-fetch:
    ./scripts/references.sh fetch all

# Fetch only the three core reference repositories.
refs-fetch-core:
    ./scripts/references.sh fetch core

# Fetch supplemental docs/mappings/open assets only.
refs-fetch-supplemental:
    ./scripts/references.sh fetch supplemental

# Fast-forward clean reference clones. Dirty/detached clones are left untouched.
refs-update:
    ./scripts/references.sh update

# Record exact reference revisions in reference/LOCK.json.
refs-lock:
    ./scripts/references.sh lock

# Format Rust sources.
fmt:
    cargo fmt --all

# Verify formatting without changing files.
fmt-check:
    cargo fmt --all --check

# Type-check the whole workspace.
check:
    cargo check --workspace --all-targets

# Run tests; use nextest automatically when it is installed.
test:
    @if cargo nextest --version >/dev/null 2>&1; then \
        cargo nextest run --workspace; \
    else \
        cargo test --workspace; \
    fi

# Force the standard Cargo test runner.
test-std:
    cargo test --workspace

# Run Clippy as a strict batch gate.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Normal local/CI quality gate.
ci: docs-test docs-check fmt-check check test lint

# Minimal repository bootstrap validation.
bootstrap-check: doctor fmt-check check test refs-status

# Run the current headless/server smoke path.
smoke:
    cargo run -p rustcraft-server -- --smoke

# Launch the local graphical client.
client:
    cargo run -p rustcraft-client

# Start the authoritative survival profile with an empty inventory.
client-survival:
    cargo run -p rustcraft-client -- --survival

# Start a renderer benchmark profile; FIFO/vsync may still be selected by the platform.
client-bench:
    RUSTCRAFT_MEASURE_SECONDS=12 cargo run -p rustcraft-client -- --capture /tmp/rustcraft-client-bench.png

# Select one diagnostic stage. Advance only after accepting the previous stage.
client-diag stage="triangle":
    cargo run -p rustcraft-client -- --diag "{{stage}}"

# Capture an actual surface frame to a new PNG, then exit. Never overwrites evidence.
client-diag-capture stage path:
    cargo run -p rustcraft-client -- --diag "{{stage}}" --capture "{{path}}"

# Focused rendering/camera regression checks.
render-unit-test:
    cargo test -p rustcraft-render -p rustcraft-client

# Inspect the workspace dependency graph.
deps:
    cargo tree --workspace

# RustSec audit when cargo-audit is installed.
audit:
    @if cargo audit --version >/dev/null 2>&1; then cargo audit; else echo "cargo-audit not installed (optional)"; fi

# Dependency policy check when cargo-deny is installed and configured.
deny:
    @if cargo deny --version >/dev/null 2>&1; then cargo deny check; else echo "cargo-deny not installed (optional)"; fi

# Unused dependency scan when cargo-machete is installed.
machete:
    @if cargo machete --version >/dev/null 2>&1; then cargo machete; else echo "cargo-machete not installed (optional)"; fi

# Public API compatibility check once stable APIs exist.
semver-check:
    @if cargo semver-checks --version >/dev/null 2>&1; then cargo semver-checks check-release; else echo "cargo-semver-checks not installed (optional)"; fi

# Show workspace metadata in JSON for tools/agents.
metadata:
    cargo metadata --format-version 1

# CPU-only lighting/edit/extraction/meshing workload; no GPU is required.
bench-m2:
    cargo run -p rustcraft-client -- --bench-m2

bench-m3:
    cargo run -p rustcraft-client -- --bench-m3

# R1.2 deterministic CPU extraction/meshing scaling and dirty-remesh workload.
render-scale profile="release":
    cargo run --{{profile}} -p rustcraft-client -- --render-scale

# Real renderer camera/culling and capacity-reuse submission diagnostic.
render-camera-motion:
    cargo run --release -p rustcraft-client -- --camera-motion

# Deterministic offscreen fidelity suite, no surface or visible window.
fidelity-m3:
    cargo test -p rustcraft-render chunk_dropped_and_gui_multi_page_paths_render_every_page -- --ignored
    cargo run -p rustcraft-client -- --fidelity-m3

# Surface-independent PNG inspector. Modes: solid, corners, uv, atlas.
render-test scene *args:
    cargo run -p rustcraft-client -- --render-test {{quote(scene)}} {{args}}

render-test-all:
    cargo test -p rustcraft-render chunk_dropped_and_gui_multi_page_paths_render_every_page -- --ignored
    cargo run -p rustcraft-client -- --render-test-all

survival-scenario:
    cargo run -p rustcraft-server -- --survival

world-roundtrip:
    cargo run -p rustcraft-server -- --world-roundtrip

# M4-003 production-path world clock, spatial entity, eviction/reload and pickup recovery.
world-state-roundtrip:
    cargo run --release -p rustcraft-server -- --world-state-roundtrip

entity-persistence-bench:
    cargo run --release -p rustcraft-server -- --entity-persistence-bench

world-info saves_root world:
    cargo run -p rustcraft-server -- --world-info {{quote(saves_root)}} {{quote(world)}}

inspect-chunk saves_root world x z:
    cargo run -p rustcraft-server -- --inspect-chunk {{quote(saves_root)}} {{quote(world)}} {{quote(x)}} {{quote(z)}}

worldgen-bench:
    cargo run --release -p rustcraft-server -- --worldgen-bench

# Deterministic versioned terrain statistics. Arguments: generator version, seed, chunk radius.
worldgen-report version="2" seed="731173" radius="6":
    cargo run --release -p rustcraft-server -- --worldgen-report {{version}} {{seed}} {{radius}}

# Project-owned text biome/height map beneath ignored target/.
worldgen-map version="2" seed="731173" radius="6":
    cargo run --release -p rustcraft-server -- --worldgen-map {{version}} {{seed}} {{radius}}

worldgen-v1-regression:
    cargo test -p rustcraft-minecraft-b173 worldgen::tests::generator_v1_canonical_region_is_frozen -- --exact

worldgen-v2-test:
    cargo test -p rustcraft-minecraft-b173 worldgen::v2::tests --lib

persistence-bench:
    cargo run --release -p rustcraft-server -- --persistence-bench

world-stream-bench:
    cargo run --release -p rustcraft-server -- --world-stream-bench

# Headless production streaming/controller route: fresh generation, normal-speed travel, edit,
# eviction, disk reload, negative coordinates, distant player reopen, and continued travel.
world-travel-test:
    cargo run --release -p rustcraft-client -- --world-travel-test

# Deterministic production-controller route in an isolated ignored test world.
# Example: RUSTCRAFT_MESH_WORKERS=2 just stream-perf target/stream-perf-w2 stream-perf-w2
stream-perf saves_dir="target/stream-perf-saves" world="stream-perf":
    RUSTCRAFT_SAVES_DIR={{saves_dir}} RUSTCRAFT_WORLD_NAME={{world}} RUSTCRAFT_F3=1 RUSTCRAFT_F3_TRACE=1 RUSTCRAFT_STREAM_PERF_SECONDS=168 RUSTCRAFT_MEASURE_SECONDS=360 cargo run --release -p rustcraft-client -- --survival --stream-perf

# Bounded actual-client M4 acceptance. Uses the selected surface/GPU and exits non-zero when
# terrain margin, input/event responsiveness, route coverage, or visibility latency fails.
# The small correctness window and bounded llvmpipe workers avoid software-GPU overhead;
# both settings are overridable for hardware measurements and leave ordinary play unchanged.
client-stream-auto saves_dir="target/client-stream-auto-saves" world="client-stream-auto":
    RUSTCRAFT_SAVES_DIR={{saves_dir}} RUSTCRAFT_WORLD_NAME={{world}} RUSTCRAFT_F3=1 RUSTCRAFT_F3_TRACE=1 LP_NUM_THREADS=${LP_NUM_THREADS:-2} RUSTCRAFT_STREAM_WINDOW_SIZE=${RUSTCRAFT_STREAM_WINDOW_SIZE:-320x240} RUSTCRAFT_STREAM_PERF_SECONDS=168 RUSTCRAFT_MEASURE_SECONDS=360 cargo run --release -p rustcraft-client -- --survival --stream-perf

# Print the sole authoritative Engine/Product SemVer.
version:
    python3 scripts/release.py version

# Build platform client/server archives and checksum sidecars beneath ignored target/release-dist/.
release-build:
    python3 scripts/release.py build

# Run full workspace CI, rebuild release archives, and validate their structure/executables.
release-check: ci release-build
    python3 scripts/release.py check

# Non-Minecraft Game API integration slice. Fails if its normal dependency graph gains the
# first-party Minecraft package, legacy Minecraft gameplay crates, or M0-M3 policy runtime.
sample-game:
    @deps="$$(cargo tree -p rustcraft-sandbox-test --edges normal --prefix none)"; if rg -q 'rustcraft-(minecraft-b173|gameplay-blocks|gameplay-flat-world|runtime)' <<<"$$deps"; then printf '%s\n' "$$deps" >&2; echo "sample-game dependency boundary violated" >&2; exit 1; fi
    cargo run -p rustcraft-sandbox-test

# Compile/report a native resource package rooted at assets/<namespace>/textures/.
resource-report root package:
    cargo run -p rustcraft-content --bin rustcraft-resource -- report {{quote(root)}} {{quote(package)}}

inspect-resource root package resource:
    cargo run -p rustcraft-content --bin rustcraft-resource -- inspect {{quote(root)}} {{quote(package)}} {{quote(resource)}}

atlas-debug root package output="target/atlas-debug":
    cargo run -p rustcraft-content --bin rustcraft-resource -- atlas-debug {{quote(root)}} {{quote(package)}} {{quote(output)}}

resource-stress count="1000":
    cargo run -p rustcraft-content --bin rustcraft-resource -- stress {{quote(count)}}

minecraft-resource-report:
    cargo run -p rustcraft-client -- --resource-report

# Explicit trusted developer tools; ordinary startup does not execute scripts.
dev-client:
    cargo run -p rustcraft-client -- --devtools

script-check path="scripts":
    cargo run -p rustcraft-server -- --script-check "{{path}}"

scenario-headless path="scripts/scenarios/dx_smoke.rhai":
    cargo run -p rustcraft-server -- --scenario "{{path}}"

scenario-client path="scripts/scenarios/dx_smoke.rhai":
    cargo run -p rustcraft-client -- --devtools --scenario "{{path}}"

dx-smoke: script-check scenario-headless

script-bench:
    cargo run --release -p rustcraft-server -- --script-bench

# Real-window DX service overhead; output target/dx-overhead.json.
dx-overhead:
    cargo run --release -p rustcraft-client -- --dx-overhead

# Shared console/editor and async lifecycle regression gates.
dx-test:
    cargo test -p rustcraft-control -p rustcraft-scripting-rhai

dx-console:
    cargo run -p rustcraft-client -- --devtools --scenario scripts/scenarios/dx_console.rhai

# Bounded shared diagnostics/selector tests and real-surface acceptance (local assets required).
dux-test:
    cargo test -p rustcraft-control -p rustcraft-scripting-rhai
    cargo test -p rustcraft-client devtools

dux-client:
    cargo run --release -p rustcraft-client -- --dux-acceptance

# Shared configuration contracts and headless operational readback.
config-test:
    cargo test -p rustcraft-config -p rustcraft-control
    cargo test -p rustcraft-client configuration

config-smoke:
    cargo run -p rustcraft-server -- --config-smoke

# Focused developer controls, Unicode editor, bundled-font/resource/cache acceptance.
ux-test:
    cargo test -p rustcraft-client developer_input
    cargo test -p rustcraft-scripting-rhai ux_editor
    cargo test -p rustcraft-render text::
    cargo test -p rustcraft-config ux_scale
    cargo test -p rustcraft-content fonts::

# Real graphical surface, disposable acceptance world; no normal save mutation.
ux-client:
    cargo run --release -p rustcraft-client -- --ux1-acceptance

# Shared ownership/race/pressure gates; full travel remains world-travel-test.
rsm1-test:
    cargo test -p rustcraft-world rsm1_
    cargo test -p rustcraft-render meshing::
    cargo test -p rustcraft-runtime rsm1_
    cargo test -p rustcraft-client rsm1_ -- --test-threads=1

# Real surface, disposable world, shared C1 policy and scalar lifetime receipts.
rsm1-client:
    cargo run --release -p rustcraft-client -- --rsm1-acceptance --set-config rustcraft:streaming/load_radius=3 --set-config rustcraft:streaming/retain_radius=4 --set-config rustcraft:lighting/work_budget=256 --set-config rustcraft:streaming/main_budget_ms=8 --set-config rustcraft:meshing/upload_sections=64

# Bounded application clocks and transient presentation correctness; no world ownership changes.
p1-test:
    cargo test -p rustcraft-client presentation::
    cargo test -p rustcraft-client p1_ -- --test-threads=1

# Matched six 20-second motion phases, disposable world, timing summary and actual surface capture.
p1-client:
    cargo run -p rustcraft-client -- --p1-acceptance

# Bounded physical-store/recovery correctness; release scale is opt-in.
s1-test:
    cargo test -p rustcraft-world s1_

s1-bench columns="10000" cycles="100" output="target/s1":
    mkdir -p {{quote(output)}}
    cargo run --release -p rustcraft-world --example s1 -- {{columns}} {{cycles}} {{quote(output)}}/current.json
    cargo run --release -p rustcraft-world --example s1_split -- {{columns}} {{cycles}} {{quote(output)}}/split.json
    cargo run --release -p rustcraft-world --example s1 -- --auxiliary {{quote(output)}}/auxiliary.json

# Offline documentation integrity; no Rust build or GitHub access.
docs-check:
    {{python}} scripts/docs.py check

# Lightweight documentation-tool regression tests.
docs-test:
    {{python}} -m unittest discover -s scripts/tests -p 'test_docs.py'

# Update only generated navigation views.
docs-sync:
    {{python}} scripts/docs.py sync

# Compact task context. Example: just codex-context F1 --offline
codex-context *args:
    {{python}} scripts/docs.py context {{args}}

# Insert a planned stage after an existing stage; no commits or GitHub mutations.
stage-new id title after:
    {{python}} scripts/docs.py new-stage {{quote(id)}} {{quote(title)}} --after {{quote(after)}}

# Current tracked paths, replacing a committed stale tree snapshot.
project-tree:
    git ls-files
