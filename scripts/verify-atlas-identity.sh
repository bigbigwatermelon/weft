#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

old_lower="$(printf '%s%s%s%s' w e f t)"
old_pascal="$(printf '%s%s' W "${old_lower#?}")"
old_upper="$(printf '%s' "$old_lower" | tr '[:lower:]' '[:upper:]')"

pattern="$(
  printf '%s' \
    "${old_upper}_|" \
    "${old_lower}_bus|" \
    "${old_lower}_planner|" \
    "/${old_lower}-mark\\.svg|" \
    "${old_lower}-dangerous|" \
    "${old_lower}-keep-awake|" \
    "${old_lower}-idle-cap-mins|" \
    "${old_lower}-theme|" \
    "layer_${old_lower}|" \
    "${old_lower}\\.db|" \
    "${old_lower}_app_lib|" \
    "\\b${old_lower}\\b|" \
    "${old_pascal}"
)"

paths=(
  src
  src-tauri/src
  src-tauri/tests
  public
  index.html
  scripts
)

if rg -n "$pattern" "${paths[@]}"; then
  echo "Old runtime identity markers remain." >&2
  exit 1
fi
