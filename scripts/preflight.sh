#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

usage() {
  cat <<'USAGE'
Usage: scripts/preflight.sh [--quick|--full]

Runs the local pre-PR gate used before pushing a branch.

  --full   run frontend build, Atlas identity check, and Rust tests (default)
  --quick  run frontend build, Atlas identity check, and Rust test compilation

Set PREFLIGHT_BASE_REF to override the whitespace diff base.
USAGE
}

mode="${1:---full}"
case "$mode" in
  --full|full)
    rust_args=(test --manifest-path src-tauri/Cargo.toml)
    ;;
  --quick|quick)
    rust_args=(test --manifest-path src-tauri/Cargo.toml --no-run)
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

run() {
  printf '\n==> '
  printf '%q ' "$@"
  printf '\n'
  "$@"
}

base_ref="${PREFLIGHT_BASE_REF:-}"
if [[ -n "$base_ref" ]]; then
  if ! git rev-parse --verify --quiet "$base_ref" >/dev/null; then
    echo "PREFLIGHT_BASE_REF does not exist locally: $base_ref" >&2
    exit 2
  fi
else
  for candidate in fork/main origin/main main; do
    if git rev-parse --verify --quiet "$candidate" >/dev/null; then
      base_ref="$candidate"
      break
    fi
  done
fi

if [[ -n "$base_ref" ]]; then
  run git diff --check "$base_ref...HEAD"
else
  run git diff --check
fi

run scripts/verify-atlas-identity.sh
run pnpm build
run cargo "${rust_args[@]}"
