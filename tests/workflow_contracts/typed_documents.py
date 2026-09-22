"""Typed boundary parsers for the JSON and YAML documents contracts read.

`json.loads` and `yaml.safe_load` both hand back `Any`. Every nested access
below one of them is therefore unchecked: a renamed key, a string where a list
belongs, or a document that is not a mapping at all reaches the assertion as an
opaque value and fails with `TypeError: string indices must be integers`
instead of with the contract's own message. Worse, the type checker cannot see
any of it, so a contract can drift from the document it claims to read without
a single diagnostic.

These parsers do the decoding once, validate the shape at the boundary, and
return `TypedDict`s the checker can follow. Nothing here uses `cast`: a cast
asserts a shape the runtime never checked, which is the same hole in a
different spelling.

Two shapes are deliberately kept whole rather than narrowed:

- A TypeDoc `validation` object is returned as a mapping, because
  `documentation_gate_test` compares it against the expected object entire.
  Narrowing it to the keys known today would silently drop a key a future
  TypeDoc release introduces, which is exactly the review the whole-object
  comparison exists to force.
- Workflow jobs and steps are returned as validated mappings rather than
  closed records, because the contracts assert on the *absence* of `if` and
  `continue-on-error` keys. A record built from a fixed key list would answer
  that question about the parser rather than about the workflow.
"""

from __future__ import annotations

import json
import typing as typ

import strict_yaml

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    from pathlib import Path

type JsonValue = (
    str | int | float | bool | list[JsonValue] | dict[str, JsonValue] | None
)

#: YAML 1.1 parses bare `on`, `off`, `yes` and `no` keys as booleans, so a
#: workflow that writes `on:` unquoted arrives with `True` where a reader
#: expects the string. The booleans are restored to their source spelling at
#: the boundary so no caller has to know; `yes` and `no` are absent because
#: PyYAML resolves all four to the same two booleans and a workflow key is
#: only ever spelled `on` or `off`.
_YAML_BOOLEAN_KEYS = {True: "on", False: "off"}


class TypeDocConfig(typ.TypedDict):
    """One TypeDoc surface's configuration, keyed as TypeDoc spells it.

    Every key is required. A configuration missing one is not a surface
    running on a TypeDoc default, it is a surface whose gate was edited
    without review, so the parser fails rather than reporting a default.
    """

    validation: dict[str, JsonValue]
    treatValidationWarningsAsErrors: bool
    treatWarningsAsErrors: bool
    emit: str
    requiredToBeDocumented: list[str]
    entryPoints: list[str]
    entryPointStrategy: str
    exclude: list[str]


class PackageManifest(typ.TypedDict):
    """The root package manifest's scripts and development dependencies."""

    scripts: dict[str, str]
    devDependencies: dict[str, str]


class Workflow(typ.TypedDict):
    """A GitHub Actions workflow reduced to the parts contracts assert on.

    `defaults` is carried even though most workflows omit it, because a
    workflow-level `defaults.run` reaches every `run` step in every job. A
    contract that read only the job and the step would miss a shell template
    or a working directory imposed from the top of the file.

    Attributes
    ----------
    triggers : dict[str, JsonValue]
        The workflow's `on` mapping, with the key restored to its source
        spelling.
    jobs : dict[str, JsonValue]
        The workflow's `jobs` mapping, one entry per job.
    defaults : JsonValue
        The workflow's `defaults` value, or None when it declares none.
        It is deliberately not narrowed here: a contract asserts on the
        absence of `shell` and `working-directory` under it, and a parser
        that closed the shape would answer that about itself rather than
        about the workflow. `None` therefore means the key is absent, and
        a `defaults` that is not a mapping fails where it is read.
    """

    triggers: dict[str, JsonValue]
    jobs: dict[str, JsonValue]
    defaults: JsonValue


class DocumentShapeError(TypeError):
    """A decoded document does not match the shape the contracts expect."""


def _shape_error(description: str, expected: str, value: object) -> typ.NoReturn:
    """Raise a `DocumentShapeError` naming the site, the shape and the value."""
    message = f"{description} must be {expected}, found {value!r}"
    raise DocumentShapeError(message)


def as_mapping(value: JsonValue, description: str) -> dict[str, JsonValue]:
    """Return ``value`` as a mapping, failing when it is anything else."""
    if not isinstance(value, dict):
        _shape_error(description, "a mapping", value)
    return value


def as_list(value: JsonValue, description: str) -> list[JsonValue]:
    """Return ``value`` as a list, failing when it is anything else."""
    if not isinstance(value, list):
        _shape_error(description, "a list", value)
    return value


def as_text(value: JsonValue, description: str) -> str:
    """Return ``value`` as a string, failing when it is anything else."""
    if not isinstance(value, str):
        _shape_error(description, "a string", value)
    return value


def as_flag(value: JsonValue, description: str) -> bool:
    """Return ``value`` as a boolean, failing when it is anything else.

    The check is against `bool` rather than truthiness. TypeDoc reads `1` and
    `"true"` as enabling a validation key, so a contract that accepted them
    would report a surface as configured while the reviewed spelling had been
    replaced by one nobody chose.
    """
    if not isinstance(value, bool):
        _shape_error(description, "a boolean", value)
    return value


def as_text_list(value: JsonValue, description: str) -> list[str]:
    """Return ``value`` as a list of strings."""
    return [
        as_text(entry, f"every entry in {description}")
        for entry in as_list(value, description)
    ]


def as_text_mapping(value: JsonValue, description: str) -> dict[str, str]:
    """Return ``value`` as a mapping of strings to strings."""
    return {
        key: as_text(entry, f"{description}[{key!r}]")
        for key, entry in as_mapping(value, description).items()
    }


def field(document: dict[str, JsonValue], key: str, description: str) -> JsonValue:
    """Return ``document[key]``, failing with the document's name when absent."""
    if key not in document:
        message = f"{description} must declare {key!r}"
        raise DocumentShapeError(message)
    return document[key]


def _decoded(value: object, description: str) -> JsonValue:
    """Convert a freshly decoded document into a checked `JsonValue` tree.

    A decoder returns `Any`, so this walk is where the document stops being
    unchecked. Keys are validated too: an unexpected key type is raised rather
    than dropped, because a parser that quietly discards part of a document
    reports on something other than the file on disk.
    """
    if value is None or isinstance(value, (str, bool, int, float)):
        return value
    if isinstance(value, list):
        return [_decoded(entry, f"{description} entry") for entry in value]
    if isinstance(value, dict):
        return {
            _decoded_key(key, description): _decoded(entry, f"{description} value")
            for key, entry in value.items()
        }
    _shape_error(description, "a JSON-compatible value", value)


def _decoded_key(key: object, description: str) -> str:
    """Return a mapping key as its source spelling."""
    if isinstance(key, bool):
        return _YAML_BOOLEAN_KEYS[key]
    if isinstance(key, str):
        return key
    _shape_error(f"every key in {description}", "a string", key)


def load_json_document(path: Path, label: str) -> dict[str, JsonValue]:
    """Return a JSON file as a checked mapping."""
    decoded = _decoded(json.loads(path.read_text(encoding="utf-8")), label)
    return as_mapping(decoded, label)


def load_yaml_document(path: Path, label: str) -> dict[str, JsonValue]:
    """Return a YAML file as a checked mapping."""
    decoded = _decoded(strict_yaml.load(path.read_text(encoding="utf-8")), label)
    return as_mapping(decoded, label)


def load_package_manifest(path: Path) -> PackageManifest:
    """Return the package manifest's scripts and development dependencies."""
    label = path.name
    document = load_json_document(path, label)
    return PackageManifest(
        scripts=as_text_mapping(
            field(document, "scripts", label), f"{label} 'scripts'"
        ),
        devDependencies=as_text_mapping(
            field(document, "devDependencies", label), f"{label} 'devDependencies'"
        ),
    )


def load_typedoc_config(path: Path, label: str) -> TypeDocConfig:
    """Return one TypeDoc surface's configuration as a checked record."""
    document = load_json_document(path, label)
    return TypeDocConfig(
        validation=as_mapping(
            field(document, "validation", label), f"{label} 'validation'"
        ),
        treatValidationWarningsAsErrors=as_flag(
            field(document, "treatValidationWarningsAsErrors", label),
            f"{label} 'treatValidationWarningsAsErrors'",
        ),
        treatWarningsAsErrors=as_flag(
            field(document, "treatWarningsAsErrors", label),
            f"{label} 'treatWarningsAsErrors'",
        ),
        emit=as_text(field(document, "emit", label), f"{label} 'emit'"),
        requiredToBeDocumented=as_text_list(
            field(document, "requiredToBeDocumented", label),
            f"{label} 'requiredToBeDocumented'",
        ),
        entryPoints=as_text_list(
            field(document, "entryPoints", label), f"{label} 'entryPoints'"
        ),
        entryPointStrategy=as_text(
            field(document, "entryPointStrategy", label),
            f"{label} 'entryPointStrategy'",
        ),
        exclude=as_text_list(field(document, "exclude", label), f"{label} 'exclude'"),
    )


def load_workflow(path: Path, label: str) -> Workflow:
    """Return a workflow's triggers, jobs and defaults as checked values.

    `on` and `jobs` are required, and a workflow missing either fails with
    `label` naming the file. `defaults` is optional, so an absent key is
    returned as None rather than raised on.

    Parameters
    ----------
    path : Path
        The workflow file to read.
    label : str
        The name the failure messages call the file.

    Returns
    -------
    Workflow
        The triggers, the jobs, and the `defaults` value or None.

    Examples
    --------
    The example reads a file, so it is shown rather than run.

    >>> workflow = load_workflow(  # doctest: +SKIP
    ...     Path(".github/workflows/ci.yml"), "ci.yml"
    ... )
    >>> sorted(workflow)  # doctest: +SKIP
    ['defaults', 'jobs', 'triggers']
    >>> workflow["defaults"] is None  # doctest: +SKIP
    True
    """
    document = load_yaml_document(path, label)
    return Workflow(
        triggers=as_mapping(field(document, "on", label), f"{label} triggers"),
        jobs=as_mapping(field(document, "jobs", label), f"{label} jobs"),
        defaults=document.get("defaults"),
    )


def workflow_job(workflow: Workflow, job_name: str, label: str) -> dict[str, JsonValue]:
    """Return one job's mapping, failing when the workflow does not declare it."""
    jobs = workflow["jobs"]
    return as_mapping(
        field(jobs, job_name, f"{label} jobs"), f"{label} job {job_name!r}"
    )


def job_steps(
    job: dict[str, JsonValue], description: str
) -> list[dict[str, JsonValue]]:
    """Return one job's steps, each checked to be a mapping."""
    return [
        as_mapping(step, f"{description} step {index}")
        for index, step in enumerate(
            as_list(field(job, "steps", description), f"{description} steps")
        )
    ]
