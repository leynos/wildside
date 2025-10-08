# Infrastructure test dependency checklist

The Go-based infrastructure test suites rely on external command-line tools.
When those binaries are absent, or when incompatible versions are installed,
tests can skip silently and reduce coverage. To address this requirement the
repository provides a small pre-flight validator:

```bash
make check-test-deps
```

The target executes `scripts/check_test_dependencies.py` and fails early when
required tools are missing or below the minimum supported version. Operators
should run it locally before `make doks-test`, `make fluxcd-test`,
`make vault-appliance-test`, or `make dev-cluster-test` to confirm that the
environment is ready. Continuous Integration (CI) pipelines should add the
same step before invoking the infrastructure test matrix.

## Required binaries

Regenerate the table below with:

```bash
./scripts/check_test_dependencies.py --emit-markdown
```

| Tool | Minimum version | Purpose |
| ---- | ---------------- | ------- |
| `tofu` | 1.7.0 | OpenTofu CLI for Terraform plan execution |
| `conftest` | 0.45.0 | Policy testing via Open Policy Agent |

If the script reports a missing dependency, install the binary via the system
package manager or follow the instructions in the tool's official
documentation. After installation rerun `make check-test-deps` to confirm that
the environment satisfies the version constraints.
