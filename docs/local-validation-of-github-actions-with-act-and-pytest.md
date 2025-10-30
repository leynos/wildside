# Local validation of GitHub Actions with act and pytest (black‑box)

This guide focuses on **pre‑CI smoke/integration testing** of a workflow using
`act` and `pytest`, treating the workflow as a **black box**. Assertions are
made on artefacts, workspace side‑effects, and structured logs. We
intentionally avoid host‑side command interception; containers execute in
isolation.

## TL;DR

- Keep **unit tests** in your action’s codebase (plain `pytest` or your
  language’s runner).
- Exercise the **workflow** locally via `act`, from a `pytest` harness.
- Assert on **artefacts**, **file outputs**, and **logs** (using `act --json`).
- Treat results as pre‑CI confidence; certify on GitHub runners for
  permissions/OIDC parity.

## Prerequisites

- Docker daemon available.
- `act` installed.
- Python 3.10+ with `pytest`.
- Optional but recommended: pin an image to reduce drift:

  ```bash
  act pull_request -P ubuntu-latest=catthehacker/ubuntu:act-latest --list
  ```

## Minimal layout

```plaintext
.github/workflows/selftest.yml
scripts/
  # optional helper scripts used by the workflow
tests/
  fixtures/pull_request.event.json
  test_workflow_integration.py
```

### Example workflow (self‑checking)

This job builds a tiny JSON artefact with environment/version data and uploads
it. That gives us deterministic material to assert on from the host.

```yaml
# .github/workflows/selftest.yml
name: selftest
on:
  workflow_dispatch:
  pull_request:
jobs:
  selftest:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build artefact
        run: |
          set -euo pipefail
          mkdir -p out
          python - <<'PY'
          import json, os, platform, sys
          print("Hello from workflow")
          data = {
            "status": "ok",
            "python": sys.version.split()[0],
            "os": platform.platform(),
            "env": {
              "CI": os.getenv("CI", ""),
              "GITHUB_REF": os.getenv("GITHUB_REF", ""),
            },
          }
          open("out/result.json", "w").write(json.dumps(data))
          PY
      - name: Upload artefact
        uses: actions/upload-artifact@v4
        with:
          name: result
          path: out/result.json
```

### Event payload fixture

```json
// tests/fixtures/pull_request.event.json
{
  "pull_request": {"number": 1, "head": {"ref": "test-branch"}},
  "repository": {"full_name": "example/repo"},
  "sender": {"login": "tester"}
}
```

## Driving `act` from `pytest` (black‑box harness)

The harness runs `act`, captures artefacts into a temp directory, and reads the
JSON log stream. It makes **no attempt** to intercept commands inside the
containers.

```python
# tests/test_workflow_integration.py
from __future__ import annotations
import json, subprocess, tempfile
from pathlib import Path

def run_act(job: str = "selftest", event_path: str = "tests/fixtures/pull_request.event.json"):
    artdir = Path(tempfile.mkdtemp(prefix="act-artifacts-"))
    cmd = [
        "act", "pull_request",
        "-j", job,
        "-e", event_path,
        "-P", "ubuntu-latest=catthehacker/ubuntu:act-latest",
        "--artifact-server-path", str(artdir),
        "--json",  # machine-parseable log stream
        "-b",      # bind-mount repo as workspace (preserves side-effects)
    ]
    cp = subprocess.run(cmd, text=True, capture_output=True)
    return cp.returncode, artdir, cp.stdout + "\n" + cp.stderr


def test_workflow_produces_expected_artefact_and_logs():
    code, artdir, logs = run_act()
    assert code == 0, f"act failed:\n{logs}"

    # Assert artefact presence and contents
    files = list(artdir.rglob("result*/result.json"))
    assert files, f"artefact missing. Logs:\n{logs}"
    data = json.loads(files[0].read_text())
    assert data["status"] == "ok"
    assert data["python"].startswith("3."), data["python"]

    # Assert on log stream — act --json prints one JSON document per line
    saw_greeting = False
    for line in logs.splitlines():
        if not line.lstrip().startswith("{"):
            continue
        try:
            evt = json.loads(line)
        except json.JSONDecodeError:
            continue
        out = evt.get("Output") or evt.get("message") or ""
        if "Hello from workflow" in out:
            saw_greeting = True
            break
    assert saw_greeting, "expected greeting in structured logs"
```

## What to assert (beyond exit code)

- **Artefacts:** existence, schema, and specific fields; normalise line endings
  if you care about CRLF.
- **Workspace side‑effects:** files created/modified when using `-b`.
- **Structured logs:** look for key lines (cache keys, matrix values, tool
  versions). Prefer `--json` and parse rather than grepping raw TTY output.
- **Idempotence:** run the same job twice and assert identical artefacts (or
  intentional cache hits).

## Useful `act` flags in this setup

- `-P ubuntu-latest=catthehacker/ubuntu:act-latest` — pin a close runner image.
- `-b/--bind` — bind‑mount the repository; enables checking file side‑effects.
- `--artifact-server-path <dir>` — export uploaded artefacts to a host
  directory.
- `--json` — emit a line‑delimited JSON log stream suitable for parsing.
- `-e <event.json>` / `--env` / `--env-file` — control the event and
  environment under test.

## Known limitations (by design)

- **Runner parity:** `act` images are close, not identical, to `ubuntu‑latest`.
- **Permissions/OIDC:** token scopes, OIDC federation, and GitHub‑provided
  credentials cannot be faithfully validated locally; rely on GH runners.
- **Service containers & networking:** usually fine but can diverge under load
  or with subtle DNS/health‑check timing.

## Validation ladder

1. **Local fast loop:** unit tests → `act` black‑box tests via `pytest`.
2. **Authoritative CI:** run the same workflow on GitHub‑hosted runners.
3. **End‑to‑end (privileged paths):** GH‑only with least‑privilege tokens; gate
   behind labels/paths.

This gives you tight feedback for workflow correctness and orchestration logic,
without pretending local containers are perfect stand‑ins for GitHub’s
environment.
