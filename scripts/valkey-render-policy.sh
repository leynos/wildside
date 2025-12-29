#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
EXAMPLE_DIR="$REPO_ROOT/infra/modules/valkey/examples/render"
POLICY_DIR="$REPO_ROOT/infra/modules/valkey/policy/manifests"

if ! command -v tofu >/dev/null 2>&1; then
  echo "tofu must be installed to run valkey render policy checks" >&2
  exit 1
fi

if ! command -v conftest >/dev/null 2>&1; then
  echo "conftest must be installed to run valkey render policy checks" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 must be installed to run valkey render policy checks" >&2
  exit 1
fi

tmpdir=$(mktemp -d)
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

out_dir="$tmpdir/out"
mkdir -p "$out_dir"

log_file="$tmpdir/tofu.log"
output_log="$tmpdir/tofu-output.log"
state_file="$tmpdir/terraform.tfstate"

# Redirect .terraform and state to temp directory to avoid leaving artifacts
export TF_DATA_DIR="$tmpdir/.terraform"

if ! TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" init -input=false -no-color > "$log_file" 2>&1; then
  echo "tofu init failed:" >&2
  cat "$log_file" >&2
  exit 1
fi

if ! TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" apply \
  -auto-approve -input=false -no-color \
  -state="$state_file" \
  > "$log_file" 2>&1; then
  echo "tofu apply failed:" >&2
  cat "$log_file" >&2
  exit 1
fi

if ! TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" output \
  -json -state="$state_file" rendered_manifests \
  > "$tmpdir/manifests.json" 2> "$output_log"; then
  echo "tofu output failed:" >&2
  cat "$output_log" >&2
  exit 1
fi

RENDER_POLICY_TMP="$tmpdir" python3 - <<'PY'
from __future__ import annotations

import json
import os
from pathlib import Path

root = Path(os.environ["RENDER_POLICY_TMP"])
out_dir = root / "out"
# tofu output -json wraps the value in {"sensitive": bool, "type": ..., "value": ...}
raw = json.loads((root / "manifests.json").read_text())
payload = raw.get("value", {}) if isinstance(raw, dict) else {}
if not isinstance(payload, dict):
    raise SystemExit(f"expected rendered_manifests.value to be a JSON object, got {type(payload)}")

for rel_path, content in payload.items():
    if not isinstance(rel_path, str) or not rel_path:
        raise SystemExit(f"invalid manifest key: {rel_path!r}")
    if not isinstance(content, str):
        raise SystemExit(f"invalid manifest content for {rel_path}: {type(content)}")
    dest = out_dir / rel_path
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(content)

# Ensure the output directory exists and contains files, otherwise conftest's
# error messages are less actionable than failing explicitly here.
paths = [p for p in out_dir.rglob("*") if p.is_file()]
if not paths:
    raise SystemExit(f"no rendered manifests written under {out_dir}")
PY

conftest_log="$tmpdir/conftest.log"
if ! conftest test "$out_dir" --policy "$POLICY_DIR" --namespace valkey.policy.manifests --combine > "$conftest_log" 2>&1; then
  echo "conftest policy validation failed:" >&2
  cat "$conftest_log" >&2
  exit 1
fi
cat "$conftest_log"
