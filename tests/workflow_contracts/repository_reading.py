"""Reads the repository's own files, and nothing else.

Every other module in this contract is pure: it takes text or parsed
documents and returns budgets. The filesystem lives here, at one named
boundary, so a file that cannot be opened or cannot be parsed fails
where the path is still in hand rather than several frames inside a
budget derivation, where the traceback names a key and not a file.

Both failures become :class:`RepositoryReadError`, which carries the
path. `OSError` alone says "No such file or directory" without saying
which contract wanted it, and `yaml.YAMLError` names a line in a
document it does not name.

The readers take their path or directory as an argument. A reader that
reached for a module-level constant would make every caller's
filesystem access invisible at the call site, which is the thing this
module exists to prevent.
"""

from __future__ import annotations

import typing as typ
from fnmatch import fnmatch

import yaml
from lane_fields import Node, mapping_of

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: The workflow file extensions GitHub reads. Both are searched: a
#: coverage lane in the one this contract did not read would escape
#: every assertion without failing anything.
WORKFLOW_PATTERNS: typ.Final[tuple[str, ...]] = ("*.yml", "*.yaml")


class RepositoryReadError(ValueError):
    """Raised when a repository file cannot be read or parsed.

    Attributes
    ----------
    path : Path
        The file at fault, so the message names what an author has to
        open rather than only the rule that was broken.
    """

    def __init__(self, message: str, *, path: Path) -> None:
        """Record the failing path alongside the message.

        Parameters
        ----------
        message : str
            The human-readable explanation.
        path : Path
            The file at fault.
        """
        super().__init__(message)
        self.path = path


def read_text(path: Path) -> str:
    """Return one repository file's text.

    Parameters
    ----------
    path : Path
        The file to read.

    Returns
    -------
    str
        The file's contents, decoded as UTF-8.

    Raises
    ------
    RepositoryReadError
        If the file cannot be opened, or is not valid UTF-8.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as name:
    ...     path = Path(name) / "ci.yml"
    ...     _ = path.write_text("jobs: {build: {steps: []}}", encoding="utf-8")
    ...     read_text(path)
    'jobs: {build: {steps: []}}'
    """
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        message = f"{path}: cannot be read: {error}"
        raise RepositoryReadError(message, path=path) from error


def parse_workflow(text: str, path: Path) -> Node | None:
    """Return one workflow document, or None when it declares nothing.

    Parameters
    ----------
    text : str
        The workflow file's text.
    path : Path
        The file it came from, for the message.

    Returns
    -------
    Node or None
        The parsed document, or None when the file is empty or its top
        level is not a mapping.

    Raises
    ------
    RepositoryReadError
        If the text is not valid YAML.

    Examples
    --------
    >>> from pathlib import Path
    >>> parse_workflow("jobs: {build: {steps: []}}", Path("ci.yml"))
    {'jobs': {'build': {'steps': []}}}

    A file whose top level is not a mapping declares nothing:

    >>> parse_workflow("", Path("empty.yml")) is None
    True
    """
    try:
        parsed = yaml.safe_load(text)
    except yaml.YAMLError as error:
        message = f"{path}: is not valid YAML: {error}"
        raise RepositoryReadError(message, path=path) from error
    return mapping_of(parsed)


def workflow_documents(directory: Path) -> dict[str, Node]:
    """Return every workflow document in one directory, keyed by name.

    Parameters
    ----------
    directory : Path
        The directory to read, ordinarily ``.github/workflows``.

    Returns
    -------
    dict[str, Node]
        File name to parsed document, in file-name order.

    Raises
    ------
    RepositoryReadError
        If the directory cannot be listed, if a file cannot be read, or
        if one is not valid YAML.

    Examples
    --------
    Only the workflow extensions are read, so a neighbouring file is
    left where it is:

    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as name:
    ...     directory = Path(name)
    ...     workflow = directory / "ci.yml"
    ...     _ = workflow.write_text("jobs: {build: {steps: []}}", encoding="utf-8")
    ...     _ = (directory / "notes.md").write_text("ignored", encoding="utf-8")
    ...     workflow_documents(directory)
    {'ci.yml': {'jobs': {'build': {'steps': []}}}}
    """
    documents: dict[str, Node] = {}
    for path in _workflow_paths(directory):
        parsed = parse_workflow(read_text(path), path)
        if parsed is not None:
            documents[path.name] = parsed
    return documents


def _workflow_paths(directory: Path) -> cabc.Sequence[Path]:
    """Return every workflow file in one directory, in name order.

    The directory is enumerated with ``iterdir`` rather than matched
    with ``glob``. ``Path.glob`` reports a directory that is missing,
    that is not a directory, or that cannot be listed as an empty
    result, and an empty result is indistinguishable from a repository
    that declares no workflows. A contract whose every assertion is over
    the lanes it found would then pass on a repository it never read,
    which is the one outcome the boundary exists to prevent.

    Parameters
    ----------
    directory : Path
        The directory to search.

    Returns
    -------
    cabc.Sequence[Path]
        The matching paths, sorted by name so the reading is the same
        whatever order the filesystem offers them in.

    Raises
    ------
    RepositoryReadError
        If the directory cannot be listed, carrying the directory's own
        path rather than a file's.
    """
    try:
        entries = list(directory.iterdir())
    except OSError as error:
        message = f"{directory}: cannot be listed: {error}"
        raise RepositoryReadError(message, path=directory) from error
    found = [
        entry
        for entry in entries
        if any(fnmatch(entry.name, pattern) for pattern in WORKFLOW_PATTERNS)
    ]
    return sorted(found, key=lambda path: path.name)
