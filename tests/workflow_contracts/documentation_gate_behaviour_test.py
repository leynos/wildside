"""Behavioural contracts for the TypeDoc documentation gate.

The static contracts in `documentation_gate_test.py` prove the gate is
configured. They cannot prove it catches anything: a configuration can name
every key the reviewer expects and still let an undocumented export through,
because what each key actually does is TypeDoc's business, not the
configuration's.

These tests run the real TypeDoc against the repository's own
`frontend-pwa/typedoc.json`, overriding only the entry point, the TypeScript
configuration and the project name, over a fixture in a temporary directory.
A documented fixture must pass; a missing comment, a link to a symbol that
does not exist, and an unknown block tag must each fail. Clearing the key
responsible must let exactly that one case through, which is what ties each
key to the defect it catches rather than to a reviewer's expectation of it.
"""

from __future__ import annotations

import json
import subprocess  # noqa: S404 - the contract has to run TypeDoc to observe it.
import typing as typ
from pathlib import Path
from shutil import which

import pytest

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc

PROJECT_ROOT = Path(__file__).resolve().parents[2]
BASE_CONFIG = PROJECT_ROOT / "frontend-pwa" / "typedoc.json"

#: The TypeScript configuration the fixture compiles under. It mirrors the
#: front-end's compiler options closely enough for a single-function module;
#: the gate's verdict comes from TypeDoc's validation, not from type checking.
FIXTURE_TSCONFIG = {
    "compilerOptions": {
        "target": "ES2022",
        "module": "ESNext",
        "moduleResolution": "Bundler",
        "strict": True,
        "noEmit": True,
        "skipLibCheck": True,
    },
    "include": ["probe.ts"],
}

DOCUMENTED = "/** A documented export. */\nexport function probeFn(): void {}\n"
UNDOCUMENTED = "export function probeFn(): void {}\n"
BROKEN_LINK = "/** See {@link noSuchThing}. */\nexport function probeFn(): void {}\n"
UNKNOWN_TAG = (
    "/**\n * A documented export.\n * @notARealTag value\n */\n"
    "export function probeFn(): void {}\n"
)

#: Each defect, the fixture that exhibits it, and the fragment TypeDoc's
#: diagnostic must contain. Matching the message as well as the exit status
#: stops a fixture that fails for an unrelated reason, a syntax error or a
#: missing dependency, from reading as proof the gate works.
#: The defect fixtures alone, for iterating over "every defect except this
#: one". Reading them back out of the parameter set would lose their type.
DEFECT_SOURCES: tuple[str, ...] = (UNDOCUMENTED, BROKEN_LINK, UNKNOWN_TAG)

DEFECTS = (
    pytest.param(UNDOCUMENTED, "does not have any documentation", id="undocumented"),
    pytest.param(BROKEN_LINK, "Failed to resolve link", id="broken-link"),
    pytest.param(UNKNOWN_TAG, "unknown block tag", id="unknown-block-tag"),
)

#: The measured relationship between each configuration key and the defect it
#: catches. Clearing a key must let exactly its own fixture through and leave
#: the other two failing, so no key can be dropped without a test going red.
#:
#: `treatValidationWarningsAsErrors` is deliberately absent. It is subsumed by
#: `treatWarningsAsErrors`, which promotes every warning including the
#: validation ones, so clearing the narrower key alone changes nothing while
#: the broader one is set. It stays in the configurations as the key that
#: carries the intent on its own, but it is not load-bearing today and this
#: contract does not pretend otherwise.
KEY_TO_DEFECT = (
    pytest.param(("validation", "notDocumented"), UNDOCUMENTED, id="notDocumented"),
    pytest.param(("validation", "invalidLink"), BROKEN_LINK, id="invalidLink"),
    pytest.param(("treatWarningsAsErrors",), UNKNOWN_TAG, id="treatWarningsAsErrors"),
)


def _typedoc() -> str:
    """Return the absolute path to the installed TypeDoc binary."""
    resolved = which("typedoc", path=str(PROJECT_ROOT / "node_modules" / ".bin"))
    assert resolved is not None, (
        "TypeDoc must be installed to run these contracts; run `make deps`"
    )
    return resolved


def _write_project(
    directory: Path, source: str, *, cleared: cabc.Sequence[str] = ()
) -> Path:
    """Write a fixture project and return its TypeDoc configuration path.

    The configuration is the repository's own, so a change to the real gate is
    a change to what these tests measure. Only the entry point, the TypeScript
    configuration and the project name are overridden, and optionally one
    validation key is cleared.
    """
    config = json.loads(BASE_CONFIG.read_text(encoding="utf-8"))
    config.pop("$schema", None)
    config["entryPoints"] = ["probe.ts"]
    config["tsconfig"] = "tsconfig.json"
    config["name"] = "probe"
    if cleared:
        target = config
        for key in cleared[:-1]:
            target = target[key]
        assert target[cleared[-1]] is True, (
            f"{'.'.join(cleared)} must be enabled before the mutation clears it"
        )
        target[cleared[-1]] = False

    directory.mkdir(parents=True, exist_ok=True)
    (directory / "probe.ts").write_text(source, encoding="utf-8")
    (directory / "tsconfig.json").write_text(
        json.dumps(FIXTURE_TSCONFIG), encoding="utf-8"
    )
    config_path = directory / "typedoc.json"
    config_path.write_text(json.dumps(config, indent=2), encoding="utf-8")
    return config_path


def _run(config_path: Path) -> subprocess.CompletedProcess[str]:
    """Run TypeDoc over a fixture configuration."""
    return subprocess.run(  # noqa: S603 - a resolved binary over a scratch project.
        [_typedoc(), "--options", str(config_path)],
        cwd=config_path.parent,
        capture_output=True,
        text=True,
        check=False,
        timeout=180,
    )


def test_a_documented_export_passes_and_writes_nothing(tmp_path: Path) -> None:
    """A fully documented fixture passes, and `emit: "none"` is honoured.

    The second half matters as much as the first: a gate that quietly wrote a
    documentation tree into the working copy would show up as untracked files
    on every contributor's machine.
    """
    config_path = _write_project(tmp_path, DOCUMENTED)

    completed = _run(config_path)

    assert completed.returncode == 0, (
        f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
    )
    written = sorted(path.name for path in tmp_path.iterdir())
    assert written == ["probe.ts", "tsconfig.json", "typedoc.json"], (
        f"TypeDoc emitted files the test did not write: {written}"
    )


@pytest.mark.parametrize(("source", "expected"), DEFECTS)
def test_each_documentation_defect_fails_the_gate(
    tmp_path: Path, source: str, expected: str
) -> None:
    """Each defect must fail, with a diagnostic naming it."""
    config_path = _write_project(tmp_path, source)

    completed = _run(config_path)

    diagnostics = completed.stdout + completed.stderr
    assert completed.returncode != 0, diagnostics
    assert expected in diagnostics, (
        f"expected a diagnostic containing {expected!r}; got {diagnostics!r}"
    )


@pytest.mark.parametrize(("cleared", "source"), KEY_TO_DEFECT)
def test_clearing_a_key_admits_exactly_its_own_defect(
    tmp_path: Path, cleared: tuple[str, ...], source: str
) -> None:
    """Clearing one key lets its defect through and leaves the others failing.

    Without this the previous test proves only that the fixtures fail, not
    that any particular key is why. Pinning each key to one defect is what
    stops a key being dropped from the configuration unnoticed.
    """
    admitted = _run(_write_project(tmp_path / "admitted", source, cleared=cleared))
    assert admitted.returncode == 0, (
        f"clearing {'.'.join(cleared)} should admit this fixture; "
        f"stdout={admitted.stdout!r} stderr={admitted.stderr!r}"
    )

    others = [other for other in DEFECT_SOURCES if other != source]
    for index, other_source in enumerate(others):
        still_failing = _run(
            _write_project(tmp_path / f"other{index}", other_source, cleared=cleared)
        )
        assert still_failing.returncode != 0, (
            f"clearing {'.'.join(cleared)} also admitted an unrelated defect"
        )
