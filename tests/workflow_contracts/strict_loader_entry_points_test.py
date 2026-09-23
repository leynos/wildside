"""Every workflow reader refuses a repeated key, not only the inventory's.

`strict_yaml.load` replaced `yaml.safe_load` at each entry point that parses
a workflow. `codescene_closure_test` proves the refusal through
`workflow_inventory.load_workflow` alone, so reverting any other entry point
to `yaml.safe_load` would leave that reader keeping the last of two keys with
nothing noticing. Each case here feeds one reader a workflow whose discarded
half is a paid runner label, so the silent reading would pass every
placement contract, and requires the reader to refuse it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import coverage_main_workflow_test
import embedded_postgres_env_test
import mutation_testing_test
import nixie_toolchain_test
import pr_concurrency_support
import pytest
import repository_reading
import typed_documents
import yaml

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc
    from pathlib import Path

#: A workflow declaring `runs-on` twice. PyYAML would keep the hosted label.
DOUBLED = """\
"on": pull_request
jobs:
  build:
    runs-on: ubicloud-standard-8
    runs-on: ubuntu-latest
    steps: []
"""


@pytest.fixture
def doubled(tmp_path: Path) -> Path:
    """Write the doubled workflow and return its path."""
    path = tmp_path / "ci.yml"
    path.write_text(DOUBLED, encoding="utf-8")
    return path


@pytest.mark.parametrize(
    ("reader", "refusal"),
    [
        pytest.param(
            lambda path: repository_reading.parse_workflow(DOUBLED, path),
            repository_reading.RepositoryReadError,
            id="repository_reading.parse_workflow",
        ),
        pytest.param(
            pr_concurrency_support._parse,
            pr_concurrency_support.UnparsableWorkflowError,
            id="pr_concurrency_support._parse",
        ),
        pytest.param(
            lambda path: typed_documents.load_yaml_document(path, "ci.yml"),
            yaml.YAMLError,
            id="typed_documents.load_yaml_document",
        ),
        pytest.param(
            embedded_postgres_env_test._load,
            yaml.YAMLError,
            id="embedded_postgres_env_test._load",
        ),
    ],
)
def test_each_path_reader_refuses_a_repeated_key(
    doubled: Path, reader: cabc.Callable[[Path], object], refusal: type[Exception]
) -> None:
    """Refuse the doubled workflow at a reader that takes its path."""
    with pytest.raises(refusal):
        reader(doubled)


@pytest.mark.parametrize(
    ("module", "reader_name"),
    [
        pytest.param(mutation_testing_test, "_load", id="mutation_testing_test"),
        pytest.param(
            coverage_main_workflow_test, "_load_steps", id="coverage_main_workflow_test"
        ),
        pytest.param(nixie_toolchain_test, "_build_steps", id="nixie_toolchain_test"),
    ],
)
def test_each_fixed_path_reader_refuses_a_repeated_key(
    doubled: Path,
    module: typ.Any,  # noqa: ANN401 - a test module, read by attribute.
    reader_name: str,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Refuse the doubled workflow at a reader bound to one workflow path."""
    # `raising` stays at its default, so a renamed constant fails loudly.
    monkeypatch.setattr(module, "WORKFLOW_PATH", doubled)
    with pytest.raises(yaml.YAMLError):
        getattr(module, reader_name)()
