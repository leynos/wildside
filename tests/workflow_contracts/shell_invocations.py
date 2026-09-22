"""Reads which commands a workflow `run:` line actually executes.

Split from `codescene_coverage_baseline_test.py` when that module crossed
the 400-line limit the Python lint gate enforces, on the seam the rest of
this suite uses: a reader module says what a thing is, and the contract
says what it must be.

It exists because taking the first word of a line is wrong in four
reachable ways, and every one of them is a way to run a forbidden command
while the contract reports that nothing ran:

- a `NAME=value` assignment stands before the executable;
- a transparent wrapper such as `env` or `sudo` stands before it, and
  some wrapper options take an operand, so `sudo -u runner cs-coverage`
  hides the executable two words further along than a naive skip finds;
- the executable is path-qualified;
- the line is a shell list, and the forbidden command is not first:
  `echo preparing && cs-coverage check` and `true; cs coverage` both run
  it.

That last one is the reason the line is split before anything else is
asked of it. A rule satisfied by a spelling is worked around by choosing
that spelling, so each of these is a hole in the rule rather than an
untidiness.

The peeling is deliberately shallow and the limit is stated rather than
hidden. A workflow `run:` block is not an arbitrary shell script, so the
wrapper set is a short list of the forms a workflow plausibly writes,
with the arity of the options each one takes. An unrecognized wrapper
leaves the command unfound.

Nothing here opens a file and nothing knows what a workflow is.
"""

from __future__ import annotations

import re
import shlex
from pathlib import PurePosixPath

#: A leading `NAME=value` word, which sets a variable for one command
#: rather than naming one.
_ASSIGNMENT = re.compile(r"\A[A-Za-z_][A-Za-z0-9_]*=")

#: Shell tokens that end one command and begin another. `shlex` with
#: `punctuation_chars` emits each as a token of its own, so they are
#: recognized by equality rather than by searching the text, and one
#: inside quotes is not mistaken for a separator.
COMMAND_SEPARATORS = frozenset({"&&", "||", "|", ";", ";;", "&", "(", ")"})

#: Words that hand their remaining arguments to another command, each
#: with the options that take an operand. The operand has to be consumed
#: or it is read as the executable: `sudo -u runner cs-coverage check`
#: runs `cs-coverage`, and a reader that skipped only `-u` would report
#: `runner`.
WRAPPER_OPTION_OPERANDS: dict[str, frozenset[str]] = {
    "command": frozenset(),
    "env": frozenset({"-u", "-C", "-S", "--unset", "--chdir", "--split-string"}),
    "exec": frozenset({"-a"}),
    "sudo": frozenset({"-u", "-g", "-U", "-p", "-C", "-r", "-t"}),
    "time": frozenset({"-f", "-o", "--format", "--output"}),
}


def commands_in(line: str) -> list[list[str]]:
    """Return each command a shell line runs, as its words.

    Parameters
    ----------
    line : str
        One logical shell line, comments already removed and
        continuations already joined by the caller.

    Returns
    -------
    list[list[str]]
        One entry per command, in the order the line runs them, or an
        empty list when the line cannot be lexed.

    Examples
    --------
    >>> commands_in("echo preparing && cs-coverage check")
    [['echo', 'preparing'], ['cs-coverage', 'check']]
    >>> commands_in("true; cs coverage")
    [['true'], ['cs', 'coverage']]
    >>> commands_in('echo "a && b"')
    [['echo', 'a && b']]
    >>> commands_in("echo 'unbalanced")
    []
    """
    lexer = shlex.shlex(line, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    try:
        tokens = list(lexer)
    except ValueError:
        return []
    commands: list[list[str]] = []
    current: list[str] = []
    for token in tokens:
        if token not in COMMAND_SEPARATORS:
            current.append(token)
            continue
        if current:
            commands.append(current)
        current = []
    if current:
        commands.append(current)
    return commands


def _peel_wrapper(words: list[str]) -> list[str]:
    """Return the words after one wrapper and the options it consumes.

    Parameters
    ----------
    words : list[str]
        A command whose first word is a known wrapper.

    Returns
    -------
    list[str]
        The remaining words.
    """
    takes_operand = WRAPPER_OPTION_OPERANDS[words[0]]
    remaining = words[1:]
    while remaining and remaining[0].startswith("-"):
        option = remaining[0]
        remaining = remaining[1:]
        # `--opt=value` carries its operand; `--opt value` does not.
        if option in takes_operand and remaining:
            remaining = remaining[1:]
    return remaining


def executable_of(words: list[str]) -> list[str]:
    """Return one command's words with assignments and wrappers peeled off.

    Parameters
    ----------
    words : list[str]
        One command's words, as :func:`commands_in` returns them.

    Returns
    -------
    list[str]
        The executable and its arguments, or an empty list when the
        command names no executable.

    Examples
    --------
    >>> executable_of(["TOKEN=x", "cs-coverage", "check"])
    ['cs-coverage', 'check']
    >>> executable_of(["sudo", "-u", "runner", "cs-coverage", "check"])
    ['cs-coverage', 'check']
    >>> executable_of(["env", "-u", "HOME", "cs-coverage", "check"])
    ['cs-coverage', 'check']
    >>> executable_of(["env", "--chdir=/tmp", "cs-coverage"])
    ['cs-coverage']
    >>> executable_of(["echo", "hello"])
    ['echo', 'hello']
    >>> executable_of(["FOO=bar"])
    []
    """
    remaining = list(words)
    while remaining:
        while remaining and _ASSIGNMENT.match(remaining[0]):
            remaining = remaining[1:]
        if not remaining or remaining[0] not in WRAPPER_OPTION_OPERANDS:
            return remaining
        remaining = _peel_wrapper(remaining)
    return remaining


def invokes(line: str, executable: str, subcommand: str | None = None) -> bool:
    """Return whether a shell line runs one executable anywhere in it.

    The line is split into commands first, so a forbidden command that is
    not the first in a `&&` or `;` list is still found. The executable is
    then compared by base name, so a path-qualified spelling counts, and
    as a whole word, so a longer name that merely starts with it does
    not.

    Parameters
    ----------
    line : str
        One logical shell line.
    executable : str
        The program's base name.
    subcommand : str or None
        A first argument to require, or None to require none.

    Returns
    -------
    bool
        True when some command on the line invokes it.

    Examples
    --------
    >>> invokes("cs-coverage check --format lcov", "cs-coverage")
    True
    >>> invokes("echo preparing && cs-coverage check", "cs-coverage")
    True
    >>> invokes("true; cs coverage upload", "cs", "coverage")
    True
    >>> invokes("sudo -u runner cs-coverage check", "cs-coverage")
    True
    >>> invokes("/opt/bin/cs-coverage check", "cs-coverage")
    True
    >>> invokes("cs rules-config validate", "cs", "coverage")
    False
    >>> invokes("cat notes/cs-coverage-notes.md", "cs-coverage")
    False
    >>> invokes('echo "cs-coverage check"', "cs-coverage")
    False
    """
    return any(
        _is_invocation(executable_of(words), executable, subcommand)
        for words in commands_in(line)
    )


def _is_invocation(command: list[str], executable: str, subcommand: str | None) -> bool:
    """Return whether one executed command is the invocation sought.

    Parameters
    ----------
    command : list[str]
        One command's words, wrappers already peeled.
    executable : str
        The program's base name.
    subcommand : str or None
        A first argument to require, or None to require none.

    Returns
    -------
    bool
        True when the command invokes it.
    """
    if not command or PurePosixPath(command[0]).name != executable:
        return False
    if subcommand is None:
        return True
    return len(command) > 1 and command[1] == subcommand
