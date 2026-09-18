"""Reads which command a workflow `run:` line actually executes.

Split from `codescene_coverage_baseline_test.py` when that module crossed
the 400-line limit the Python lint gate enforces, on the seam the rest of
this suite uses: a reader module says what a thing is, and the contract
says what it must be.

It exists because taking the first word of a command is wrong in three
reachable ways. A `NAME=value` assignment, a transparent wrapper such as
`env`, and a path-qualified spelling each put something other than the
executable at position zero, and a contract that read position zero
would return False for all three. That is the direction that matters: a
rule satisfied by a spelling is worked around by choosing that spelling.

The peeling is deliberately shallow, and the limit is stated rather than
hidden. A workflow `run:` block is not an arbitrary shell script, so the
wrapper set is a short list of the forms a workflow plausibly writes
rather than a shell parser. An unrecognized wrapper leaves the command
unfound.

Nothing here opens a file and nothing knows what a workflow is.
"""

from __future__ import annotations

import re
from pathlib import PurePosixPath

#: A leading `NAME=value` word, which sets a variable for one command
#: rather than naming one.
_ASSIGNMENT = re.compile(r"\A[A-Za-z_][A-Za-z0-9_]*=")

#: Words that hand their remaining arguments to another command.
TRANSPARENT_WRAPPERS = frozenset({"command", "env", "exec", "sudo", "time"})


def executable_of(command: str) -> list[str]:
    """Return a command's words with assignments and wrappers peeled off.

    Parameters
    ----------
    command : str
        One logical shell command.

    Returns
    -------
    list[str]
        The executable and its arguments, or an empty list when the
        command names no executable.

    Examples
    --------
    >>> executable_of("TOKEN=x cs-coverage check")
    ['cs-coverage', 'check']
    >>> executable_of("env --chdir=/tmp cs-coverage check")
    ['cs-coverage', 'check']
    >>> executable_of("echo hello")
    ['echo', 'hello']
    >>> executable_of("FOO=bar")
    []
    """
    words = command.split()
    while words:
        while words and _ASSIGNMENT.match(words[0]):
            words = words[1:]
        if not words or words[0] not in TRANSPARENT_WRAPPERS:
            return words
        words = words[1:]
        while words and words[0].startswith("-"):
            words = words[1:]
    return words


def invokes(command: str, executable: str, subcommand: str | None = None) -> bool:
    """Return whether a command runs one executable, optionally with a subcommand.

    The executable is compared by base name, so a path-qualified
    spelling counts, and as a whole word, so a longer name that merely
    starts with it does not.

    Parameters
    ----------
    command : str
        One logical shell command.
    executable : str
        The program's base name.
    subcommand : str or None
        A first argument to require, or None to require none.

    Returns
    -------
    bool
        True when the command invokes it.

    Examples
    --------
    >>> invokes("cs-coverage check --format lcov", "cs-coverage")
    True
    >>> invokes("/opt/bin/cs-coverage check", "cs-coverage")
    True
    >>> invokes("cs coverage upload", "cs", "coverage")
    True
    >>> invokes("cs rules-config validate", "cs", "coverage")
    False
    >>> invokes("cat notes/cs-coverage-notes.md", "cs-coverage")
    False
    """
    words = executable_of(command)
    if not words or PurePosixPath(words[0]).name != executable:
        return False
    if subcommand is None:
        return True
    return len(words) > 1 and words[1] == subcommand
