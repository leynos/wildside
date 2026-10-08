"""Execute the compiler-cache statistics step and evaluate its condition.

`compiler_cache_test.py` pins the step's text, which shows the text is as
written and nothing about what a runner does with it. These tests do the two
things a reader would otherwise have to trust.

The condition is evaluated the way GitHub Actions would for each case that
matters: `setup-rust` reporting `fallback`, `started` and no status, each with
the job green and with an earlier step failed. A fallback has to skip the report
in both states, because an uncached job has no statistics to publish. Every
other status has to run it, and a failed build has to run it too, because a
failed build is when the numbers are wanted.

The step's own script is run under `bash` with a stand-in `sccache` defined as a
shell function through `BASH_ENV`, so a started job's report is read back: the
backend named in the log, the statistics in the log and the job summary, and a
job that never installed sccache reporting nothing and succeeding.

The evaluator mirrors GitHub's implicit `success() &&` on a condition naming no
status function. It supports only `always()`, `success()`, `failure()`, an
output compared with `==` or `!=` against a string literal, `&&` and `||`.
Anything else raises, so a condition it cannot evaluate fails the contract
instead of being guessed at.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the step's own script is the subject.
import typing as typ

import pytest
import workflow_inventory as inv

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The jobs that carry the statistics step, as (workflow, job).
JOBS = (
    ("ci.yml", "build"),
    ("ci.yml", "coverage"),
    ("coverage-main.yml", "coverage-upload"),
)
REPORT_STEP = "Record compiler-cache effectiveness"
SETUP_ID = "setup-rust"
_COMPARISON = re.compile(
    r"^steps\.(?P<step>[\w-]+)\.outputs\.(?P<name>[\w-]+)"
    r"(?P<op>==|!=)'(?P<literal>[^']*)'$"
)
_STATUS_FUNCTION = re.compile(r"\b(?:always|success|failure|cancelled)\(\)")

#: `(setup-rust's sccache-status, whether an earlier step failed, runs?)`.
CASES: typ.Final = [
    pytest.param(("fallback", False, False), id="fallback-green-skips"),
    pytest.param(("fallback", True, False), id="fallback-after-a-failure-skips"),
    pytest.param(("started", False, True), id="started-green-runs"),
    pytest.param(("started", True, True), id="started-after-a-failure-runs"),
    pytest.param(("", False, True), id="no-status-green-runs"),
    pytest.param(("", True, True), id="no-status-after-a-failure-runs"),
]

#: The stand-in `sccache`: records its arguments and prints one statistics line.
FAKE_SCCACHE = (
    "sccache() {\n"
    '  printf \'%s\\n\' "$*" >> "$CALLS_FILE"\n'
    "  printf '%s\\n' 'Cache location                  ghac (fake)'\n"
    "}\n"
)
FAKE_STATISTICS = "Cache location                  ghac (fake)"


def _with_implicit_success(condition: str) -> str:
    """Return the condition text GitHub would evaluate, braces stripped.

    Returns
    -------
    str
        The condition with an implicit `success() &&` made explicit.
    """
    text = condition.strip().removeprefix("${{").removesuffix("}}").strip()
    if _STATUS_FUNCTION.search(text):
        return text
    return f"success() && {text}" if text else "success()"


def _atom(atom: str, outputs: dict[tuple[str, str], str], *, job_failed: bool) -> bool:
    """Evaluate one comparison or status function."""
    functions = {
        "always()": True,
        "success()": not job_failed,
        "failure()": job_failed,
    }
    if atom in functions:
        return functions[atom]
    match = _COMPARISON.match(re.sub(r"\s+", "", atom))
    if match is None:
        message = f"cannot evaluate {atom!r}"
        raise ValueError(message)
    equal = outputs.get((match["step"], match["name"]), "") == match["literal"]
    return equal if match["op"] == "==" else not equal


def _arm_holds(
    arm: str, outputs: dict[tuple[str, str], str], *, job_failed: bool
) -> bool:
    """Return whether every `&&` conjunct of one `||` arm holds."""
    return all(
        _atom(atom.strip(), outputs, job_failed=job_failed) for atom in arm.split("&&")
    )


def evaluate(
    condition: str, outputs: dict[tuple[str, str], str], *, job_failed: bool
) -> bool:
    """Evaluate a step `if:` the way GitHub Actions would for the given state.

    Parameters
    ----------
    condition : str
        The step's `if:` text. An empty string means no condition, and a
        condition that names no status function gets an implicit
        `success() &&`. The grammar is `&&` and `||` over `always()`,
        `success()`, `failure()` and `steps.<id>.outputs.<name>` compared with
        `==` or `!=` against a single-quoted string literal.
    outputs : dict[tuple[str, str], str]
        Step outputs by `(step id, output name)`. A missing output reads as an
        empty string, as it does on a runner.
    job_failed : bool
        Whether an earlier step failed, which decides `success()` and
        `failure()`.

    Returns
    -------
    bool
        Whether the step would run.
    """
    text = _with_implicit_success(condition)
    return any(
        _arm_holds(arm, outputs, job_failed=job_failed) for arm in text.split("||")
    )


def _report(filename: str, job_id: str) -> dict[str, typ.Any]:
    """Return one job's statistics step."""
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    return inv.find_step(steps, REPORT_STEP)


@pytest.mark.parametrize(("filename", "job_id"), JOBS)
@pytest.mark.parametrize("case", CASES)
def test_the_statistics_step_runs_exactly_when_there_are_statistics(
    filename: str, job_id: str, case: tuple[str, bool, bool]
) -> None:
    """A fallback skips the report in both job states; everything else runs it."""
    status, job_failed, runs = case
    report = _report(filename, job_id)
    outputs = {(SETUP_ID, "sccache-status"): status}
    verdict = evaluate(str(report.get("if", "")), outputs, job_failed=job_failed)
    assert verdict is runs, (
        f"{filename}:{job_id}: with sccache-status={status!r} and "
        f"job_failed={job_failed}, {REPORT_STEP!r} must "
        f"{'run' if runs else 'be skipped'} (condition {report.get('if')!r})"
    )


@pytest.mark.parametrize(
    ("condition", "status", "job_failed", "expected"),
    [
        pytest.param(
            "always() && steps.setup-rust.outputs.sccache-status != 'fallback'",
            "fallback",
            True,
            False,
            id="the-lane-condition-skips-a-fallback-after-a-failure",
        ),
        pytest.param(
            "always() || steps.setup-rust.outputs.sccache-status != 'fallback'",
            "fallback",
            False,
            True,
            id="an-or-composed-guard-runs-on-a-fallback",
        ),
        pytest.param(
            "steps.setup-rust.outputs.sccache-status != 'fallback'",
            "started",
            True,
            False,
            id="a-bare-guard-gets-an-implicit-success-and-loses-a-failed-build",
        ),
        pytest.param(
            "always() && steps.setup-rust.outputs.sccache-status == 'fallback'",
            "fallback",
            False,
            True,
            id="an-inverted-guard-runs-on-a-fallback",
        ),
        pytest.param("always()", "fallback", False, True, id="an-omitted-guard-runs"),
        pytest.param("", "started", True, False, id="no-condition-means-success-only"),
    ],
)
def test_the_evaluator_tells_a_working_guard_from_a_broken_one(
    condition: str, status: str, *, job_failed: bool, expected: bool
) -> None:
    """The narrow half: each way of breaking the guard changes the outcome."""
    outputs = {(SETUP_ID, "sccache-status"): status}
    assert evaluate(condition, outputs, job_failed=job_failed) is expected, (
        f"{condition!r} with sccache-status={status!r}, job_failed={job_failed} "
        f"must evaluate to {expected}"
    )


def test_an_expression_the_evaluator_cannot_read_is_refused() -> None:
    """Guessing at unsupported syntax would pass a condition nobody evaluated."""
    with pytest.raises(ValueError, match="cannot evaluate"):
        evaluate("!(always())", {}, job_failed=False)


class Outcome(typ.NamedTuple):
    """What one run of the statistics step produced."""

    succeeded: bool
    stdout: str
    summary: str
    calls: str


def _run_step(script: str, tmp_path: Path, *, installed: bool, backend: str) -> Outcome:
    """Run `script` with the stand-in `sccache` defined, or without it."""
    bash = shutil.which("bash")
    if bash is None:
        pytest.skip("bash not found on PATH")
    (tmp_path / "fake.sh").write_text(FAKE_SCCACHE, encoding="utf-8")
    (tmp_path / "summary").write_text("", encoding="utf-8")
    environment = {
        "PATH": "/usr/bin:/bin",
        "SCCACHE_BACKEND": backend,
        "GITHUB_STEP_SUMMARY": str(tmp_path / "summary"),
        "CALLS_FILE": str(tmp_path / "calls"),
    }
    if installed:
        environment["BASH_ENV"] = str(tmp_path / "fake.sh")
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the step's own script under test.
        [bash, "-c", script],
        capture_output=True,
        check=False,
        cwd=tmp_path,
        env=environment,
        text=True,
        timeout=30,
    )
    calls = tmp_path / "calls"
    return Outcome(
        succeeded=completed.returncode == 0,
        stdout=completed.stdout,
        summary=(tmp_path / "summary").read_text(encoding="utf-8"),
        calls=calls.read_text(encoding="utf-8") if calls.exists() else "",
    )


@pytest.mark.parametrize(("filename", "job_id"), JOBS)
def test_a_started_job_reports_its_backend_and_statistics(
    filename: str, job_id: str, tmp_path: Path
) -> None:
    """The backend and the statistics reach the log and the job summary."""
    script = str(_report(filename, job_id)["run"])
    outcome = _run_step(script, tmp_path, installed=True, backend="ubicloud")

    assert outcome.succeeded, f"{filename}:{job_id} report must succeed"
    assert outcome.calls.strip() == "--show-stats", (
        f"{filename}:{job_id} must ask sccache for its statistics once"
    )
    assert "backend: ubicloud" in outcome.stdout, outcome.stdout
    assert FAKE_STATISTICS in outcome.stdout, outcome.stdout
    assert "- backend: ubicloud" in outcome.summary, outcome.summary
    assert FAKE_STATISTICS in outcome.summary, outcome.summary


@pytest.mark.parametrize(("filename", "job_id"), JOBS)
def test_a_job_that_never_installed_sccache_reports_nothing_and_succeeds(
    filename: str, job_id: str, tmp_path: Path
) -> None:
    """`always()` reaches this step after an early failure; it must not add one."""
    script = str(_report(filename, job_id)["run"])
    outcome = _run_step(script, tmp_path, installed=False, backend="ubicloud")

    assert outcome.succeeded, f"{filename}:{job_id} must not fail without sccache"
    assert "sccache is not installed" in outcome.stdout, outcome.stdout
    assert not outcome.summary, "nothing may reach the summary without sccache"
