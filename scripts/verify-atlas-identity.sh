#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

old_lower="$(printf '%s%s%s%s' w e f t)"
old_pascal="$(printf '%s%s' W "${old_lower#?}")"
old_upper="$(printf '%s' "$old_lower" | tr '[:lower:]' '[:upper:]')"

content_pattern="$(
  printf '%s' \
    "${old_upper}_|" \
    "(^|[^[:alnum:]_])${old_upper}([^[:alnum:]_]|$)|" \
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
    "(^|[^[:alnum:]_])${old_lower}([^[:alnum:]_]|$)|" \
    "(^|[^[:alnum:]_])${old_pascal}([^[:alnum:]_]|$)"
)"

required_tracked=(
  assets/brand/atlas-icon-embedded.png
  assets/brand/atlas-icon-master.png
  assets/brand/atlas-icon-source.svg
  public/atlas-icon.png
  public/atlas-mark.png
  src-tauri/icons/icon.png
  src-tauri/icons/icon.icns
  src-tauri/icons/icon.ico
)

for path in "${required_tracked[@]}"; do
  if ! git ls-files --error-unmatch "$path" >/dev/null 2>&1; then
    echo "Required Atlas asset is not tracked: $path" >&2
    exit 1
  fi
  if [[ ! -s "$path" ]]; then
    echo "Required Atlas asset is missing or empty: $path" >&2
    exit 1
  fi
done

if git grep -I -n -E "$content_pattern" -- .; then
  echo "Old runtime identity markers remain in file contents." >&2
  exit 1
fi

filename_pattern="$(
  printf '%s' \
    "${old_upper}|" \
    "${old_lower}_bus|" \
    "${old_lower}_planner|" \
    "mcp__${old_lower}|" \
    "${old_lower}-mark\\.svg|" \
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
    "\\.${old_lower}|" \
    "${old_lower}|" \
    "${old_pascal}"
)"
if git ls-files | rg -n "$filename_pattern"; then
  echo "Old runtime identity markers remain in filenames." >&2
  exit 1
fi
