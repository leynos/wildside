"""Property tests for the document string walk.

The contract in `codescene_coverage_baseline_test.py` asks whether a
credential name appears anywhere in a pull-request workflow, and reads
the two workflows this repository happens to carry. Neither is nested
deeply, neither mixes string and non-string keys at the same level, and
neither carries an empty container. So the example-based cases exercise
one point in the space the walk claims to cover, and the claim is the
thing the contract rests on: a walk that missed a shape would report a
clean workflow, and the contract would pass for the wrong reason rather
than because the credential is gone.

These properties pin the rule instead, independently of any file. The
oracle is an explicit stack rather than a second recursion, so the two
agree only when both are right rather than when both share a bug, and it
is written to yield in the same order so the comparison can be on the
sequence rather than on a set. Comparing sets would hide a walk that
found every string once but dropped every duplicate, and a credential
appearing twice is exactly the case the contract cares about.
"""

from __future__ import annotations

import string

from document_strings import strings_in
from hypothesis import given
from hypothesis import strategies as st

#: Keys a workflow plausibly carries, from a small alphabet so a
#: counter-example reads as a key rather than as noise.
_KEYS = st.text(alphabet="abcdefghijklmnopqrstuvwxyz-_.", min_size=1, max_size=6)

#: Keys a parsed workflow really does carry that are not strings. YAML
#: 1.1 turns a bare `on` into the boolean `True`, so this is not a
#: hypothetical: the walk meets one in `ci.yml` itself.
_NON_STRING_KEYS = st.one_of(st.booleans(), st.integers(-5, 5), st.none())

#: Leaves. The non-string scalars are drawn as often as the strings
#: because the walk's rule is as much about what it refuses as about
#: what it finds: a timeout is a number and must yield nothing.
_LEAVES = st.one_of(
    st.text(max_size=8),
    st.none(),
    st.booleans(),
    st.integers(-1_000, 1_000),
    st.floats(allow_nan=False, allow_infinity=False),
)

#: Bounded trees mixing both key kinds, lists, and empty containers.
#: `max_size=4` rather than a larger bound keeps a falsifying example
#: small enough to read, and `max_leaves` bounds the whole tree.
_DOCUMENTS = st.recursive(
    _LEAVES,
    lambda children: st.one_of(
        st.lists(children, max_size=4),
        st.dictionaries(_KEYS, children, max_size=4),
        st.dictionaries(_NON_STRING_KEYS, children, max_size=3),
        st.dictionaries(st.one_of(_KEYS, _NON_STRING_KEYS), children, max_size=4),
    ),
    max_leaves=12,
)


def _oracle(root: object) -> list[str]:
    """Return every string in a document, found with an explicit stack.

    The implementation recurses and yields; this drives a stack and
    appends, so a mutation to either one's control flow does not move
    the other. The stack is extended in reverse so the output order
    matches, which lets the properties compare sequences rather than
    sets and so keep duplicates meaningful.

    Parameters
    ----------
    root : object
        Any part of a parsed document.

    Returns
    -------
    list[str]
        Each string found, in the order met, duplicates kept.
    """
    found: list[str] = []
    pending: list[object] = [root]
    while pending:
        node = pending.pop()
        if isinstance(node, str):
            found.append(node)
        elif isinstance(node, dict):
            children: list[object] = []
            for key, value in node.items():
                if isinstance(key, str):
                    children.append(key)
                children.append(value)
            pending.extend(reversed(children))
        elif isinstance(node, list):
            pending.extend(reversed(node))
    return found


@given(document=_DOCUMENTS)
def test_the_walk_agrees_with_an_independent_traversal(document: object) -> None:
    """Every string is found, in order, at any depth.

    Scenario: bounded trees mixing scalars, lists, mappings keyed by
    strings, mappings keyed by the booleans and integers YAML really
    produces, mappings mixing both, and empty containers. Invariant: the
    walk's output equals a stack-based oracle's, as a sequence.

    Sequence equality rather than set equality is the point. A walk that
    found every distinct string but collapsed duplicates would satisfy a
    set comparison, and the contract this supports counts occurrences to
    report which parts of a workflow name a credential.
    """
    assert list(strings_in(document)) == _oracle(document), (
        f"the walk disagreed with the oracle on {document!r}"
    )


@given(document=_DOCUMENTS)
def test_the_walk_yields_only_strings(document: object) -> None:
    """Nothing but text comes out, whatever went in.

    Scenario: the same trees, which carry numbers, booleans and `None`
    at every level. Invariant: every yielded item is a string. A walk
    that coerced a non-string key or value would satisfy the agreement
    property above only if the oracle coerced identically, so this is
    asserted on the walk alone rather than between the two.
    """
    assert all(isinstance(found, str) for found in strings_in(document)), (
        f"a non-string escaped the walk on {document!r}"
    )


@given(text=st.text(max_size=8), key=_KEYS)
def test_a_nested_string_is_found_wherever_it_is_put(text: str, key: str) -> None:
    """A string is found under a list, a mapping, or a non-string key.

    Scenario: one string placed at the bottom of each shape the walk
    descends, including a mapping keyed by the boolean YAML makes of a
    bare `on`. Invariant: it comes out every time.

    This is the direction the agreement property cannot fail alone. Two
    traversals that both stopped at a mapping keyed by a boolean would
    agree with each other perfectly, and the contract resting on the
    walk would then miss a credential declared under exactly that key.
    """
    for wrapped in (
        [text],
        [[text]],
        {key: text},
        {key: [text]},
        {True: text},
        {True: {key: [text]}},
        [{key: {True: text}}],
    ):
        assert text in strings_in(wrapped), f"{text!r} was not found inside {wrapped!r}"


#: Values drawn from an alphabet disjoint from :data:`_KEYS`, so a
#: generated value can never also be one of the generated keys. Without
#: that, `{"_": "_"}` yields the string three times and the counting
#: property below is wrong rather than the walk. The full suite found
#: this; the module in isolation had passed.
_DISJOINT_VALUES = st.text(alphabet=string.digits, min_size=1, max_size=6).map(
    lambda digits: f"v{digits}"
)


@given(text=_DISJOINT_VALUES, key=_KEYS, other=_KEYS)
def test_a_repeated_string_is_yielded_once_per_occurrence(
    text: str, key: str, other: str
) -> None:
    """A string appearing twice comes out twice.

    Scenario: the same value under two different keys, and the same
    value twice in a list. Invariant: the walk yields it once per
    occurrence rather than once in total.

    Asserted directly rather than left to the agreement property,
    because the agreement property compares against an oracle and two
    traversals that both de-duplicated would agree. The contract resting
    on this walk reports *which* parts of a workflow name a credential,
    so an occurrence dropped is a site not reported.
    """
    if key == other:
        other = f"{other}x"
    assert list(strings_in([text, text])).count(text) == 2, (
        f"{text!r} appears twice in a list and must be yielded twice"
    )
    assert list(strings_in({key: text, other: text})).count(text) == 2, (
        f"{text!r} is the value of two keys and must be yielded twice"
    )
    assert list(strings_in({key: [text, text]})).count(text) == 2, (
        f"{text!r} appears twice inside one value and must be yielded twice; "
        "this is the case a per-subtree de-duplication would collapse while "
        "leaving the two-keys case above intact"
    )
