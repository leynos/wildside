"""Reads which commands a shell script actually executes.

Split from :mod:`ci_step_predicates` because the question is a different
one: that module says what a workflow step means, and this one says only
what a `run` block executes, with no knowledge of workflows at all.

It exists because a substring search cannot answer the question. The
first version of `runs_the_suite` matched `cargo test` anywhere in a
script, so `# cargo test`, `echo cargo test` and `grep "cargo test"`
each reported that the step ran the backend suite. That direction is the
dangerous one: a contract satisfied by a commented-out command stays
green while the real invocation is replaced by a diagnostic echo, which
is precisely the regression the lane contracts exist to catch.

So a command is read at its executable position. A script is split into
lines, each line is lexed with :mod:`shlex` so quoting and `#` comments
are honoured, and the words are cut at the shell operators that end one
command and begin another. What remains of each command is stripped of
leading environment assignments and transparent wrappers, and the first
word left is the executable.

The reverse error is safe here. A wrapper this module does not know
makes a real suite invocation read as no invocation at all, and the lane
contracts assert that *some* step runs the suite, so that direction
fails loudly rather than silently certifying an empty lane.

Nothing here opens a file and nothing here knows a workflow's shape.
"""

from __future__ import annotations

import re
import shlex
from pathlib import PurePosixPath

#: Shell tokens that end one command and begin another. `shlex` with
#: `punctuation_chars` yields each of these as a token of its own, so
#: they are recognized by equality rather than by searching the text.
COMMAND_SEPARATORS = frozenset({"&&", "||", "|", ";", ";;", "&", "(", ")"})

#: Commands that hand their remaining words to another command. Each is
#: transparent in the sense that matters here: `env cargo test` runs the
#: suite and `echo cargo test` does not, and the difference is that only
#: these pass the words on to be executed.
TRANSPARENT_WRAPPERS = frozenset({
    "command",
    "env",
    "exec",
    "nice",
    "sudo",
    "time",
    "timeout",
    "xvfb-run",
})

#: A leading `NAME=value` word, which sets a variable for one command
#: rather than naming one. `RUSTFLAGS="-D warnings" cargo nextest run`
#: is the form this workflow actually uses.
_ASSIGNMENT = re.compile(r"\A[A-Za-z_][A-Za-z0-9_]*=")

#: A bare number or duration, which is how the wrappers that take one
#: argument spell it: `timeout 60`, `timeout 5m`, `nice -n 5`.
_DURATION = re.compile(r"\A\d+(?:\.\d+)?[smhd]?\Z")


def _lex(line: str) -> list[str] | None:
    """Return one line's shell words, or None when it cannot be lexed.

    Parameters
    ----------
    line : str
        One physical line of a script.

    Returns
    -------
    list[str] or None
        The words and operator tokens, or None when the line has an
        unbalanced quote and names no command that can be read.
    """
    lexer = shlex.shlex(line, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    try:
        return list(lexer)
    except ValueError:
        return None


def _cut_at_operators(tokens: list[str]) -> list[list[str]]:
    """Return one word list per command, cut at the shell operators.

    Split out of :func:`command_words` rather than nested inside it. The
    loop over lines and the loop over one line's tokens answer different
    questions, and holding both at once was the nesting CodeScene flagged
    when this module was first written.

    Parameters
    ----------
    tokens : list[str]
        One line's words and operator tokens.

    Returns
    -------
    list[list[str]]
        The commands, each as its words, with empty runs between two
        adjacent operators dropped.
    """
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


def command_words(script: str) -> list[list[str]]:
    r"""Return each command a script executes, as its words.

    Backslash continuations are joined first, then each line is lexed
    and cut at :data:`COMMAND_SEPARATORS`. Comments and quoting are
    handled by the lexer, so a `#` line yields nothing and a quoted
    command name is one ordinary word rather than an invocation.

    Parameters
    ----------
    script : str
        A step's `run` value.

    Returns
    -------
    list[list[str]]
        One entry per command, in the order the script runs them.

    Examples
    --------
    >>> command_words("make lint && make test")
    [['make', 'lint'], ['make', 'test']]
    >>> command_words("# cargo test\nmake lint")
    [['make', 'lint']]
    >>> command_words('echo "cargo test"')
    [['echo', 'cargo test']]
    """
    commands: list[list[str]] = []
    for line in script.replace("\\\n", " ").splitlines():
        tokens = _lex(line)
        if tokens is not None:
            commands += _cut_at_operators(tokens)
    return commands


def _strip_wrapper(words: list[str]) -> list[str]:
    """Return the words after one wrapper and the arguments it consumes.

    Parameters
    ----------
    words : list[str]
        A command whose first word is a transparent wrapper.

    Returns
    -------
    list[str]
        The remaining words, with the wrapper's own options and any
        single duration argument removed.
    """
    remaining = words[1:]
    while remaining and (remaining[0].startswith("-") or _DURATION.match(remaining[0])):
        remaining = remaining[1:]
    return remaining


def executed_command(words: list[str]) -> list[str]:
    """Return the command a word list executes, wrappers removed.

    Leading environment assignments and transparent wrappers are peeled
    away in turn, because either can precede the other:
    `env RUSTFLAGS=-D cargo test` has both.

    Parameters
    ----------
    words : list[str]
        One command's words, as :func:`command_words` returns them.

    Returns
    -------
    list[str]
        The executable and its arguments, or an empty list when the
        words name no executable at all.

    Examples
    --------
    >>> executed_command(["RUSTFLAGS=-D warnings", "cargo", "test"])
    ['cargo', 'test']
    >>> executed_command(["timeout", "60", "make", "test"])
    ['make', 'test']
    >>> executed_command(["echo", "cargo", "test"])
    ['echo', 'cargo', 'test']
    >>> executed_command(["FOO=bar"])
    []
    """
    remaining = list(words)
    while remaining:
        while remaining and _ASSIGNMENT.match(remaining[0]):
            remaining = remaining[1:]
        if not remaining or remaining[0] not in TRANSPARENT_WRAPPERS:
            return remaining
        remaining = _strip_wrapper(remaining)
    return remaining


def executable_name(words: list[str]) -> str:
    """Return the base name of the executable a command names.

    A path-qualified spelling names the same program: `/usr/bin/cargo`
    and `cargo` are one executable, and a contract that knew only the
    bare word would miss the other.

    Parameters
    ----------
    words : list[str]
        One command's words, as :func:`command_words` returns them.

    Returns
    -------
    str
        The executable's base name, or an empty string when the words
        name no executable.

    Examples
    --------
    >>> executable_name(["/usr/bin/cargo", "test"])
    'cargo'
    >>> executable_name(["RUSTFLAGS=-D warnings", "cargo", "test"])
    'cargo'
    >>> executable_name([])
    ''
    """
    command = executed_command(words)
    return PurePosixPath(command[0]).name if command else ""


def option_value(command: list[str], option: str) -> str | None:
    """Return the value a command passes one option, or None.

    Both spellings are read, `--opt value` and `--opt=value`, because a
    contract that knew only one would report an option as absent when it
    had merely been rewritten.

    Parameters
    ----------
    command : list[str]
        One command's words.
    option : str
        The option, including its leading dashes.

    Returns
    -------
    str or None
        The value, or None when the option is absent or is the command's
        last word and so has no value after it.

    Examples
    --------
    >>> option_value(["cargo", "nextest", "run", "-E", "not binary(x)"], "-E")
    'not binary(x)'
    >>> option_value(["cargo", "nextest", "run", "-E=not binary(x)"], "-E")
    'not binary(x)'
    >>> option_value(["cargo", "nextest", "run"], "-E") is None
    True
    >>> option_value(["cargo", "nextest", "run", "-E"], "-E") is None
    True
    """
    joined = f"{option}="
    for index, word in enumerate(command):
        if word.startswith(joined):
            return word[len(joined) :]
        if word == option:
            return command[index + 1] if index + 1 < len(command) else None
    return None
