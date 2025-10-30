# Local validation of GitHub Actions with act, pytest, and cmd‑mox

Use `act` for fast, local feedback on workflow logic; `pytest` for assertive,
automated checks; and `cmd‑mox` to stub or record external commands your action
calls (`gh`, `aws`, `docker`, bespoke CLIs). Treat this as a **pre‑CI
smoke/integration layer**; GitHub-hosted runners remain the source of truth for
permissions, OIDC, and runner images.

## TL;DR

- **Unit test your action code** directly (plain `pytest`).
- **Integration test the workflow** via `act`, driven by `pytest`.
- **Quarantine the outside world** with `cmd‑mox` (record → replay → verify
  calls).
- **Certify on GitHub** after local green.

## Prerequisites

- Docker daemon running.
- `act` installed (Homebrew, Scoop, or binary).
- Python 3.10+ with `pytest`.
- `cmd‑mox` installed and the pytest plugin enabled (guarded on Windows):

  ```python
  # conftest.py
  import sys

  if sys.platform != "win32":
      pytest_plugins = ("cmd_mox.pytest_plugin",)
  ```

> Tip: Pin an `act` runner image to reduce drift by adding
> `-P ubuntu-latest=catthehacker/ubuntu:act-latest`.

## Minimal layout

```text
.yamllint
.github/workflows/selftest.yml
tests/
  fixtures/pull_request.event.json
  test_workflow_integration.py
```

### Example workflow

This job pretends to call an external CLI (`gh`) whose behaviour we will
stub/record with `cmd‑mox`.

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
      - name: Query latest release via gh
        id: query
        run: |
          set -euo pipefail
          # External dependency we will mock/record
          gh release view --json tagName > out.json
          jq -r '.tagName' out.json | tee out.tag
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: release-tag
          path: out.tag
```

### Event payload (fixture)

```json
// tests/fixtures/pull_request.event.json
{
  "pull_request": {"number": 1, "head": {"ref": "test-branch"}},
  "repository": {"full_name": "example/repo"},
  "sender": {"login": "tester"}
}
```

## Driving `act` from `pytest`

Below is a compact harness you can drop into
`tests/test_workflow_integration.py`. It shells out to `act`, captures
artifacts, and integrates `cmd‑mox` for command stubbing or mocking via the
fixture provided by `cmd_mox.pytest_plugin`.

```python
# tests/test_workflow_integration.py
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import pytest

EVENT = Path("tests/fixtures/pull_request.event.json")


def run_act(job: str = "selftest") -> tuple[int, Path, str]:
    """Execute `act` and return (exit_code, artifact_dir, logs)."""

    artifact_dir = Path(tempfile.mkdtemp(prefix="act-artifacts-"))
    command = [
        "act",
        "pull_request",
        "-j",
        job,
        "-e",
        str(EVENT),
        "-P",
        "ubuntu-latest=catthehacker/ubuntu:act-latest",
        "--artifact-server-path",
        str(artifact_dir),
        "-v",
    ]
    completed = subprocess.run(
        command,
        capture_output=True,
        text=True,
        env=os.environ.copy(),
    )
    logs = completed.stdout + "\n" + completed.stderr
    return completed.returncode, artifact_dir, logs


@pytest.mark.skipif(sys.platform == "win32", reason="cmd-mox ships Unix shims")
def test_workflow_with_cmd_mox_replay(cmd_mox) -> None:
    """Replay a golden response for `gh release view` and verify artefacts."""

    cmd_mox.mock("gh").with_args(
        "release",
        "view",
        "--json",
        "tagName",
    ).returns(stdout='{"tagName":"v9.9.9"}\n', exit_code=0)

    cmd_mox.replay()

    code, artifacts, logs = run_act()
    assert code == 0, f"act failed:\n{logs}"

    matches = list(artifacts.rglob("release-tag*/out.tag"))
    assert matches, f"No artifact found. Logs:\n{logs}"
    assert matches[0].read_text(encoding="utf-8").strip() == "v9.9.9"

    cmd_mox.verify()
    shutil.rmtree(artifacts, ignore_errors=True)
```

`cmd_mox.replay()` installs shims and environment variables (notably `PATH`
and `CMOX_IPC_SOCKET`) so any subprocesses spawned inside the replay window are
observed. `cmd_mox.verify()` asserts that the declared expectations were met and
that no unexpected commands were invoked.

## Record → replay → verify (closing the loop)

`cmd‑mox` is designed for **record–replay–verify**. A practical workflow:

1. **Record** a golden trace once. Use a spy with passthrough to capture actual
   behaviour while letting the real command run:

   ```python
   with CmdMox() as mox:
       gh = mox.spy("gh").passthrough()
       mox.replay()
       code, _, logs = run_act()
       assert code == 0, logs
       mox.verify()
       assert gh.call_count == 1
   ```

2. **Replay** deterministically. Replace the spy with a mock or stub and
   configure the expected payload using the fluent API shown in the pytest
   example above. Keep verification mandatory so regressions surface quickly.

3. **Inspect the journal**. After verification, the `cmd_mox.journal`
   deque exposes the `Invocation` objects captured during replay. Serialise the
   data in whatever format fits your project (JSON lines, YAML, etc.) so future
   test runs can bootstrap mocks from the same expectations.

## What to assert (beyond exit code)

- **Artifacts:** contents, line endings, naming convention.
- **Logs:** presence or absence of critical lines (e.g., cache key, matrix
  values).
- **Side-effects:** created files, modified config, version stamp.
- **External calls:** `cmd‑mox` journal and spy helpers confirm the commands you
  expected (no more, no less).

## Known limitations

- **Runner parity:** `act` images differ from `ubuntu‑latest` toolcache. Validate
  logic, not performance.
- **Permissions/OIDC:** not faithfully reproduced locally; keep those tests on
  GH.
- **Services:** service containers work, but health-checks and DNS timing can
  diverge.

## Suggested ladder

1. **Fast local loop:** unit tests → `act` + `pytest` + `cmd‑mox`.
2. **Authoritative CI:** same workflow on GitHub runners.
3. **End-to-end (privileged):** GH-only, least-privilege tokens, feature-gated.

This setup keeps your feedback loop tight, your external surfaces deterministic,
and your confidence proportional to reality.
