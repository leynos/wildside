"""Property tests for the shared-actions pin-shape assertion helper.

The example-based tests in :mod:`ci_workflow_test` check the pins that
``.github/workflows/ci.yml`` happens to carry today. These tests instead pin the
helper's acceptance rule itself, independently of any workflow file: ``uses``
must be a string of the exact form ``<expected_path>@<40 lowercase hex digits>``
and nothing else.

Every strategy here constructs only the shape it means to exercise rather than
generating broadly and discarding with ``assume``, so the search budget is spent
on the boundary under test and counter-examples shrink cleanly.
"""

from __future__ import annotations

import typing as typ

import pytest
from ci_workflow_test import _assert_pinned_to_full_sha
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    from hypothesis.strategies import DrawFn

EXPECTED_PATH = "leynos/shared-actions/.github/actions/upload-codescene-coverage"

_SHA_LENGTH = 40
_HEX_CHARS = "0123456789abcdef"
_UPPER_HEX_CHARS = "ABCDEF"
_NON_HEX_CHARS = "ghijklmnopqrstuvwxyz-_. !/*"
# Deliberately excludes "@" so a generated path never splits early.
_PATH_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789/.-_"

# Wrong-path strategies stay at or below this length, which is shorter than
# EXPECTED_PATH. A generated path therefore cannot coincide with the real one,
# so the "wrong path" properties need no filtering to stay honest.
_MAX_WRONG_PATH = 40

shas = st.text(alphabet=_HEX_CHARS, min_size=_SHA_LENGTH, max_size=_SHA_LENGTH)


@st.composite
def _shas_with_uppercase(draw: DrawFn) -> str:
    """Generate a 40-character SHA with at least one uppercase hex digit."""
    sha = draw(shas)
    index = draw(st.integers(min_value=0, max_value=_SHA_LENGTH - 1))
    replacement = draw(st.sampled_from(_UPPER_HEX_CHARS))
    return sha[:index] + replacement + sha[index + 1 :]


@st.composite
def _shas_with_non_hex(draw: DrawFn) -> str:
    """Generate a 40-character reference with at least one non-hex character."""
    sha = draw(shas)
    index = draw(st.integers(min_value=0, max_value=_SHA_LENGTH - 1))
    replacement = draw(st.sampled_from(_NON_HEX_CHARS))
    return sha[:index] + replacement + sha[index + 1 :]


# min_size=0 means this also covers the empty reference, i.e. a bare trailing "@".
wrong_length_refs = st.one_of(
    st.text(alphabet=_HEX_CHARS, max_size=_SHA_LENGTH - 1),
    st.text(alphabet=_HEX_CHARS, min_size=_SHA_LENGTH + 1, max_size=80),
)

non_strings = st.one_of(
    st.none(),
    st.booleans(),
    st.integers(),
    st.floats(allow_nan=False, allow_infinity=False),
    st.binary(max_size=8),
    st.lists(st.integers(), max_size=3),
    st.tuples(st.integers()),
    st.dictionaries(st.text(max_size=3), st.integers(), max_size=3),
)


def test_wrong_path_bound_cannot_collide_with_the_expected_path() -> None:
    """The wrong-path strategies are too short to generate the real path."""
    assert len(EXPECTED_PATH) > _MAX_WRONG_PATH


@given(sha=shas)
def test_accepts_the_expected_path_with_a_full_lowercase_sha(sha: str) -> None:
    """Every well-formed pin is accepted."""
    _assert_pinned_to_full_sha(f"{EXPECTED_PATH}@{sha}", EXPECTED_PATH)


@given(value=non_strings)
def test_rejects_non_string_uses_values(value: object) -> None:
    """A `uses` value that is not a string is rejected."""
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(value, EXPECTED_PATH)


@given(path=st.text(alphabet=_PATH_CHARS, max_size=_MAX_WRONG_PATH), sha=shas)
def test_rejects_an_unexpected_action_path(path: str, sha: str) -> None:
    """A valid SHA on the wrong action path is rejected."""
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(f"{path}@{sha}", EXPECTED_PATH)


@given(value=st.text(alphabet=_PATH_CHARS, max_size=80))
def test_rejects_uses_without_an_at_separator(value: str) -> None:
    """A `uses` value carrying no `@` ref is rejected."""
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(value, EXPECTED_PATH)


@given(ref=wrong_length_refs)
def test_rejects_references_that_are_not_forty_characters(ref: str) -> None:
    """Hex references shorter or longer than 40 characters are rejected."""
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(f"{EXPECTED_PATH}@{ref}", EXPECTED_PATH)


@given(sha=_shas_with_uppercase())
def test_rejects_uppercase_hexadecimal_references(sha: str) -> None:
    """Uppercase hex digits are rejected, so the pin stays canonically lowercase."""
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(f"{EXPECTED_PATH}@{sha}", EXPECTED_PATH)


@given(sha=_shas_with_non_hex())
def test_rejects_non_hexadecimal_references(sha: str) -> None:
    """A 40-character reference containing a non-hex character is rejected."""
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(f"{EXPECTED_PATH}@{sha}", EXPECTED_PATH)


@given(sha=shas, extra=st.text(alphabet=_HEX_CHARS, max_size=_SHA_LENGTH))
def test_rejects_references_containing_extra_at_separators(
    sha: str, extra: str
) -> None:
    """A second `@` leaves it in the ref, which is then not a bare 40-hex SHA."""
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(f"{EXPECTED_PATH}@{sha}@{extra}", EXPECTED_PATH)


@given(sha=shas, suffix=st.sampled_from(["\n", "\r\n", "\r", " ", "\t", "\n\n"]))
def test_rejects_trailing_whitespace_after_the_sha(sha: str, suffix: str) -> None:
    """Guard the `re.match` to `re.fullmatch` fix.

    `SHA_RE` is anchored ``^...$``, and Python's ``$`` also matches just before a
    trailing newline, so ``re.match`` accepted a 40-hex SHA with one appended.
    """
    with pytest.raises(AssertionError):
        _assert_pinned_to_full_sha(f"{EXPECTED_PATH}@{sha}{suffix}", EXPECTED_PATH)
