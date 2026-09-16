"""Property tests for the boundary parser's recursive decode.

:mod:`typed_documents` exists so that a decoded document stops being
``Any`` at one place. ``_decoded`` is that place: it walks a freshly
decoded tree, validates every key, and hands back a ``JsonValue`` the
type checker can follow. The example-based contracts read the files this
repository happens to carry today, so none of them reaches a document
nested more than two deep, and none of them carries a key the loader
should refuse.

These properties pin the walk's rule instead, independently of any file:
a JSON-compatible tree survives it unchanged at every depth, a YAML
boolean key is restored to its source spelling, and anything outside
that set is refused rather than dropped. The last is the one that
matters most, because a parser that quietly discarded part of a document
would report on something other than the file on disk.
"""

from __future__ import annotations

import typing as typ
from itertools import starmap

import pytest
import typed_documents as docs
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc

#: Keys a workflow or manifest plausibly carries. Restricted to a small
#: alphabet so a counter-example reads as a key rather than as noise.
_KEY_CHARS = "abcdefghijklmnopqrstuvwxyz-_."

_KEYS = st.text(alphabet=_KEY_CHARS, min_size=1, max_size=6)

#: The scalars JSON and YAML share. ``nan`` and the infinities are left
#: out because they compare unequal to themselves, which would make the
#: round-trip property fail on a value the parser handled correctly.
_SCALARS = st.one_of(
    st.none(),
    st.booleans(),
    st.integers(min_value=-1_000_000, max_value=1_000_000),
    st.floats(allow_nan=False, allow_infinity=False),
    st.text(max_size=8),
)

#: Bounded so the search spends its budget on shape rather than on size.
_DOCUMENTS = st.recursive(
    _SCALARS,
    lambda children: st.one_of(
        st.lists(children, max_size=4),
        st.dictionaries(_KEYS, children, max_size=4),
    ),
    max_leaves=12,
)

#: Keys neither a string nor a boolean. YAML cannot produce most of
#: these, but JSON manifests are decoded by the same walk and a document
#: assembled by hand in a test can carry any of them.
_BAD_KEYS = st.one_of(
    st.integers(min_value=-100, max_value=100),
    st.floats(allow_nan=False, allow_infinity=False),
    st.none(),
    st.tuples(st.integers(min_value=0, max_value=3)),
)

#: Values outside the JSON-compatible set. A set and a tuple are the two
#: a helper most easily writes by accident; ``object()`` stands for
#: everything else.
_BAD_VALUES = st.sampled_from([{1, 2}, (1, 2), object(), b"bytes"])


def _identical(left: object, right: object) -> bool:
    """Return whether two decoded trees match in both shape and type.

    Equality alone is too weak here: ``True == 1`` and ``0 == 0.0``, so a
    walk that turned a boolean into an integer would satisfy ``==`` while
    changing what the document says. Mappings are compared by their key
    lists and then value by value, in iteration order, so a walk that
    reordered a document is caught as well as one that changed it.
    """
    if type(left) is not type(right):
        return False
    if isinstance(left, list) and isinstance(right, list):
        return len(left) == len(right) and _identical_pairs(
            zip(left, right, strict=True)
        )
    if isinstance(left, dict) and isinstance(right, dict):
        return list(left) == list(right) and _identical_pairs(
            zip(left.values(), right.values(), strict=True)
        )
    return left == right


def _identical_pairs(pairs: cabc.Iterable[tuple[object, object]]) -> bool:
    """Return whether every pair matches under :func:`_identical`."""
    return all(starmap(_identical, pairs))


@given(document=_DOCUMENTS)
def test_a_json_compatible_tree_survives_the_walk_unchanged(document: object) -> None:
    """The walk validates; it does not transform.

    Scenario: an arbitrary tree of the scalars, lists and string-keyed
    mappings that JSON and YAML share, nested to any depth the strategy
    reaches.

    Invariant: what comes back matches what went in, in type as well as
    value, at every level. The parser's whole claim is that a contract
    asserting on its result is asserting on the file; a walk that
    reordered a mapping, dropped an empty list or widened a boolean would
    break that claim while every example-based contract still passed.
    """
    assert _identical(docs._decoded(document, "probe"), document), (
        "the walk must return the tree it was given"
    )


@given(spelling=st.booleans(), value=_SCALARS)
def test_a_yaml_boolean_key_is_restored_to_its_source_spelling(
    *, spelling: bool, value: object
) -> None:
    """`on:` and `off:` are keys, and YAML 1.1 hands them back as booleans.

    Scenario: a single-entry mapping whose key is a boolean, which is
    what the loader produces for an unquoted `on:` or `off:`.

    Invariant: the key comes back as the word the file spells. A reader
    looking for `"on"` finds nothing when the key is `True`, so every
    contract over a workflow's triggers would be asserting on a key that
    is not there.
    """
    decoded = docs._decoded({spelling: value}, "probe")

    assert list(typ.cast("dict[str, object]", decoded)) == [
        "on" if spelling else "off"
    ], "a boolean key must be restored to the word the document spells"


@given(key=_BAD_KEYS, value=_SCALARS)
def test_a_key_that_is_neither_text_nor_a_boolean_is_refused(
    key: object, value: object
) -> None:
    """An unexpected key is raised on rather than dropped.

    Scenario: a single-entry mapping whose key is a number, None or a
    tuple.

    Invariant: the walk fails. Skipping the entry would be the quieter
    behaviour and the worse one: the contract would then report on a
    document with one fewer key than the file has.
    """
    with pytest.raises(docs.DocumentShapeError):
        docs._decoded({key: value}, "probe")


@given(document=_DOCUMENTS, bad=_BAD_VALUES, key=_KEYS)
def test_a_value_outside_the_json_set_is_refused_at_any_depth(
    document: object, bad: object, key: str
) -> None:
    """Depth is not a hiding place for a value the walk cannot represent.

    Scenario: an arbitrary tree with one unsupported value placed beside
    it, both inside a mapping, so the fault is never at the root.

    Invariant: the walk fails. The alternative is a `JsonValue` the
    annotation says cannot exist, which is the hole these parsers were
    written to close.
    """
    with pytest.raises(docs.DocumentShapeError):
        docs._decoded({key: document, "offending": [bad]}, "probe")
