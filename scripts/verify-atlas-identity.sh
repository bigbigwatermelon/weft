#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

old_lower="$(printf '%s%s%s%s' w e f t)"
old_pascal="$(printf '%s%s' W "${old_lower#?}")"
old_upper="$(printf '%s' "$old_lower" | tr '[:lower:]' '[:upper:]')"

content_pattern="$(
  printf '%s' \
    "${old_upper}_|" \
    "\\b${old_upper}\\b|" \
    "${old_lower}_bus|" \
    "${old_lower}_planner|" \
    "mcp__${old_lower}|" \
    "/${old_lower}-mark\\.svg|" \
    "${old_lower}-(icon|logo|mark)\\.svg|" \
    "${old_lower}-recovery-key\\.json|" \
    "${old_lower}-dangerous|" \
    "${old_lower}-keep-awake|" \
    "${old_lower}-idle-cap-mins|" \
    "${old_lower}-wall-cap-mins|" \
    "${old_lower}-projects-dir|" \
    "${old_lower}-review-skill|" \
    "${old_lower}-auto-review|" \
    "${old_lower}-notify|" \
    "${old_lower}-theme|" \
    "layer_${old_lower}|" \
    "${old_lower}\\.db|" \
    "${old_lower}_app_lib|" \
    "${old_lower}-app|" \
    "com\\.jingchen\\.${old_lower}|" \
    "~/.${old_lower}|" \
    "\\b${old_lower}\\b|" \
    "\\b${old_pascal}\\b"
)"

paths=(
  AGENTS.md
  ARCHITECTURE.md
  DESIGN.md
  PRODUCT.md
  README.md
  README.zh-CN.md
  assets/diagrams
  assets/readme
  src
  src-tauri/Cargo.toml
  src-tauri/Cargo.lock
  src-tauri/build.rs
  src-tauri/capabilities
  src-tauri/src
  src-tauri/tests
  src-tauri/tauri.conf.json
  public
  index.html
  docs/superpowers
  scripts
)

existing_paths=()
for path in "${paths[@]}"; do
  if [[ -e "$path" ]]; then
    existing_paths+=("$path")
  fi
done

if rg -n "$content_pattern" "${existing_paths[@]}" \
  --glob '!*.png' \
  --glob '!*.jpg' \
  --glob '!*.jpeg' \
  --glob '!*.webp' \
  --glob '!*.gif' \
  --glob '!*.icns' \
  --glob '!*.ico'
then
  echo "Old runtime identity markers remain in file contents." >&2
  exit 1
fi

filename_pattern="$content_pattern"
if rg --files "${existing_paths[@]}" | rg -n "$filename_pattern"; then
  echo "Old runtime identity markers remain in filenames." >&2
  exit 1
fi
