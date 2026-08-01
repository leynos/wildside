"""Properties and examples for Nixie's NUL-delimited worklist merge."""

from __future__ import annotations

from itertools import chain

from hypothesis import given, strategies as st

from scripts.nixie_worklist import main, merge_path_streams, select_nixie_paths

PATH_CHARACTER = st.characters(
    blacklist_categories=("Cs",),
    blacklist_characters=("\0", "/"),
)
PATH_RECORD = st.text(PATH_CHARACTER, min_size=1, max_size=40).map(
    lambda name: f"docs/{name}.md".encode()
)
PATH_STREAM = st.lists(PATH_RECORD, max_size=12)


def encode_records(records: list[bytes]) -> bytes:
    """Encode path records without passing through shell-style splitting."""
    return b"".join(record + b"\0" for record in records)


@given(st.tuples(PATH_STREAM, PATH_STREAM, PATH_STREAM, PATH_STREAM))
def test_merge_emits_bytewise_sorted_unique_union(
    record_streams: tuple[list[bytes], list[bytes], list[bytes], list[bytes]],
) -> None:
    """Every ordering and duplicate distribution yields the canonical union."""
    streams = tuple(encode_records(records) for records in record_streams)
    expected = encode_records(sorted(set(chain.from_iterable(record_streams))))

    assert merge_path_streams(streams) == expected, (
        "the merge must produce the canonical bytewise sorted unique union"
    )


def test_empty_union_selects_repository_fallback() -> None:
    """An empty worklist selects the repository rather than an empty argv."""
    worklist = merge_path_streams((b"", b"", b"", b""))

    assert select_nixie_paths(worklist) == (b".",), (
        "an empty worklist must select the dot fallback"
    )


def test_explicit_empty_arguments_are_not_replaced_by_process_arguments() -> None:
    """An explicitly empty argument sequence reports invalid usage."""
    assert main(()) == 2, "explicitly empty arguments must produce the usage exit code"


def test_unusual_filenames_remain_literal_records() -> None:
    """Spaces and shell metacharacters remain opaque path bytes."""
    records = [
        b"docs/zeta [draft].md",
        b"docs/semi;colon & brackets[1].md",
        b"docs/a space.md",
        b"docs/semi;colon & brackets[1].md",
    ]

    worklist = merge_path_streams((encode_records(records), b"", b"", b""))

    assert select_nixie_paths(worklist) == tuple(sorted(set(records))), (
        "unusual filenames must remain literal records"
    )
