# Local validation of GitHub Actions with act, pytest, and cmd‑mox

Use `act` for fast, local feedback on workflow logic; `pytest` for assertive,
automated checks; and `cmd‑mox` to stub or record external commands your action
calls (`gh`, `aws`, `docker`, bespoke CLIs). Treat this as a **pre‑CI
smoke/integration layer**; GitHub‑hosted runners remain the source of truth for
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
- `cmd‑mox` in your dev environment (repo: <https://github.com/leynos/cmd-mox>),
  installed in editable mode if you like: `pip install -e ./cmd-mox` (adjust to
  your layout).

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

Below is a compact harness you can drop into `tests/test_workflow_integration.py`.
It shells out to `act`, captures artifacts, and integrates `cmd‑mox` for command
stubbing and recording.

> **Note on API shape:** The calls to `cmd‑mox` below illustrate the intended
> flow (stub → run → verify). If your current API names differ, translate
> accordingly.

```python
# tests/test_workflow_integration.py
import json, os, subprocess, tempfile, pathlib, shutil

# Pseudocode-level API. Adjust to the actual cmd-mox API in your branch.
# from cmd_mox import MoxSession
class MoxSession:
    """Sketch of how a cmd‑mox session might be used in tests.
    Replace this class with: from cmd_mox import MoxSession
    """
    def __init__(self, record=False, replay=None):
        self.record = record
        self.replay = replay
        self._shim = pathlib.Path(tempfile.mkdtemp(prefix="cmdmox-shim-"))
        self._ledger = []
    def stub(self, cmd, match=None, stdout="", stderr="", code=0):
        # Register a stubbed program behaviour; cmd-mox would create a shim here.
        return self
    def env(self):
        # Returns env updates, e.g., PATH pointing to the shim dir
        return {"PATH": f"{self._shim}:{os.environ['PATH']}"}
    def verify(self):
        # Assert expected calls, no unexpected calls, etc.
        return self


def run_act(job="selftest", event="tests/fixtures/pull_request.event.json", extra_env=None):
    artdir = tempfile.mkdtemp(prefix="act-artifacts-")
    cmd = [
        "act", "pull_request",
        "-j", job,
        "-e", event,
        "-P", "ubuntu-latest=catthehacker/ubuntu:act-latest",
        "--artifact-server-path", artdir,
        "-v",
    ]
    env = os.environ.copy()
    if extra_env:
        env.update(extra_env)
    cp = subprocess.run(cmd, capture_output=True, text=True, env=env)
    return cp.returncode, pathlib.Path(artdir), cp.stdout + "\n" + cp.stderr


def test_workflow_with_cmd_mox_replay(tmp_path):
    # Arrange: stub the gh call our workflow makes
    with MoxSession() as mox:
        mox.stub(
            "gh",
            match=["release", "view", "--json", "tagName"],
            stdout='{"tagName":"v9.9.9"}\n',
            code=0,
        )
        code, artdir, logs = run_act(extra_env=mox.env())
        # Assert: act succeeded
        assert code == 0, f"act failed:\n{logs}"
        # Assert: artifact exists and contains our stubbed value
        files = list(artdir.rglob("release-tag*/out.tag"))
        assert files, f"No artifact found. Logs:\n{logs}"
        assert files[0].read_text().strip() == "v9.9.9"
        # Verify: no unexpected external calls
        mox.verify()
        shutil.rmtree(artdir, ignore_errors=True)
```

Run it:

```bash
# Pull the runner image once (optional but speeds up first run)
act pull_request -P ubuntu-latest=catthehacker/ubuntu:act-latest --list

# Execute your integration test
pytest -q
```

## Record → replay → verify (closing the loop)

`cmd‑mox` is designed for **record–replay–verify**. A practical workflow:

1. **Record** a golden trace (once): run the test with real commands allowed,
   capturing I/O.
2. **Replay** deterministically in all local/CI runs.
3. **Verify** that the same set of external calls occurred in the expected order
   with the expected arguments.

Sketch:

```python
# RECORD
with MoxSession(record=True) as mox:
    code, _, logs = run_act(extra_env=mox.env())
    assert code == 0
    mox.verify()               # optional: ensure no unexpected calls
    mox.save("tests/goldens/gh_release_view.jsonl")

# REPLAY
with MoxSession(replay="tests/goldens/gh_release_view.jsonl") as mox:
    code, _, logs = run_act(extra_env=mox.env())
    assert code == 0
    mox.verify()
```

You can commit golden ledgers for stability, or regenerate them explicitly when
behaviour changes.

## What to assert (beyond exit code)

- **Artifacts:** contents, line endings, naming convention.
- **Logs:** presence or absence of critical lines (e.g., cache key, matrix
  values).
- **Side‑effects:** created files, modified config, version stamp.
- **External calls:** `cmd‑mox` ledger has only the calls you expect, in the
  order you expect.

## Known limitations

- **Runner parity:** `act` images differ from `ubuntu‑latest` toolcache. Validate
  logic, not performance.
- **Permissions/OIDC:** not faithfully reproduced locally; keep those tests on
  GH.
- **Services:** service containers work, but health‑checks and DNS timing can
  diverge.

## Suggested ladder

1. **Fast local loop:** unit tests → `act` + `pytest` + `cmd‑mox`.
2. **Authoritative CI:** same workflow on GitHub runners.
3. **End‑to‑end (privileged):** GH‑only, least‑privilege tokens, feature-gated.

This setup keeps your feedback loop tight, your external surfaces deterministic,
and your confidence proportional to reality.
