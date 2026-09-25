"""Evaluates the `if` expressions the suite lanes are guarded by.

The lane contracts pin those expressions verbatim and assert one against
the other, which catches a narrowed condition but says nothing about
what the pair *does*. Two conditions can each be the reviewed string and
still leave an actor with no lane, because the question a reader wants
answered is not "are these the agreed strings" but "for this actor on
this event, does the suite run anywhere". That question needs the
expressions evaluated rather than compared.

The grammar here is deliberately tiny: `&&`-joined comparisons of a
`github.<field>` or `steps.<id>.outputs.<name>` reference against a
single-quoted literal, with `==` or `!=`. That is exactly what the three
lanes and the CodeScene publisher's upload use. Anything else raises,
and raising is the point. A partial evaluator that returned False for an
expression it could not parse would report a lane as unreachable and
make the very contract that depends on it pass for the wrong reason, so
an expression outside the grammar has to stop the contract and be read
by a person.

Nothing here opens a file, and nothing knows what a lane is.
"""

from __future__ import annotations

import re

#: One comparison: a context reference, an operator, a quoted literal.
#: A `github` field is keyed by its bare name, as `actor`; a step output by
#: its whole reference, as `steps.check.outputs.available`, since two steps
#: can publish outputs of the same name.
_COMPARISON = re.compile(
    r"\A(?:github\.(?P<field>[a-z_]+)"
    r"|(?P<output>steps\.[A-Za-z_][A-Za-z0-9_-]*\.outputs\.[A-Za-z_][A-Za-z0-9_-]*))"
    r"\s*(?P<operator>==|!=)\s*'(?P<literal>[^']*)'\Z"
)


class ConditionSyntaxError(ValueError):
    """Raised when an `if` expression is outside the supported grammar.

    A shape fault rather than a False verdict, because a lane wrongly
    read as unreachable is a contract passing for the wrong reason. The
    message quotes the expression, so an author extending a condition
    sees both what was written and that this evaluator has to learn it.
    """


def evaluate(expression: str | None, context: dict[str, str]) -> bool:
    """Return whether an `if` expression holds in one context.

    Parameters
    ----------
    expression : str or None
        The condition, or None for a job or step that declares none.
        GitHub runs an unconditional step, so None is True.
    context : dict[str, str]
        The values the expression may reference: `github` fields by bare
        name, such as `actor` and `event_name`, and step outputs by their
        whole reference, such as `steps.check.outputs.available`.

    Returns
    -------
    bool
        Whether the guarded job or step would run.

    Raises
    ------
    ConditionSyntaxError
        If the expression is outside the supported grammar, or names a
        value the context does not supply.

    Examples
    --------
    >>> context = {"actor": "dependabot[bot]", "event_name": "pull_request"}
    >>> evaluate("github.actor == 'dependabot[bot]'", context)
    True
    >>> evaluate(
    ...     "github.actor != 'dependabot[bot]' && github.event_name != 'push'",
    ...     context,
    ... )
    False
    >>> evaluate(None, context)
    True
    >>> evaluate(
    ...     "steps.check.outputs.available == 'true'",
    ...     {"steps.check.outputs.available": "false"},
    ... )
    False
    """
    if expression is None:
        return True
    # Every term is evaluated before any result is combined. `all()` over a
    # generator stops at the first false comparison, so a later term outside
    # the grammar would never reach `_holds` and never raise, and a lane
    # guarded by an expression this evaluator cannot read would be reported
    # as simply not running. That is the failure the raise exists to
    # prevent, so the short circuit has to go.
    verdicts = [
        _holds(term.strip(), context, expression) for term in expression.split("&&")
    ]
    return all(verdicts)


def _holds(term: str, context: dict[str, str], whole: str) -> bool:
    """Return whether one comparison holds.

    Parameters
    ----------
    term : str
        One `&&`-separated comparison.
    context : dict[str, str]
        The `github` fields and step outputs available.
    whole : str
        The expression the term came from, for the message.

    Returns
    -------
    bool
        The comparison's value.

    Raises
    ------
    ConditionSyntaxError
        If the term is not a supported comparison, or references a value
        the context does not supply.
    """
    match = _COMPARISON.match(term)
    if match is None:
        message = (
            f"{whole!r}: the term {term!r} is outside the grammar these "
            "contracts evaluate, which is `&&`-joined `github.<field>` or "
            "`steps.<id>.outputs.<name>` comparisons against single-quoted "
            "literals"
        )
        raise ConditionSyntaxError(message)
    key = match["field"] or match["output"]
    if key not in context:
        reference = f"github.{key}" if match["field"] else key
        message = (
            f"{whole!r}: no value for {reference}; the contract must "
            "supply every value its conditions reference"
        )
        raise ConditionSyntaxError(message)
    if match["operator"] == "==":
        return context[key] == match["literal"]
    return context[key] != match["literal"]
