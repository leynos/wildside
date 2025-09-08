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
all_pnpm=$(fd --strip-cwd-prefix -t f package.json -E node_modules || true)
configured=(
  "package.json"
  "frontend-pwa/package.json"
  "packages/tokens/package.json"
  "packages/types/package.json"
)

mapfile -t all_pnpm_arr <<<"$all_pnpm"
for p in "${all_pnpm_arr[@]}"; do
  if ! printf '%s\n' "${configured[@]}" | grep -Fxq "$p"; then
    echo "UNCONFIGURED pnpm package.json: $p"
  fi
done

if [[ $missing -ne 0 ]]; then
  echo "One or more expected manifests are missing."
  exit 1
fi
echo "All expected manifests found."
