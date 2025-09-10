#!/bin/bash
set -euo pipefail

missing=0
check() { [[ -f "$1" ]] || { echo "MISS: $1"; missing=1; }; }

# Expected manifests (paths are repo-root relative)
check backend/Cargo.toml
check package.json
check frontend-pwa/package.json
check packages/tokens/package.json
check packages/types/package.json
check deploy/charts/wildside/Chart.yaml

echo "---- Scan for unconfigured pnpm packages ----"
# Find all package.json, strip leading ./, ignore node_modules
if ! command -v fd >/dev/null 2>&1; then
  echo "fd not found; falling back to find" >&2
  all_pnpm=$(find . -type f -name package.json -not -path "*/node_modules/*" -printf "%P\n" || true)
else
  all_pnpm=$(fd --strip-cwd-prefix -t f package.json -E node_modules || true)
fi
configured=(
  "package.json"
  "frontend-pwa/package.json"
  "packages/tokens/package.json"
  "packages/types/package.json"
)

unconfigured=0
mapfile -t all_pnpm_arr <<<"$all_pnpm"
for p in "${all_pnpm_arr[@]}"; do
  if ! printf '%s\n' "${configured[@]}" | grep -Fxq "$p"; then
    echo "UNCONFIGURED pnpm package.json: $p"
    unconfigured=1
  fi
done
[[ $unconfigured -eq 0 ]] || { echo "Unconfigured pnpm packages detected"; exit 1; }

if [[ $missing -ne 0 ]]; then
  echo "One or more expected manifests are missing."
  exit 1
fi
echo "All expected manifests found."
