"""Contract tests for the zero-tolerance TypeDoc documentation gate."""

from __future__ import annotations

import json
import re
import shutil
import subprocess  # noqa: S404 - the contract has to run the gate to observe it.
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_MANIFEST = PROJECT_ROOT / "package.json"
MAKEFILE = PROJECT_ROOT / "Makefile"
PNPM_LOCK = PROJECT_ROOT / "pnpm-lock.yaml"
BUN_LOCK = PROJECT_ROOT / "bun.lock"
FIXTURE_CONFIG = Path(__file__).parent / "fixtures" / "typedoc.json"

#: Every declaration kind TypeDoc can require documentation for on a
#: TypeScript surface. `validation.notDocumented` only inspects the kinds
#: named here, so an omission is a silent hole rather than a visible one: drop
#: `Function` and undocumented exported functions sail through a gate that
#: still reports itself as zero-tolerance.
TYPESCRIPT_KINDS = frozenset({
    "Enum",
    "EnumMember",
    "Variable",
    "Function",
    "Class",
    "Interface",
    "Property",
    "Method",
    "Accessor",
    "TypeAlias",
})

#: The tokens package is JavaScript, so the four kinds that cannot occur in it
#: are absent by necessity rather than by choice.
JAVASCRIPT_ONLY_ABSENT = frozenset({"Enum", "EnumMember", "Interface", "TypeAlias"})

TYPEDOC_CONFIGS = (
    pytest.param("frontend-pwa/typedoc.json", TYPESCRIPT_KINDS, id="frontend-pwa"),
    pytest.param("packages/types/typedoc.json", TYPESCRIPT_KINDS, id="packages-types"),
    pytest.param(
        "packages/tokens/typedoc.json",
        TYPESCRIPT_KINDS - JAVASCRIPT_ONLY_ABSENT,
        id="packages-tokens",
    ),
)

#: What each surface must look at, and what it must ignore. An entry point
#: silently narrowed is the cheapest way to make a zero-tolerance gate pass:
#: it examines less and reports the same verdict. The exclusions are asserted
#: too, because widening them has the same effect one directory at a time.
SURFACE_SCOPE = (
    pytest.param(
        "frontend-pwa/typedoc.json",
        ["src"],
        {
            "**/*.d.ts",
            "**/src/api/generated/**",
            "**/*.gen.*",
            "**/*.generated.*",
            "**/__generated__/**",
            "**/*.test.*",
            "**/tests/**",
            "**/fixtures/**",
        },
        id="frontend-pwa",
    ),
    pytest.param(
        "packages/types/typedoc.json",
        ["src"],
        {"**/*.d.ts", "**/dist/**"},
        id="packages-types",
    ),
    pytest.param(
        "packages/tokens/typedoc.json",
        ["build", "build-utils", "src/utils"],
        {"**/*.d.ts", "**/node_modules/**"},
        id="packages-tokens",
    ),
)
EXACT_VERSION = re.compile(r"^\d+\.\d+\.\d+$")

#: The validation object every surface must carry, compared whole rather than
#: key by key. TypeDoc defaults an unrecognized key to its own preference, so
#: a key introduced by a future release would otherwise arrive unreviewed.
EXPECTED_VALIDATION = {
    "notDocumented": True,
    "notExported": False,
    "invalidLink": True,
    "invalidPath": True,
    "rewrittenLink": True,
    "unusedMergeModuleWith": False,
}

EXPECTED_TYPEDOC_COMMANDS = [
    "typedoc --options frontend-pwa/typedoc.json",
    "typedoc --options packages/types/typedoc.json",
    "typedoc --options packages/tokens/typedoc.json",
]


def _resolve(executable: str) -> str:
    """Return the absolute path to ``executable``.

    Ruff rejects a bare program name in a subprocess call because PATH decides
    what runs. Resolving it here keeps the call explicit and fails with a
    readable message when the tool is missing rather than an OSError.
    """
    resolved = shutil.which(executable)
    assert resolved is not None, (
        f"{executable} must be installed to verify the documentation gate"
    )
    return resolved


def test_docs_check_runs_all_three_typedoc_configs() -> None:
    """The package script must validate every maintained TypeDoc surface."""
    manifest = json.loads(PACKAGE_MANIFEST.read_text(encoding="utf-8"))
    command = manifest["scripts"]["docs:check"]
    assert command.split(" && ") == EXPECTED_TYPEDOC_COMMANDS


def test_make_targets_keep_docs_check_in_the_repository_gate() -> None:
    """The Makefile must expose TypeDoc and retain it in the aggregate gate."""
    makefile = MAKEFILE.read_text(encoding="utf-8")
    aggregate = re.search(r"(?m)^all:(.*)$", makefile)
    assert aggregate is not None, "the Makefile must declare an 'all' target"
    assert "docs-check" in aggregate.group(1).split(), (
        "'all' must depend on docs-check; reordering its other prerequisites "
        "is fine, dropping this one is not"
    )

    target = re.search(r"(?m)^docs-check:(.*)$", makefile)
    assert target is not None, "the Makefile must declare a 'docs-check' target"
    assert "deps" in target.group(1).split(), (
        "docs-check must require deps; TypeDoc runs from the workspace "
        "install, so without it a clean checkout reports a missing binary "
        "rather than a documentation verdict"
    )

    completed = subprocess.run(  # noqa: S603 - a fixed, resolved local command.
        [_resolve("make"), "--dry-run", "docs-check"],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "pnpm run docs:check" in completed.stdout.splitlines()

    recipe = re.search(r"(?m)^docs-check:.*\n((?:\t.*\n)+)", makefile)
    assert recipe is not None, "the docs-check target must carry a recipe"
    prefixed = [
        line
        for line in recipe.group(1).splitlines()
        if line.lstrip("\t").startswith("-")
    ]
    assert prefixed == [], (
        "no recipe line may carry make's '-' ignore-errors prefix; under "
        f"the global .ONESHELL it would silently do nothing anyway: {prefixed}"
    )


def test_typedoc_rejects_an_undocumented_public_function() -> None:
    """The configured warning policy must fail on an undocumented declaration."""
    completed = subprocess.run(  # noqa: S603 - a fixed, resolved local command.
        [
            _resolve("pnpm"),
            "exec",
            "typedoc",
            "--options",
            str(FIXTURE_CONFIG),
        ],
        cwd=PROJECT_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    diagnostics = completed.stdout + completed.stderr
    assert completed.returncode != 0, diagnostics
    assert "undocumentedFixture" in diagnostics


def _typedoc_pin() -> str:
    """Return the root manifest's declared TypeDoc version."""
    manifest = json.loads(PACKAGE_MANIFEST.read_text(encoding="utf-8"))
    return manifest["devDependencies"]["typedoc"]


def test_typedoc_is_pinned_to_an_exact_version() -> None:
    """A range would let a TypeDoc minor change the gate's verdict silently."""
    pin = _typedoc_pin()
    assert EXACT_VERSION.fullmatch(pin), (
        f"typedoc must be pinned to an exact version, found {pin!r}"
    )


@pytest.mark.parametrize("lockfile", [PNPM_LOCK, BUN_LOCK], ids=["pnpm", "bun"])
def test_lockfiles_resolve_the_pinned_typedoc(lockfile: Path) -> None:
    """Both lockfiles must resolve TypeDoc to the version the manifest names.

    The match is anchored on a non-version character. An unanchored substring
    search reports success when the manifest and the lockfile disagree, since
    a pin of `0.28.2` is a prefix of the text `typedoc@0.28.20`, and that
    disagreement is the whole thing this contract exists to catch.
    """
    pin = _typedoc_pin()
    anchored = re.compile(rf"typedoc@{re.escape(pin)}(?![0-9.])")
    assert anchored.search(lockfile.read_text(encoding="utf-8")), (
        f"{lockfile.name} does not resolve typedoc@{pin} exactly"
    )


@pytest.mark.parametrize(("config_path", "expected_kinds"), TYPEDOC_CONFIGS)
def test_every_surface_enforces_the_zero_tolerance_policy(
    config_path: str, expected_kinds: frozenset[str]
) -> None:
    """Each surface must fail on an undocumented export and emit nothing.

    The fixture test proves TypeDoc honours this policy. This test proves the
    three real configurations actually set it, so a surface cannot quietly
    opt out by dropping a key or by shortening the list of kinds it applies
    to.
    """
    config = json.loads((PROJECT_ROOT / config_path).read_text(encoding="utf-8"))
    assert config["validation"] == EXPECTED_VALIDATION, (
        "the validation object is compared whole, so a key added by a future "
        "TypeDoc release has to be considered rather than inherited silently"
    )
    assert config["treatValidationWarningsAsErrors"] is True
    assert config["treatWarningsAsErrors"] is True, (
        "an unknown block tag is a warning rather than a validation warning, "
        "so only this key turns a misspelled tag name into a failure"
    )
    assert config["emit"] == "none"
    required = frozenset(config["requiredToBeDocumented"])
    assert required == expected_kinds, (
        "requiredToBeDocumented decides which declaration kinds the gate can "
        "see, so the set is asserted whole rather than merely non-empty; "
        f"missing={sorted(expected_kinds - required)} "
        f"unexpected={sorted(required - expected_kinds)}"
    )


@pytest.mark.parametrize(("config_path", "entry_points", "exclude"), SURFACE_SCOPE)
def test_every_surface_looks_at_what_it_claims_to(
    config_path: str, entry_points: list[str], exclude: set[str]
) -> None:
    """Each surface's entry points and exclusions are pinned.

    A gate that examines nothing passes. Narrowing an entry point, or adding a
    pattern to `exclude`, is the cheapest way to make this one green without
    documenting anything, and neither shows up in a verdict.
    """
    config = json.loads((PROJECT_ROOT / config_path).read_text(encoding="utf-8"))
    assert config["entryPoints"] == entry_points
    assert config["entryPointStrategy"] == "expand", (
        "'expand' is what walks a directory; another strategy would treat the "
        "entry point as a single module and examine far less"
    )
    assert set(config["exclude"]) == exclude, (
        "exclusions are compared whole: each one removes declarations from the "
        "gate's view and needs a reviewed reason"
    )
