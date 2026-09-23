"""Follows reusable-workflow calls, so a lane is read as a closure.

A workflow declaring only `workflow_call` has no trigger a pull request
intersects, yet it runs on every pull request whose workflow calls it,
and `secrets: inherit` hands it the caller's secrets. A contract that
enumerates workflows by trigger alone cannot see it, so every absence
built on that enumeration passes over it while it does the forbidden
thing. episodic measured exactly that: a called probe curling the
CodeScene project API with an inherited token passed every clause.

A call is local when its reference, less one of the two same-repository
prefixes GitHub documents, names a file directly under
`.github/workflows/`. The prefixes are `./`, which is workspace-relative,
and `$/`, the self-repository form GitHub.com recommends; a reader
knowing only one drops callers written the other way. A call to another
repository is not
followed, because its content is not in this tree; which secrets may be
handed to one is the contract's question, answered with
:func:`inherits_into_other_repositories`.

Everything here reads parsed documents keyed by file name, so the
contracts can drive shapes the repository does not have.
"""

from __future__ import annotations

import typing as typ
from pathlib import PurePosixPath

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc

#: Where GitHub looks for a same-repository reusable workflow. It does not
#: look in subdirectories.
WORKFLOWS_PREFIX: typ.Final[str] = ".github/workflows/"
WORKFLOWS_DIRECTORY: typ.Final[PurePosixPath] = PurePosixPath(".github/workflows")

#: The prefixes GitHub documents for a same-repository call.
SELF_REPOSITORY_PREFIXES: typ.Final[tuple[str, ...]] = ("./", "$/")

#: The `secrets:` value that forwards every secret the caller holds.
INHERIT_ALL_SECRETS: typ.Final[str] = "inherit"


def _as_mapping(value: object) -> cabc.Mapping[object, object]:
    """Return ``value`` as a mapping, or an empty one when it is not one."""
    # The cast says only what the check established: a mapping whose keys
    # and values are unconstrained, which is how every caller treats it.
    if isinstance(value, dict):
        return typ.cast("cabc.Mapping[object, object]", value)
    return {}


class UnresolvedWorkflowCallError(LookupError):
    """Raised when a local call names a workflow the reading does not hold.

    The closure cannot vouch for a workflow it never read, so a call it
    cannot resolve fails the reading rather than dropping out of the lane.
    """


def local_workflow_name(reference: str) -> str | None:
    """Return the workflow file a same-repository call names, or None.

    Parameters
    ----------
    reference : str
        A job's `uses:` value.

    Returns
    -------
    str or None
        The workflow's file name when the reference, less a leading
        `./` or `$/`, names a file directly under `.github/workflows/`.

    Raises
    ------
    UnresolvedWorkflowCallError
        If the reference names `.github/workflows/` with neither prefix,
        a form GitHub does not document. Reading it as another
        repository's call would drop its callee from the lane silently.

    Examples
    --------
    >>> local_workflow_name("./.github/workflows/release.yml")
    'release.yml'
    >>> local_workflow_name("$/.github/workflows/release.yml")
    'release.yml'
    >>> local_workflow_name("owner/repo/.github/workflows/release.yml@main") is None
    True
    """
    prefix = next(
        (p for p in SELF_REPOSITORY_PREFIXES if reference.startswith(p)), None
    )
    if prefix is None:
        if reference.startswith(WORKFLOWS_PREFIX):
            message = f"{reference} names a local workflow without `./` or `$/`"
            raise UnresolvedWorkflowCallError(message)
        return None
    path = PurePosixPath(reference.removeprefix(prefix))
    return path.name if path.parent == WORKFLOWS_DIRECTORY else None


def called_workflows(document: cabc.Mapping[str, object]) -> list[tuple[str, str]]:
    """Return the job name and reference of every reusable-workflow call.

    Only a job calls a reusable workflow; a step's `uses:` names an
    action and is not read here.

    Parameters
    ----------
    document : Mapping[str, object]
        One parsed workflow.

    Returns
    -------
    list[tuple[str, str]]
        One entry per job whose `uses:` is a string, in declaration order.

    Examples
    --------
    >>> called_workflows({"jobs": {"call": {"uses": "./.github/workflows/x.yml"}}})
    [('call', './.github/workflows/x.yml')]
    """
    jobs = _as_mapping(document.get("jobs"))
    return [
        (str(name), uses)
        for name, job in jobs.items()
        if isinstance(uses := _as_mapping(job).get("uses"), str)
    ]


def reachable_workflows(
    documents: cabc.Mapping[str, cabc.Mapping[str, object]],
    entries: cabc.Iterable[str],
) -> frozenset[str]:
    """Return the entry workflows and every workflow they call, transitively.

    Parameters
    ----------
    documents : Mapping[str, Mapping[str, object]]
        Every workflow, keyed by file name.
    entries : Iterable[str]
        The file names the traversal starts from.

    Returns
    -------
    frozenset[str]
        The file names reached.

    Raises
    ------
    UnresolvedWorkflowCallError
        If an entry or a local call names a file `documents` does not hold.

    Examples
    --------
    >>> call = {"jobs": {"call": {"uses": "./.github/workflows/b.yml"}}}
    >>> sorted(reachable_workflows({"a.yml": call, "b.yml": {}}, ["a.yml"]))
    ['a.yml', 'b.yml']
    """
    pending = list(entries)
    reached: set[str] = set()
    while pending:
        current = pending.pop()
        if current in reached:
            continue
        if current not in documents:
            message = f"{current} is called or named but was not read"
            raise UnresolvedWorkflowCallError(message)
        reached.add(current)
        pending.extend(_local_calls(documents[current]) - reached)
    return frozenset(reached)


def _local_calls(document: cabc.Mapping[str, object]) -> frozenset[str]:
    """Return the file names of the same-repository workflows one calls."""
    names = (local_workflow_name(ref) for _, ref in called_workflows(document))
    return frozenset(name for name in names if name is not None)


def inherits_into_other_repositories(
    document: cabc.Mapping[str, object],
) -> list[str]:
    """Return the jobs that hand every secret to another repository's workflow.

    A local call passing `secrets: inherit` is not listed: the closure
    reads its callee, so whatever that workflow does with a secret is
    checked there. A call to another repository cannot be read, so
    inheriting into one hands the credential to code no contract sees.

    Parameters
    ----------
    document : Mapping[str, object]
        One parsed workflow.

    Returns
    -------
    list[str]
        The offending job names, in declaration order.

    Examples
    --------
    >>> remote = {"uses": "owner/repo/.github/workflows/x.yml@v1", "secrets": "inherit"}
    >>> local = {"uses": "./.github/workflows/x.yml", "secrets": "inherit"}
    >>> inherits_into_other_repositories({"jobs": {"a": remote, "b": local}})
    ['a']
    """
    # Each job is read under its own key, not the name `called_workflows`
    # reports. A bare `on:` job key parses to `True`, which that name renders
    # as "True", and a lookup by the rendered name would miss the job.
    jobs = _as_mapping(document.get("jobs"))
    return [
        str(name) for name, job in jobs.items() if _inherits_elsewhere(_as_mapping(job))
    ]


def _inherits_elsewhere(job: cabc.Mapping[object, object]) -> bool:
    """Return whether one job passes every secret to another repository."""
    uses = job.get("uses")
    return (
        isinstance(uses, str)
        and local_workflow_name(uses) is None
        and job.get("secrets") == INHERIT_ALL_SECRETS
    )
