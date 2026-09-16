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

import contextlib
import os
import stat
import typing as typ

import pytest
from repository_reading import (
    RepositoryReadError,
    parse_workflow,
    read_text,
    workflow_documents,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: The smallest workflow with a job in it, used where the contents do
#: not matter and only the reading does.
MINIMAL_WORKFLOW: typ.Final[str] = "jobs:\n  build:\n    steps: []\n"

#: Raised inside the helper to check it restores on the way out.
RAISED_THROUGH: typ.Final[str] = "raised through the permission helper"


@contextlib.contextmanager
def _permissions_denied(target: Path) -> cabc.Iterator[None]:
    """Withhold every permission on ``target`` for the body's duration.

    The mode to restore is read from the target rather than written
    down. A literal ``0o644`` or ``0o755`` is a mode the process umask
    may never have produced, so restoring one would leave the file in a
    state the test did not find it in, and the temporary directory's own
    clean-up would be the thing that noticed.

    ``stat.S_IMODE`` masks off the file-type bits that ``st_mode``
    carries. No test can observe it: Linux ignores those bits in
    ``chmod``, so passing ``st_mode`` whole has the same effect here. It
    is what ``chmod`` documents as its argument, and that is the reason
    it is written, not a guard the suite holds up.
    """
    original = stat.S_IMODE(target.stat().st_mode)
    target.chmod(0o000)
    try:
        yield
    finally:
        target.chmod(original)


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

    Scenario: a directory holds one workflow under each extension, and
    a neighbouring file that is not a workflow but does parse as a
    mapping.

    Invariant: both workflows appear, keyed by file name, and the
    neighbour does not. A coverage lane in the extension the contract
    did not read would escape every assertion without failing anything;
    a reading that took every entry in the directory would report a
    document GitHub never runs, and the narrowness is not observable
    unless the neighbour would otherwise have been returned.
    """
    (tmp_path / "first.yml").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    (tmp_path / "second.yaml").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    (tmp_path / "notes.md").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    assert sorted(workflow_documents(tmp_path)) == ["first.yml", "second.yaml"], (
        "both extensions are read, and nothing else in the directory is"
    )


def test_a_workflow_that_declares_nothing_is_not_a_document(
    tmp_path: Path,
) -> None:
    """An empty file is not a workflow, and is not an error either.

    Scenario: a directory holds an empty file, a file whose top level is
    a bare scalar, a file whose top level is a sequence, and a real
    workflow.

    Invariant: only the real one comes back. YAML decodes an empty file
    to None, a bare scalar to a string and a sequence to a list; none of
    them has jobs, and raising on them would fail the contract over a
    file GitHub simply ignores. The three are written rather than
    described, because only the empty one decodes to None and a reading
    that dropped the mapping check would return the other two.
    """
    (tmp_path / "empty.yml").write_text("", encoding="utf-8")
    (tmp_path / "scalar.yml").write_text("a bare scalar\n", encoding="utf-8")
    (tmp_path / "sequence.yml").write_text("- one\n- two\n", encoding="utf-8")
    (tmp_path / "real.yml").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    assert list(workflow_documents(tmp_path)) == ["real.yml"], (
        "a file with no mapping at its top level declares no jobs"
    )


#: A mode no test in this file would think to write down, so a helper
#: that restored a literal cannot accidentally restore this one.
UNUSUAL_MODE: typ.Final[int] = 0o640


def test_the_permission_helper_gives_back_the_mode_it_found(tmp_path: Path) -> None:
    """Permissions are withheld for the body and no longer.

    Scenario: a file whose mode is not one either permission case would
    have written down, taken through the helper twice, once leaving
    cleanly and once with an exception raised through it.

    Invariant: the mode afterwards is the mode before, both times, and
    nothing is permitted inside. Nothing else in this file can notice a
    failed restore, because by then the reading has already raised and
    the temporary directory is about to be discarded, so the helper is
    asserted on directly or not at all.
    """
    target = tmp_path / "ci.yml"
    target.write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    target.chmod(UNUSUAL_MODE)
    before = stat.S_IMODE(target.stat().st_mode)

    with _permissions_denied(target):
        assert stat.S_IMODE(target.stat().st_mode) == 0, (
            "the body runs with every permission withheld"
        )

    assert stat.S_IMODE(target.stat().st_mode) == before, (
        "a clean exit gives back the mode the helper found"
    )

    with pytest.raises(RuntimeError), _permissions_denied(target):
        raise RuntimeError(RAISED_THROUGH)

    assert stat.S_IMODE(target.stat().st_mode) == before, (
        "an exception carried through the body gives it back too"
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
    with (
        _permissions_denied(unreadable),
        pytest.raises(RepositoryReadError) as caught,
    ):
        workflow_documents(tmp_path)

    assert caught.value.path == unreadable, (
        "the failure names the file it could not open"
    )


def test_a_workflow_directory_that_is_not_there_is_not_an_empty_one(
    tmp_path: Path,
) -> None:
    """A directory that cannot be listed is a fault, not a count of zero.

    Scenario: the contract reads a workflow directory that does not
    exist.

    Invariant: `workflow_documents` raises and names the directory. This
    is the failure with no symptom: every assertion in the timeout
    contract is over the lanes the reading found, so a reading that
    returned nothing would satisfy all of them and report a repository
    whose workflows were never opened as one in perfect order.
    """
    absent = tmp_path / "workflows"

    with pytest.raises(RepositoryReadError) as caught:
        workflow_documents(absent)

    assert caught.value.path == absent, (
        "the failure names the directory it could not list, not a file"
    )


def test_a_workflow_directory_that_is_a_file_is_refused(tmp_path: Path) -> None:
    """A path that is not a directory cannot hold workflows.

    Scenario: the path given to the reading is an ordinary file.

    Invariant: `workflow_documents` raises and names it. The mistake is
    an easy one, a caller passing the workflow rather than the directory
    holding it, and its quiet form reports every lane as absent.
    """
    not_a_directory = tmp_path / "ci.yml"
    not_a_directory.write_text(MINIMAL_WORKFLOW, encoding="utf-8")

    with pytest.raises(RepositoryReadError) as caught:
        workflow_documents(not_a_directory)

    assert caught.value.path == not_a_directory, (
        "the failure names the path that was not a directory"
    )


@pytest.mark.skipif(
    os.geteuid() == 0,
    reason="root ignores the mode bits this case needs to make a directory unlistable",
)
def test_a_workflow_directory_that_cannot_be_listed_is_refused(
    tmp_path: Path,
) -> None:
    """Permission to read the directory is part of reading it.

    Scenario: the workflow directory exists and holds a workflow, but
    its own mode bits forbid listing it.

    Invariant: `workflow_documents` raises and names the directory. The
    file inside is readable, so a reading that swallowed the enumeration
    failure would report no lanes while the lane sat there on disk.
    """
    locked = tmp_path / "workflows"
    locked.mkdir()
    (locked / "ci.yml").write_text(MINIMAL_WORKFLOW, encoding="utf-8")
    with (
        _permissions_denied(locked),
        pytest.raises(RepositoryReadError) as caught,
    ):
        workflow_documents(locked)

    assert caught.value.path == locked, (
        "the failure names the directory whose listing was refused"
    )
