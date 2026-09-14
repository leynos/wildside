"""Contract tests that keep audit exceptions honest.

An ignored advisory is only defensible while the reason given for ignoring it
still holds. These tests enforce the reasoning rather than trusting a future
reader to remember it, so a change that invalidates an exception fails here
instead of silently widening the repository's exposure.
"""

from __future__ import annotations

import json
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


#: The frontend audit exception ledger. `security/run-bun-audit.js` turns every
#: entry here into a `bun audit --ignore=<GHSA>` flag, so an entry silences the
#: advisory repository-wide until someone removes it.
LEDGER_PATH = REPOSITORY_ROOT / "security" / "audit-exceptions.json"
BUN_LOCK_PATH = REPOSITORY_ROOT / "bun.lock"

#: The two unfixable advisories against `extract-zip` 2.0.1. Both rest on the
#: same argument, so both die with the same upgrade.
EXTRACT_ZIP_EXCEPTION_IDS = (
    "EXTRACT_ZIP_SYMLINK_TRAVERSAL_2026_08",
    "EXTRACT_ZIP_ARBITRARY_WRITE_2026_09",
)
EXTRACT_ZIP_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/491"

#: The two Picomatch advisories Bun cannot resolve through `resolutions`.
PICOMATCH_EXCEPTION_IDS = (
    "BUN_PICOMATCH_POSIX_2026_06",
    "BUN_PICOMATCH_REDOS_2026_06",
)
PICOMATCH_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/492"

#: Picomatch releases each advisory covers, as the `pnpm.overrides` ranged keys
#: in `package.json` express them. A resolved version inside any of these is
#: what the Bun exceptions exist to tolerate.
VULNERABLE_PICOMATCH_RANGES = (
    ((0, 0, 0), (2, 3, 2)),
    ((3, 0, 0), (3, 0, 2)),
    ((4, 0, 0), (4, 0, 4)),
)

#: A removal condition is an issue reference, either bare or as a full URL. An
#: exception without one has no owner and no end.
_ISSUE_REFERENCE = re.compile(r"(?:#\d+|/issues/\d+)")

_PICOMATCH_RESOLUTION = re.compile(r'"picomatch@(\d+)\.(\d+)\.(\d+)"')


def _ledger() -> list[dict[str, str]]:
    """Return the frontend audit exception entries."""
    return json.loads(LEDGER_PATH.read_text(encoding="utf-8"))


def _entry_ids() -> set[str]:
    """Return the identifiers of every ledger entry."""
    return {entry["id"] for entry in _ledger()}


def _resolved_picomatch_versions() -> list[tuple[int, int, int]]:
    """Return every Picomatch version Bun's lockfile resolves."""
    return [
        (int(major), int(minor), int(patch))
        for major, minor, patch in _PICOMATCH_RESOLUTION.findall(
            BUN_LOCK_PATH.read_text(encoding="utf-8")
        )
    ]


def test_every_audit_exception_names_a_removal_condition() -> None:
    """An exception with no tracking issue becomes permanent by default.

    The ledger's `expiresAt` field forces a decision on a date, but it does not
    say who makes it or what would settle it. Requiring an issue reference in
    the reason keeps the removal condition findable from the entry itself.
    """
    unowned = [
        entry["id"]
        for entry in _ledger()
        if not _ISSUE_REFERENCE.search(entry.get("reason", ""))
    ]
    assert not unowned, (
        f"these audit exceptions name no tracking issue: {unowned}. "
        f"An exception nobody can close is a permanent one."
    )


def test_the_extract_zip_exceptions_are_void_once_puppeteer_drops_it() -> None:
    """Both `extract-zip` exceptions rest on the package still being reachable.

    `extract-zip` 2.0.1 has no patched release, so the entries argue from
    reachability instead: only `puppeteer` 23 pulls it in, through
    `@puppeteer/browsers` 2.x, to unpack an archive Puppeteer fetched itself.
    `@puppeteer/browsers` 3.x replaced it with `modern-tar`, so upgrading past
    `puppeteer` 25.7 removes the package and the advisories together. Failing
    here is the intended outcome of that upgrade: delete both entries.
    """
    present = _entry_ids() & set(EXTRACT_ZIP_EXCEPTION_IDS)
    if not present:
        pytest.skip("the extract-zip advisories are no longer excepted")

    lockfile = BUN_LOCK_PATH.read_text(encoding="utf-8")
    assert '"extract-zip@' in lockfile, (
        f"{sorted(present)} except advisories against extract-zip, but Bun no "
        f"longer resolves it. Remove the entries and close "
        f"{EXTRACT_ZIP_TRACKING_ISSUE}."
    )


def test_the_picomatch_exceptions_are_void_once_bun_resolves_patched_builds() -> None:
    """The Picomatch exceptions rest on Bun holding a vulnerable build.

    Patched Picomatch releases exist on every affected major and `pnpm` reaches
    them through ranged `pnpm.overrides` keys. Bun ignores ranged `resolutions`
    keys and a bare key collapses the tree onto one major, so `bun.lock` keeps a
    vulnerable build. Once it stops doing so, whether Bun gained ranged
    resolution or the last Picomatch 2 consumer left, the exceptions are
    unnecessary rather than merely stale.
    """
    present = _entry_ids() & set(PICOMATCH_EXCEPTION_IDS)
    if not present:
        pytest.skip("the Picomatch advisories are no longer excepted")

    resolved = _resolved_picomatch_versions()
    assert resolved, "bun.lock must resolve Picomatch for these entries to apply"

    vulnerable = [
        version
        for version in resolved
        if any(low <= version < high for low, high in VULNERABLE_PICOMATCH_RANGES)
    ]
    assert vulnerable, (
        f"{sorted(present)} except Picomatch advisories on the grounds that Bun "
        f"cannot resolve a patched build, but bun.lock now resolves only "
        f"{sorted(resolved)}. Remove the entries and close "
        f"{PICOMATCH_TRACKING_ISSUE}."
    )
