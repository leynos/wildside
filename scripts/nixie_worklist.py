"""Merge NUL-delimited Markdown path streams for Nixie validation."""

from __future__ import annotations

import sys
from collections.abc import Iterable, Sequence
from pathlib import Path

NUL = b"\0"
FALLBACK_PATH = b"."


def parse_path_records(stream: bytes) -> tuple[bytes, ...]:
    """Return complete NUL-delimited path records from one Git stream.

    Parameters
    ----------
    stream : bytes
        Raw path records separated and terminated by NUL bytes.

    Returns
    -------
    tuple[bytes, ...]
        The path records in their original order, without their delimiters.

    Raises
    ------
    ValueError
        If a non-empty stream lacks its final NUL delimiter.

    Examples
    --------
    Parse valid output from a Git command using ``-z``::

        >>> parse_path_records(b"docs/one.md\0docs/two.md\0")
        (b'docs/one.md', b'docs/two.md')

    Reject a truncated record stream::

        >>> parse_path_records(b"docs/one.md")
        Traceback (most recent call last):
        ...
        ValueError: Nixie path stream must end with a NUL delimiter
    """
    if not stream:
        return ()
    if not stream.endswith(NUL):
        msg = "Nixie path stream must end with a NUL delimiter"
        raise ValueError(msg)
    return tuple(stream[:-1].split(NUL))


def merge_path_streams(streams: Iterable[bytes]) -> bytes:
    """Return the bytewise sorted unique union of NUL-delimited streams.

    Parameters
    ----------
    streams : Iterable[bytes]
        Git path streams whose records are separated and terminated by NUL
        bytes.

    Returns
    -------
    bytes
        The unique path records in bytewise order, separated and terminated by
        NUL bytes.

    Raises
    ------
    ValueError
        If any non-empty input stream lacks its final NUL delimiter.

    Examples
    --------
    Merge duplicate and unordered records without decoding filenames::

        >>> merge_path_streams((b"docs/z.md\0docs/a.md\0", b"docs/z.md\0"))
        b'docs/a.md\x00docs/z.md\x00'
    """
    records = {record for stream in streams for record in parse_path_records(stream)}
    return b"".join(record + NUL for record in sorted(records))


def select_nixie_paths(worklist: bytes) -> tuple[bytes, ...]:
    """Return worklist records, or the repository fallback for an empty union.

    Parameters
    ----------
    worklist : bytes
        A merged path stream whose records are separated and terminated by NUL
        bytes.

    Returns
    -------
    tuple[bytes, ...]
        The worklist records, or ``(b".",)`` when the stream is empty.

    Raises
    ------
    ValueError
        If a non-empty worklist lacks its final NUL delimiter.

    Examples
    --------
    Select either the discovered paths or the full-repository fallback::

        >>> select_nixie_paths(b"docs/guide.md\0")
        (b'docs/guide.md',)
        >>> select_nixie_paths(b"")
        (b'.',)
    """
    records = parse_path_records(worklist)
    return records or (FALLBACK_PATH,)


def main(arguments: Sequence[str] | None = None) -> int:
    """Merge four path-stream files to standard output for the Makefile recipe.

    Parameters
    ----------
    arguments : Sequence[str] or None
        Four path-stream filenames. When omitted, command-line arguments after
        the program name are used.

    Returns
    -------
    int
        Zero after writing a valid merged stream, or two when the number of
        stream files is not four.

    Raises
    ------
    OSError
        If a stream file cannot be read or the merged stream cannot be written.
    ValueError
        If any non-empty stream file lacks its final NUL delimiter.

    Examples
    --------
    An explicit empty file list is invalid::

        >>> main(())
        2
    """
    supplied_arguments = sys.argv[1:] if arguments is None else arguments
    paths = tuple(Path(argument) for argument in supplied_arguments)
    if len(paths) != 4:
        print("expected four Git discovery stream files", file=sys.stderr)
        return 2
    sys.stdout.buffer.write(merge_path_streams(path.read_bytes() for path in paths))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
