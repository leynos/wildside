"""Recipe-shape contracts for the `lint-actions` and `test-lint-actions` targets.

These read the Makefile text directly; the command-level contracts that run
Make against fake tools live in `makefile_tooling_test.py`.
"""

from __future__ import annotations

import re
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

#: The single command `lint-actions` may run. The recipe used to carry a
#: `while` loop over composite actions and a `yamllint`-then-`actionlint` pair
#: in one `if`, where an earlier failure was replaced by a later command's
#: status. Shell guards could hide that; the scripting standards say gate
#: logic of this size belongs in a script, so the recipe now invokes one and
#: the ordering is tested in `scripts/tests/test_lint_actions.py`.
LINT_ACTIONS_SCRIPT = "scripts/lint_actions.py"


def test_lint_actions_runs_the_script_as_one_command() -> None:
    """The recipe must delegate rather than sequence tools itself.

    Asserting the shape, not just the presence of the script, is the point: a
    loop or a second command reintroduces exactly the ordering defect the
    script exists to remove, and neither shows up in a passing run.
    """
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf8")
    body = makefile[makefile.index("define LINT_ACTIONS_CMD") :]
    body = body[: body.index("\nendef")]

    commands = [
        line.strip()
        for line in body.splitlines()[1:]
        if line.strip() and not line.strip().startswith("#")
    ]
    checks = [line for line in commands if line.startswith("$(call ensure_tool,")]
    remaining = [line for line in commands if line not in checks]

    assert len(remaining) == 1, (
        "lint-actions must run exactly one command besides its tool checks; "
        f"found {remaining}"
    )
    assert LINT_ACTIONS_SCRIPT in remaining[0], (
        f"the one command must invoke {LINT_ACTIONS_SCRIPT}: {remaining[0]}"
    )
    assert "$(YAMLLINT_VERSION)" in remaining[0], (
        "the yamllint pin must reach the script, or the gate floats between versions"
    )
    for forbidden in ("while ", "; do", "&&", "||"):
        assert forbidden not in remaining[0], (
            f"{forbidden!r} in the recipe reintroduces shell sequencing: {remaining[0]}"
        )


def test_test_lint_actions_is_reachable_from_the_aggregate_test_target() -> None:
    """`make test` must gather the lint-actions suite.

    The workflow names this target explicitly, so CI runs it either way, but a
    contributor running `make test` before pushing should get the same answer
    CI will give them. A suite reachable only from CI is one nobody runs until
    it is too late to be cheap.
    """
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf8")
    aggregate = re.search(r"(?m)^test:(.*)$", makefile)
    assert aggregate is not None, "the Makefile must declare a 'test' target"
    assert "test-lint-actions" in aggregate.group(1).split(), (
        "'test' must depend on test-lint-actions; reordering its other "
        "prerequisites is fine, dropping this one is not"
    )


def test_test_lint_actions_builds_its_own_environment() -> None:
    """The target must materialize a virtual environment, not layer one.

    cmd-mox's shim needs an interpreter that can import it, and the suite has
    only been stable under a materialized environment. Running it through
    `uv run --with`, the shape the other Python targets use, is what this
    assertion exists to prevent, because the failure there is a hang rather
    than a diagnosis.
    """
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf8")
    recipe = re.search(r"(?m)^test-lint-actions:.*\n((?:\t.*\n)+)", makefile)
    assert recipe is not None, "the Makefile must declare a test-lint-actions recipe"
    body = recipe.group(1)

    assert "uv venv" in body or "$(UV) venv" in body, (
        "test-lint-actions must materialize a virtual environment"
    )
    assert "--with" not in body, (
        "test-lint-actions must not run under a layered `uv run --with` "
        "environment; cmd-mox's shim hangs there rather than failing"
    )
