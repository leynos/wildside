"""Exercises the one place this contract touches the filesystem.

The repository's own workflows are readable and valid, so nothing here
can be exercised against them: every assertion below needs a file that
cannot be opened, or one that is not YAML. Those are written into a
temporary directory instead.

What is being checked is the boundary's promise rather than YAML itself.
A failure has to name the file, because a contract spread over five
modules reports a missing workflow from wherever the value was finally
needed unless the reader says which path it wanted.
"""

from __future__ import annotations

import os
import typing as typ

import pytest
from repository_reading import (
    RepositoryReadError,
    parse_workflow,
    read_text,
    workflow_documents,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The smallest workflow with a job in it, used where the contents do
#: not matter and only the reading does.
MINIMAL_WORKFLOW: typ.Final[str] = "jobs:\n  build:\n    steps: []\n"


def test_a_file_that_is_not_there_names_the_path_it_wanted(
    tmp_path: Path,
) -> None:
    """A missing file is a fault about a path, and must say which.

    Scenario: the contract reads a configuration that does not exist.

    Invariant: the failure is a `RepositoryReadError` carrying the path,
    and the path appears in the message. A bare `OSError` from inside a
    fixture names the errno and leaves the reader to guess which of the
    contract's several files was wanted.
    """
    missing = tmp_path / "absent.toml"
    with pytest.raises(RepositoryReadError) as caught:
        read_text(missing)
    assert caught.value.path == missing, "the failure carries the path it wanted"
    assert str(missing) in str(caught.value), (
        "the message names the file, not only the rule that was broken"
    )


def test_a_workflow_that_is_not_yaml_names_the_file(tmp_path: Path) -> None:
    """A parse failure is a fault about a file, and must say which.

    Scenario: a workflow file holds text YAML cannot parse.

    Invariant: the failure is a `RepositoryReadError` carrying that
    path. `yaml.YAMLError` names a line and a column in a document it
    does not name, which is unhelpful when several workflows are read
    in one pass.
    """
    broken = tmp_path / "broken.yml"
    broken.write_text("jobs: [unclosed\n", encoding="utf-8")
    with pytest.raises(RepositoryReadError) as caught:
        parse_workflow(broken.read_text(encoding="utf-8"), broken)
    assert caught.value.path == broken, "the failure carries the file at fault"


def test_both_workflow_extensions_are_read(tmp_path: Path) -> None:
    """GitHub reads `.yml` and `.yaml`, so the contract must read both.

    Scenario: a directory holds one workflow under each extension.

    Invariant: both appear, keyed by file name. A coverage lane in the
    extension the contract did not read would escape every assertion
    without failing anything.
    """
    (tmp_path / "first.yml").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    (tmp_path / "second.yaml").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    assert sorted(workflow_documents(tmp_path)) == ["first.yml", "second.yaml"], (
        "both extensions are read, and each document is keyed by its file name"
    )


def test_a_workflow_that_declares_nothing_is_not_a_document(
    tmp_path: Path,
) -> None:
    """An empty file is not a workflow, and is not an error either.

    Scenario: a directory holds an empty file and a real workflow.

    Invariant: only the real one comes back. YAML decodes an empty file
    to None and a bare scalar to a string; neither has jobs, and raising
    on them would fail the contract over a file GitHub simply ignores.
    """
    (tmp_path / "empty.yml").write_text("", encoding="utf-8")
    (tmp_path / "real.yml").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    assert list(workflow_documents(tmp_path)) == ["real.yml"], (
        "a file with no mapping at its top level declares no jobs"
    )


@pytest.mark.skipif(
    os.geteuid() == 0,
    reason="root ignores the mode bits this case needs to make a file unreadable",
)
def test_an_unreadable_workflow_fails_the_whole_reading(tmp_path: Path) -> None:
    """A directory that cannot be read whole cannot be reasoned about.

    Scenario: a workflow file exists but cannot be opened.

    Invariant: `workflow_documents` raises rather than returning the
    files it managed to read. Silently skipping the unreadable one would
    let a coverage lane disappear from the contract, and the loss would
    look exactly like a repository that has one fewer lane.
    """
    unreadable = tmp_path / "locked.yml"
    unreadable.write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    unreadable.chmod(0o000)
    try:
        with pytest.raises(RepositoryReadError) as caught:
            workflow_documents(tmp_path)
    finally:
        unreadable.chmod(0o644)
    assert caught.value.path == unreadable, (
        "the failure names the file it could not open"
    )
