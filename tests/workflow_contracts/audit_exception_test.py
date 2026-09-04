"""Contract tests that keep audit exceptions honest.

An ignored advisory is only defensible while the reason given for ignoring it
still holds. These tests enforce the reasoning rather than trusting a future
reader to remember it, so a change that invalidates an exception fails here
instead of silently widening the repository's exposure.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPOSITORY_ROOT / "Makefile"
BACKEND_SOURCE_DIR = REPOSITORY_ROOT / "backend" / "src"

#: RUSTSEC-2026-0258, "h2 unbounded empty DATA frames". `actix-http` 3.x
#: depends on `h2` `^0.3`, which has no patched release, so the advisory is
#: ignored on the grounds that the code path is unreachable as deployed.
H2_ADVISORY = "RUSTSEC-2026-0258"
H2_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/472"

#: The bindings that would make actix-web negotiate HTTP/2. It does so only
#: over TLS ALPN or through an explicit cleartext h2c binding, so the presence
#: of any of these is exactly what invalidates the unreachability argument.
HTTP2_ENABLING_BINDINGS = ("bind_rustls", "bind_openssl", "bind_auto_h2c")

_IGNORES_LINE = re.compile(r"^CARGO_AUDIT_IGNORES\s*:?=\s*(?P<value>.*)$", re.MULTILINE)


def _cargo_audit_ignores() -> str:
    """Return the advisory identifiers `make rust-audit` suppresses."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = _IGNORES_LINE.search(makefile)
    if match is None:
        pytest.fail("the Makefile must declare CARGO_AUDIT_IGNORES")
    return match.group("value")


def _rust_sources() -> list[Path]:
    """Return the backend's Rust sources."""
    return sorted(BACKEND_SOURCE_DIR.rglob("*.rs"))


def test_the_h2_ignore_records_its_removal_condition() -> None:
    """An ignore with no tracking issue becomes permanent by default."""
    if H2_ADVISORY not in _cargo_audit_ignores():
        pytest.skip(f"{H2_ADVISORY} is no longer ignored")
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    assert H2_TRACKING_ISSUE in makefile, (
        f"the {H2_ADVISORY} ignore must name its removal condition"
    )


def test_the_h2_ignore_is_void_once_the_server_can_speak_http2() -> None:
    """The h2 ignore rests on the server never negotiating HTTP/2.

    `backend/src/server/mod.rs` binds plaintext, so `actix-http`'s vulnerable
    `h2` 0.3 path is unreachable. Adding a TLS listener or a cleartext h2c
    binding makes it reachable, at which point the advisory must be resolved
    rather than ignored. Failing here is the intended outcome of that change:
    remove the ignore, or establish a new reason and rewrite this test.
    """
    if H2_ADVISORY not in _cargo_audit_ignores():
        pytest.skip(f"{H2_ADVISORY} is no longer ignored")

    offenders = [
        f"{source.relative_to(REPOSITORY_ROOT)}:{number} {binding}"
        for source in _rust_sources()
        for number, line in enumerate(
            source.read_text(encoding="utf-8").splitlines(), start=1
        )
        for binding in HTTP2_ENABLING_BINDINGS
        if binding in line
    ]
    assert not offenders, (
        f"{H2_ADVISORY} is ignored because the server cannot negotiate HTTP/2, "
        f"but these bindings would let it: {offenders}. Resolve the advisory "
        f"or revisit {H2_TRACKING_ISSUE} before enabling them."
    )
