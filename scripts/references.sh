#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCES="$ROOT/reference/sources.json"
LOCK="$ROOT/reference/LOCK.json"

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "required command missing: $1" >&2
    exit 1
  }
}

require git
require jq

source_rows() {
  jq -r '.sources[] | select(.enabled == true) | [.id, .url, .path, .tier] | @tsv' "$SOURCES"
}

fetch_one() {
  local id="$1" url="$2" rel="$3" tier="$4"
  local path="$ROOT/$rel"
  if [[ -d "$path/.git" ]]; then
    printf '[present] %-20s %s\n' "$id" "$rel"
    return
  fi
  if [[ -e "$path" ]]; then
    printf '[skip]    %-20s path exists but is not a git clone: %s\n' "$id" "$rel" >&2
    return 1
  fi
  mkdir -p "$(dirname "$path")"
  printf '[clone]   %-20s %s\n' "$id" "$url"
  git clone --depth 1 "$url" "$path"
}

cmd_fetch() {
  local wanted_tier="${1:-all}"
  while IFS=$'\t' read -r id url rel tier; do
    if [[ "$wanted_tier" != "all" && "$tier" != "$wanted_tier" ]]; then
      continue
    fi
    fetch_one "$id" "$url" "$rel" "$tier"
  done < <(source_rows)
}

cmd_update() {
  while IFS=$'\t' read -r id url rel tier; do
    local path="$ROOT/$rel"
    if [[ ! -d "$path/.git" ]]; then
      printf '[missing] %-20s %s\n' "$id" "$rel"
      continue
    fi
    printf '[update]  %-20s ' "$id"
    if git -C "$path" diff --quiet && git -C "$path" diff --cached --quiet; then
      if git -C "$path" symbolic-ref -q HEAD >/dev/null; then
        git -C "$path" pull --ff-only --quiet
        printf '%s\n' "$(git -C "$path" rev-parse --short=12 HEAD)"
      else
        printf 'detached at %s (not updated)\n' "$(git -C "$path" rev-parse --short=12 HEAD)"
      fi
    else
      printf 'dirty working tree (not updated)\n'
    fi
  done < <(source_rows)
}

cmd_status() {
  printf '%-21s %-12s %-14s %s\n' SOURCE TIER REVISION PATH
  printf '%-21s %-12s %-14s %s\n' '--------------------' '----------' '------------' '----'
  while IFS=$'\t' read -r id url rel tier; do
    local path="$ROOT/$rel"
    if [[ -d "$path/.git" ]]; then
      local rev branch dirty=''
      rev="$(git -C "$path" rev-parse --short=12 HEAD)"
      branch="$(git -C "$path" symbolic-ref --short -q HEAD || echo detached)"
      if ! git -C "$path" diff --quiet || ! git -C "$path" diff --cached --quiet; then
        dirty='*'
      fi
      printf '%-21s %-12s %-14s %s (%s%s)\n' "$id" "$tier" "$rev" "$rel" "$branch" "$dirty"
    else
      printf '%-21s %-12s %-14s %s\n' "$id" "$tier" MISSING "$rel"
    fi
  done < <(source_rows)

  echo
  local source_path="$ROOT/reference/minecraft-beta-1.7.3-src"
  local tex_dir="$ROOT/reference/assets/vanilla-b1.7.3"
  local tex_zip="$ROOT/reference/assets/vanilla-b1.7.3.zip"
  [[ -e "$source_path" ]] && echo '[local] historical source: present' || echo '[local] historical source: missing (optional)'
  if [[ -d "$tex_dir" ]]; then
    echo '[local] vanilla texture pack: present (directory)'
  elif [[ -f "$tex_zip" ]]; then
    echo '[local] vanilla texture pack: present (zip)'
  else
    echo '[local] vanilla texture pack: missing (optional)'
  fi
}

cmd_lock() {
  local tmp
  tmp="$(mktemp)"
  local generated_at
  generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  jq -n --arg generated_at "$generated_at" '{schema: 1, generated_at: $generated_at, sources: {}}' > "$tmp"

  while IFS=$'\t' read -r id url rel tier; do
    local path="$ROOT/$rel"
    if [[ -d "$path/.git" ]]; then
      local rev branch
      rev="$(git -C "$path" rev-parse HEAD)"
      branch="$(git -C "$path" symbolic-ref --short -q HEAD || echo detached)"
      jq --arg id "$id" --arg url "$url" --arg path "$rel" --arg tier "$tier" \
         --arg rev "$rev" --arg branch "$branch" \
         '.sources[$id] = {url: $url, path: $path, tier: $tier, revision: $rev, branch: $branch}' \
         "$tmp" > "$tmp.next"
      mv "$tmp.next" "$tmp"
    fi
  done < <(source_rows)

  mv "$tmp" "$LOCK"
  echo "wrote reference/LOCK.json"
}

case "${1:-status}" in
  fetch) cmd_fetch "${2:-all}" ;;
  update) cmd_update ;;
  status) cmd_status ;;
  lock) cmd_lock ;;
  *)
    echo "usage: $0 {fetch [all|core|supplemental]|update|status|lock}" >&2
    exit 2
    ;;
esac
