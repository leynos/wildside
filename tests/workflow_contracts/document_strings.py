"""Finds every string in a parsed document, at any depth.

Split from `codescene_coverage_baseline_test.py` so the walk can be
generated against rather than only exercised through the two workflows
this repository happens to carry. The contract there asks whether a
credential appears anywhere in a pull-request workflow, and the answer is
only as good as the walk: a reader that missed one shape would report a
clean workflow and the contract would pass for the wrong reason.

Keys are yielded as well as values, deliberately. A credential reaches a
step three ways, as an `env:` key, as a `${{ secrets.X }}` value and as
an action input, and a walk that returned values alone would find two of
them. Every string is yielded in the order it is met and duplicates are
kept, because a caller counting occurrences is asking a different
question from one testing membership and the walk should not decide
which.

Nothing here opens a file and nothing knows what a workflow is.
"""

from __future__ import annotations

import typing as typ

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc


def strings_in(node: object) -> cabc.Iterator[str]:
    """Yield every string in a parsed document, keys and values alike.

    Anything that is neither a string, a mapping nor a sequence yields
    nothing: an integer timeout and a boolean flag are not text and a
    caller searching for a name should not have to filter them out.
    Strings are not descended into, so a six-character name yields one
    string rather than six.

    Parameters
    ----------
    node : object
        Any part of a parsed document.

    Yields
    ------
    str
        Each string found, in the order met, duplicates kept.

    Examples
    --------
    >>> list(strings_in({"env": {"TOKEN": "${{ secrets.X }}"}}))
    ['env', 'TOKEN', '${{ secrets.X }}']
    >>> list(strings_in([1, True, None, "tail"]))
    ['tail']
    >>> list(strings_in({1: "keyed by an integer"}))
    ['keyed by an integer']
    >>> list(strings_in("bare"))
    ['bare']
    """
    # The cast in the mapping arm says only what the pattern established,
    # that this is a mapping whose keys and values are unconstrained: the
    # narrowed type is an unparameterized dict, which the type checker will
    # not accept against an invariant key type. `_mapping_strings` inspects
    # every key before using it.
    match node:
        case str() as text:
            yield text
        case dict() as mapping:
            yield from _mapping_strings(
                typ.cast("cabc.Mapping[object, object]", mapping)
            )
        case list() as items:
            for item in items:
                yield from strings_in(item)
        case _:
            return


def _mapping_strings(mapping: cabc.Mapping[object, object]) -> cabc.Iterator[str]:
    """Yield every string in one mapping, its keys included.

    Split out of :func:`strings_in` rather than nested inside it. Walking
    a mapping needs both halves of each entry and walking a sequence
    needs neither, and holding both shapes in one body was the nesting
    CodeScene flagged when this was first written.

    A non-string key is skipped rather than coerced. YAML 1.1 parses a
    bare `on` as the boolean `True`, so a document really does carry
    non-string keys, and `str(True)` would put the word "True" into a
    search for a credential name.

    Parameters
    ----------
    mapping : collections.abc.Mapping[object, object]
        Any mapping from a parsed document.

    Yields
    ------
    str
        Each string key, and each string anywhere in each value.
    """
    for key, value in mapping.items():
        if isinstance(key, str):
            yield key
        yield from strings_in(value)
