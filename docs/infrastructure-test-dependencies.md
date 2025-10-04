# Infrastructure test dependency checklist

The Go-based infrastructure test suites rely on external command-line tools.
When those binaries are absent the tests previously skipped silently, which
made it difficult to spot coverage gaps in CI. To address this we ship a small
pre-flight validator:

```bash
make check-test-deps
```

The target executes `scripts/check_test_dependencies.py` and fails early if the
required tools are missing. Run it locally before `make doks-test`,
`make fluxcd-test`, `make vault-appliance-test`, or `make dev-cluster-test` so
you know whether the environment is ready. CI pipelines should add the same
step before invoking the infrastructure test matrix.

## Required binaries

| Tool       | Purpose                                                         |
| ---------- | --------------------------------------------------------------- |
| `tofu`     | Executes plans for the Terraform modules exercised in tests.    |
| `conftest` | Evaluates Open Policy Agent assertions against generated plans. |

If the script reports a missing dependency, install it via your system package
manager or follow the instructions in the tool's official documentation. Once
installed, rerun `make check-test-deps` to confirm the environment is healthy.
