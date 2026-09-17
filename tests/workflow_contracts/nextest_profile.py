"""Reads which nextest profile a workflow step would run under.

nextest chooses its profile from ``--profile`` on the command line and,
failing that, from the ``NEXTEST_PROFILE`` environment variable; with
neither it runs ``profile.default``. The profile decides the slow
timeout, the global timeout and the test groups, so two lanes that run
the same tests under different profiles are not running the same thing.

The question this module answers is narrow: which scope, if any,
declares ``NEXTEST_PROFILE`` for a given step. GitHub resolves ``env``
innermost first, and an inner declaration masks an outer one whatever
its value, so the search stops at the first scope that declares the key
rather than the first that yields a non-empty value. Deciding on the
value instead reads a step's ``NEXTEST_PROFILE: ""`` as "no declaration
here" and reports the job's value, which is not the value nextest
receives.

Two spellings make that concrete. ``NEXTEST_PROFILE: ""`` parses to the
empty string and a valueless ``NEXTEST_PROFILE:`` parses to ``None``.
Both are declarations at that scope and both mask what is outside them.
A reader that guarded its walk with ``is not None`` would handle the
first and fall through on the second.

Nothing here opens a file. Parsed documents arrive as arguments, in the
same shape :mod:`coverage_lanes` takes them, so the query is pure and a
file that cannot be read fails at the boundary that read it.
"""

from __future__ import annotations

import typing as typ

from lane_fields import Node, mapping_of

#: The variable nextest reads when no ``--profile`` is passed.
PROFILE_VARIABLE: typ.Final[str] = "NEXTEST_PROFILE"

#: The scopes GitHub resolves ``env`` through, innermost first.
SCOPE_NAMES: typ.Final[tuple[str, ...]] = ("step", "job", "workflow")


class ProfileDeclaration(typ.NamedTuple):
    """Where ``NEXTEST_PROFILE`` is declared for one step, and as what.

    Attributes
    ----------
    scope : str or None
        ``"step"``, ``"job"`` or ``"workflow"`` for the innermost scope
        that declares the variable, or None when no scope declares it.
    value : object
        The value that scope declared, exactly as the YAML loader
        returned it, so an empty string and a valueless key stay
        distinguishable. None when no scope declares the variable, which
        is why :attr:`scope` rather than this field says whether one
        does.
    """

    scope: str | None
    value: object

    def is_absent(self) -> bool:
        """Return whether no scope declares the variable.

        Returns
        -------
        bool
            True when nextest would fall back to ``profile.default``
            because nothing sets the variable.

        Examples
        --------
        >>> ProfileDeclaration(None, None).is_absent()
        True
        >>> ProfileDeclaration("step", "").is_absent()
        False
        """
        return self.scope is None


def _environment(owner: object) -> Node | None:
    """Return one scope's ``env`` mapping, or None when it declares none.

    Parameters
    ----------
    owner : object
        A parsed step, job or document, or anything else.

    Returns
    -------
    Node or None
        The ``env`` mapping, or None when the scope is not a mapping or
        declares no ``env`` mapping.
    """
    scope = mapping_of(owner)
    return None if scope is None else mapping_of(scope.get("env"))


def declared_profile(document: object, job: object, step: object) -> ProfileDeclaration:
    """Return the innermost scope declaring ``NEXTEST_PROFILE`` for a step.

    Parameters
    ----------
    document : object
        The whole workflow document.
    job : object
        The enclosing job.
    step : object
        The step in question.

    Returns
    -------
    ProfileDeclaration
        The innermost declaring scope and its raw value, or a
        declaration whose scope is None when nothing declares it.

    Examples
    --------
    A step's declaration wins over the job's:

    >>> declared_profile(
    ...     {"env": {"NEXTEST_PROFILE": "slow"}},
    ...     {"env": {"NEXTEST_PROFILE": "ci"}},
    ...     {"env": {"NEXTEST_PROFILE": "quick"}},
    ... )
    ProfileDeclaration(scope='step', value='quick')

    An empty step declaration still masks the job's, because that is the
    value nextest receives:

    >>> declared_profile(
    ...     {}, {"env": {"NEXTEST_PROFILE": "ci"}}, {"env": {"NEXTEST_PROFILE": ""}}
    ... )
    ProfileDeclaration(scope='step', value='')

    A valueless key is a declaration too, and parses to None:

    >>> declared_profile(
    ...     {}, {"env": {"NEXTEST_PROFILE": "ci"}}, {"env": {"NEXTEST_PROFILE": None}}
    ... )
    ProfileDeclaration(scope='step', value=None)

    With no scope declaring it, the scope is None and nextest runs
    ``profile.default``:

    >>> declared_profile({}, {}, {"run": "cargo nextest run"})
    ProfileDeclaration(scope=None, value=None)
    """
    for name, owner in zip(SCOPE_NAMES, (step, job, document), strict=True):
        environment = _environment(owner)
        if environment is None or PROFILE_VARIABLE not in environment:
            continue
        return ProfileDeclaration(name, environment[PROFILE_VARIABLE])
    return ProfileDeclaration(None, None)
