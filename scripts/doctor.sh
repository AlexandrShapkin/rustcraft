#!/usr/bin/env bash
set -uo pipefail

ok=0
missing=0

check_cmd() {
  local label="$1"
  local cmd="$2"
  if command -v "$cmd" >/dev/null 2>&1; then
    local version
    version="$($cmd --version 2>/dev/null | head -n1 || true)"
    printf '[ok]      %-18s %s\n' "$label" "${version:-$cmd}"
    ok=$((ok + 1))
  else
    printf '[missing] %-18s %s\n' "$label" "$cmd"
    missing=$((missing + 1))
  fi
}

check_cargo_subcommand() {
  local label="$1"
  local sub="$2"
  if cargo "$sub" --version >/dev/null 2>&1; then
    local version
    version="$(cargo "$sub" --version 2>/dev/null | head -n1 || true)"
    printf '[optional] %-17s %s\n' "$label" "$version"
  else
    printf '[optional] %-17s not installed\n' "$label"
  fi
}

echo '== Required / baseline =='
check_cmd git git
if command -v python3 >/dev/null 2>&1 && python3 -c 'import tomllib' >/dev/null 2>&1; then
  printf '[ok]      %-18s %s\n' 'Python 3.11+' "$(python3 --version)"
  ok=$((ok + 1))
else
  printf '[missing] %-18s %s\n' 'Python 3.11+' 'python3 with stdlib tomllib required for docs/CI'
  missing=$((missing + 1))
fi
check_cmd rustc rustc
check_cmd cargo cargo
check_cmd rustfmt rustfmt
check_cmd clippy cargo-clippy
check_cmd just just
check_cmd ripgrep rg
check_cmd jq jq

echo
echo '== Useful shell/network helpers =='
for pair in 'fd:fd' 'delta:delta' 'xh:xh' 'yq:yq' 'doggo:doggo' 'websocat:websocat' 'wl-copy:wl-copy' 'hyperfine:hyperfine' 'samply:samply' 'flamegraph:flamegraph' 'perf:perf' 'wasm-tools:wasm-tools'; do
  label="${pair%%:*}"
  cmd="${pair##*:}"
  if command -v "$cmd" >/dev/null 2>&1; then
    version="$($cmd --version 2>/dev/null | head -n1 || true)"
    printf '[optional] %-17s %s\n' "$label" "${version:-$cmd}"
  else
    printf '[optional] %-17s not installed\n' "$label"
  fi
done

echo
echo '== Optional Cargo tooling =='
if command -v cargo >/dev/null 2>&1; then
  check_cargo_subcommand nextest nextest
  check_cargo_subcommand audit audit
  check_cargo_subcommand deny deny
  check_cargo_subcommand machete machete
  check_cargo_subcommand llvm-cov llvm-cov
  check_cargo_subcommand bloat bloat
  check_cargo_subcommand semver-checks semver-checks
  check_cargo_subcommand mutants mutants
else
  echo 'cargo unavailable; Cargo subcommands not checked'
fi

echo
printf 'baseline commands present: %d; missing: %d\n' "$ok" "$missing"
if (( missing > 0 )); then
  echo 'Install missing baseline tools before relying on the standard just recipes.'
  exit 1
fi
