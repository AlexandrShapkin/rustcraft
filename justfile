set shell := ["bash", "-euc"]

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
ci: fmt-check check test lint

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

# Deterministic offscreen fidelity suite, no surface or visible window.
fidelity-m3:
    cargo run -p rustcraft-client -- --fidelity-m3

# Surface-independent PNG inspector. Modes: solid, corners, uv, atlas.
render-test scene *args:
    cargo run -p rustcraft-client -- --render-test {{quote(scene)}} {{args}}

render-test-all:
    cargo run -p rustcraft-client -- --render-test-all

survival-scenario:
    cargo run -p rustcraft-server -- --survival

# Non-Minecraft Game API integration slice. Fails if its normal dependency graph gains the
# first-party Minecraft package, legacy Minecraft gameplay crates, or M0-M3 policy runtime.
sample-game:
    @deps="$$(cargo tree -p rustcraft-sandbox-test --edges normal --prefix none)"; if rg -q 'rustcraft-(minecraft-b173|gameplay-blocks|gameplay-flat-world|runtime)' <<<"$$deps"; then printf '%s\n' "$$deps" >&2; echo "sample-game dependency boundary violated" >&2; exit 1; fi
    cargo run -p rustcraft-sandbox-test
