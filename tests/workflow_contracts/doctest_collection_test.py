"""Every Python example in this repository is executed by a gate.

A docstring example is documentation that claims a result, and an
unexecuted one is the only kind of documentation that can be wrong
without anything noticing. Before `--doctest-modules` was added, all
forty-three example lines here were unexecuted, and five of them were
wrong: two stated a double-quoted `repr` that Python has never produced,
one continued from a skipped line and raised `NameError`, one
over-escaped its input so the function returned an empty list, and one
shelled out to `kubectl` on whatever host ran it.

Collection is the part that rots. The `test-scripts` target cannot name
`scripts` as a whole, because `scripts/local_k8s.py` and
`scripts/local_k8s/` share a name and the `*_test.py` files beside them
belong to other targets, so it names the package and each helper script
that carries examples. A list like that goes stale silently: the next
script with an example is simply not collected, and a gate that examines
nothing reports success.

So this contract compares the list against the files. It reads the
Makefile rather than restating the paths, because a contract that
restated them would agree with itself after someone edited the recipe.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import pytest

REPO_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
MAKEFILE: typ.Final[Path] = REPO_ROOT / "Makefile"

#: The directories searched for examples. `tests/workflow_contracts` is
#: collected wholesale by its own target, so only `scripts` needs a list.
EXAMPLE_ROOTS: typ.Final[tuple[str, ...]] = ("scripts", "tests")

#: Directories that hold no source of this repository's own.
EXCLUDED_PARTS: typ.Final[frozenset[str]] = frozenset({
    ".venv",
    "__pycache__",
    "node_modules",
    "third_party",
})

#: A doctest prompt at the start of a line, allowing for indentation.
PROMPT: typ.Final[re.Pattern[str]] = re.compile(r"^\s*>>> ", re.MULTILINE)

#: The recipes that must collect doctests, and the variable or literal
#: each one passes as its paths.
COLLECTING_TARGETS: typ.Final[tuple[str, ...]] = (
    "test-workflow-contracts",
    "test-scripts",
)


def _recipe(target: str) -> str:
    """Return one Makefile recipe, with its continuations joined.

    Parameters
    ----------
    target : str
        The target whose recipe is wanted.

    Returns
    -------
    str
        The recipe body as one line.

    Raises
    ------
    AssertionError
        If the Makefile declares no such target.
    """
    text = MAKEFILE.read_text(encoding="utf-8").replace("\\\n", " ")
    for line in text.splitlines():
        if line.startswith(f"{target}:"):
            start = text.index(line) + len(line)
            body = text[start:]
            return body.split("\n\n", 1)[0]
    message = f"the Makefile declares no {target} target"
    raise AssertionError(message)


def _variable(name: str) -> list[str]:
    """Return a Makefile variable's value, split into words.

    Parameters
    ----------
    name : str
        The variable to read.

    Returns
    -------
    list[str]
        Its words, with continuations joined.

    Raises
    ------
    AssertionError
        If the Makefile defines no such variable.
    """
    text = MAKEFILE.read_text(encoding="utf-8").replace("\\\n", " ")
    for line in text.splitlines():
        if line.startswith(f"{name} ="):
            return line.split("=", 1)[1].split()
    message = f"the Makefile defines no {name} variable"
    raise AssertionError(message)


def _files_with_examples() -> list[Path]:
    """Return every Python file carrying a doctest prompt.

    Returns
    -------
    list[Path]
        Paths relative to the repository root, in sorted order.
    """
    found: list[Path] = []
    for root in EXAMPLE_ROOTS:
        for path in sorted((REPO_ROOT / root).rglob("*.py")):
            if EXCLUDED_PARTS.intersection(path.parts):
                continue
            if PROMPT.search(path.read_text(encoding="utf-8")):
                found.append(path.relative_to(REPO_ROOT))
    return found


def _collected_paths() -> list[Path]:
    """Return every path the doctest-collecting targets examine.

    Returns
    -------
    list[Path]
        Paths relative to the repository root.
    """
    paths = [Path("tests/workflow_contracts")]
    paths.extend(Path(word) for word in _variable("PY_DOCTEST_PATHS"))
    return paths


@pytest.mark.parametrize("target", COLLECTING_TARGETS)
def test_the_target_asks_pytest_for_doctests(target: str) -> None:
    """A pytest run collects doctests only when told to.

    Scenario: one of the two targets that run pytest over modules
    carrying examples.

    Invariant: its recipe passes `--doctest-modules`. Without the flag
    the target still passes, still reports a healthy count, and executes
    no example at all, which is indistinguishable from success.
    """
    assert "--doctest-modules" in _recipe(target), (
        f"{target} runs pytest without --doctest-modules, so every example "
        f"it covers is documentation nothing executes"
    )


def test_every_file_with_an_example_is_collected() -> None:
    """A named collection list must name every file that needs it.

    Scenario: the helper scripts and contract modules that carry `>>>`
    examples, against the paths the two targets actually examine.

    Invariant: each such file lies under a collected path. The list in
    `PY_DOCTEST_PATHS` cannot cover `scripts` wholesale, so a new script
    with examples would otherwise go uncollected, and nothing about a
    green run would look different.
    """
    collected = _collected_paths()
    uncovered = [
        path
        for path in _files_with_examples()
        if not any(path == root or root in path.parents for root in collected)
    ]
    assert not uncovered, (
        f"these files carry doctest examples that no target collects: "
        f"{[str(path) for path in uncovered]}; add them to PY_DOCTEST_PATHS"
    )


def test_the_repository_still_has_examples_to_execute() -> None:
    """A contract over an empty set passes while proving nothing.

    Scenario: the search for example-bearing files itself.

    Invariant: it finds some. If a refactor moved every example, or the
    prompt pattern stopped matching, the assertion above would pass over
    an empty list and this gate would quietly stop meaning anything.
    """
    assert _files_with_examples(), (
        "no Python file carries a doctest example; either they have all gone "
        "or this contract has stopped recognizing them"
    )
