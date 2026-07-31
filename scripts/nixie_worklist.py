"""Merge NUL-delimited Markdown path streams for Nixie validation."""

from __future__ import annotations

import sys
from collections.abc import Iterable, Sequence
from pathlib import Path

NUL = b"\0"
FALLBACK_PATH = b"."


def parse_path_records(stream: bytes) -> tuple[bytes, ...]:
    """Return complete NUL-delimited path records from one Git stream.

    Raises:
        ValueError: If a non-empty stream lacks its final NUL delimiter.
    """
    if not stream:
        return ()
    if not stream.endswith(NUL):
        msg = "Nixie path stream must end with a NUL delimiter"
        raise ValueError(msg)
    return tuple(stream[:-1].split(NUL))


def merge_path_streams(streams: Iterable[bytes]) -> bytes:
    """Return the bytewise sorted unique union of NUL-delimited streams."""
    records = {record for stream in streams for record in parse_path_records(stream)}
    return b"".join(record + NUL for record in sorted(records))


def select_nixie_paths(worklist: bytes) -> tuple[bytes, ...]:
    """Return worklist records, or the repository fallback for an empty union."""
    records = parse_path_records(worklist)
    return records or (FALLBACK_PATH,)


def main(arguments: Sequence[str] | None = None) -> int:
    """Merge path-stream files to standard output for the Makefile recipe."""
    paths = tuple(Path(argument) for argument in (arguments or sys.argv[1:]))
    if len(paths) != 4:
        print("expected four Git discovery stream files", file=sys.stderr)
        return 2
    sys.stdout.buffer.write(merge_path_streams(path.read_bytes() for path in paths))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
