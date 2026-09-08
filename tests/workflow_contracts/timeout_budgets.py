"""Reading the timers that can end a test run.

The contract in :mod:`timeout_ordering_test` compares budgets written
down in three different files. Turning those files into comparable
seconds is the part that can be wrong without any file being wrong, so
it lives here where it can be read on its own.

The reading that matters most is the per-test one. nextest warns once
per ``period`` and terminates after ``terminate-after`` of them, so the
budget is their product, and a ``slow-timeout`` that names no
``terminate-after`` never terminates anything at all. That form is
refused rather than read as a single period: a test under it runs until
some outer tier stops it, which is the absence of tier one rather than a
small tier one.

The configuration is parsed with ``tomllib`` rather than matched as
text. A text match finds a key inside a comment, inside a ``filter``
string, or in a table nextest never consults, and reports a budget the
runner does not use. The commented-out ``global-timeout`` is the case
that matters most, because this contract requires that tier to be
present: a scraping reader would go on reporting a budget somebody had
switched off, and the four-tier contract would pass with three.

See "Test timeouts: four tiers, outermost last" in
``docs/developers-guide.md``, and the canonical wording in
`leynos/shared-actions`' `generate-coverage` README.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
WORKFLOWS_DIRECTORY: typ.Final[Path] = REPO_ROOT / ".github" / "workflows"
NEXTEST_CONFIG: typ.Final[Path] = REPO_ROOT / ".config" / "nextest.toml"

#: The environment variable the shared coverage action reads for its
#: wall-clock cap on one `cargo` invocation.
WATCHDOG_VARIABLE: typ.Final[str] = "RUN_RUST_CARGO_WAIT_TIMEOUT"

#: The action whose steps run under that watchdog.
COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)

#: Everything in a coverage job that is not a `cargo` invocation the
#: watchdog bounds: checkout, toolchain setup, the database fixtures, and
#: above all the cache save and restore. The job timer covers it; the
#: watchdog does not.
#:
#: Measured from the worst of several runs rather than one. Across ten
#: successful `coverage-main.yml` runs the widest gap between the
#: coverage step and its job was 522 s on run 33921794186, where the
#: database fixtures and the artefact upload run outside the coverage
#: step. Across ten of `ci.yml` it was 59 s on run 33938872167. Fifteen
#: minutes covers the worse of those, and none of those runs was
#: genuinely cold.
OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS: typ.Final[float] = 15 * 60.0

#: How far a ceiling must sit above the sum it contains, rather than
#: merely reaching it. A ceiling equal to that sum cancels the job at
#: the moment the watchdog would have reported the overrun, and the
#: report is the only thing that makes an overrun actionable.
CEILING_MARGIN_SECONDS: typ.Final[float] = 15 * 60.0

#: What nextest allows a test between `SIGTERM` and `SIGKILL` when the
#: configuration names no `grace-period`.
NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS: typ.Final[float] = 10.0

#: Added to that grace period to cover the teardown and report writing
#: that follow it. A separate term rather than a floor over the two, so
#: raising a grace period raises the requirement instead of vanishing
#: into it.
TERMINATION_SAFETY_MARGIN_SECONDS: typ.Final[float] = 60.0

#: Build time inside a `cargo` invocation before nextest starts its own
#: clock. Only used if a `global-timeout` appears.
COLD_BUILD_ALLOWANCE_SECONDS: typ.Final[float] = 10 * 60.0


def required_ceiling(budgets: cabc.Sequence[float], allowance: float) -> float:
    """Return the smallest acceptable ceiling for one job, in seconds.

    Three terms. Each coverage step may legitimately spend its whole
    watchdog, so the sum is the floor. The measured work outside those
    windows is added because the job timer covers it and the watchdogs
    do not. The margin is added because a ceiling equal to that sum
    cancels the job at the moment the watchdog would have reported the
    overrun.

    Parameters
    ----------
    budgets : cabc.Sequence[float]
        One watchdog budget per coverage step in the job.
    allowance : float
        The measured work outside those windows, in seconds.

    Returns
    -------
    float
        The smallest acceptable ceiling, in seconds.

    Examples
    --------
    One 5,400 s watchdog with 900 s of work outside it needs a ceiling
    of 7,200 s once the 900 s margin is added, which is this
    repository's 120 minutes:

    >>> required_ceiling([5400.0], 900.0)
    7200.0

    A job running the action twice must contain both budgets:

    >>> required_ceiling([5400.0, 1800.0], 0.0)
    8100.0
    """
    return sum(budgets) + allowance + CEILING_MARGIN_SECONDS
