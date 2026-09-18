"""Evaluates the `if` expressions the suite lanes are guarded by.

The lane contracts pin those expressions verbatim and assert one against
the other, which catches a narrowed condition but says nothing about
what the pair *does*. Two conditions can each be the reviewed string and
still leave an actor with no lane, because the question a reader wants
answered is not "are these the agreed strings" but "for this actor on
this event, does the suite run anywhere". That question needs the
expressions evaluated rather than compared.

The grammar here is deliberately tiny: `&&`-joined comparisons of a
`github.<field>` reference against a single-quoted literal, with `==` or
`!=`. That is exactly what the three lanes use. Anything else raises,
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
_COMPARISON = re.compile(
    r"\Agithub\.(?P<field>[a-z_]+)\s*(?P<operator>==|!=)\s*'(?P<literal>[^']*)'\Z"
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
        The `github` fields the expression may reference, such as
        `actor` and `event_name`.

    Returns
    -------
    bool
        Whether the guarded job or step would run.

    Raises
    ------
    ConditionSyntaxError
        If the expression is outside the supported grammar, or names a
        `github` field the context does not supply.

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
    """
    if expression is None:
        return True
    return all(
        _holds(term.strip(), context, expression) for term in expression.split("&&")
    )


def _holds(term: str, context: dict[str, str], whole: str) -> bool:
    """Return whether one comparison holds.

    Parameters
    ----------
    term : str
        One `&&`-separated comparison.
    context : dict[str, str]
        The `github` fields available.
    whole : str
        The expression the term came from, for the message.

    Returns
    -------
    bool
        The comparison's value.

    Raises
    ------
    ConditionSyntaxError
        If the term is not a supported comparison, or references a field
        the context does not supply.
    """
    match = _COMPARISON.match(term)
    if match is None:
        message = (
            f"{whole!r}: the term {term!r} is outside the grammar these "
            "contracts evaluate, which is `&&`-joined `github.<field>` "
            "comparisons against single-quoted literals"
        )
        raise ConditionSyntaxError(message)
    field = match["field"]
    if field not in context:
        message = (
            f"{whole!r}: no value for github.{field}; the contract must "
            "supply every field its lanes' conditions reference"
        )
        raise ConditionSyntaxError(message)
    if match["operator"] == "==":
        return context[field] == match["literal"]
    return context[field] != match["literal"]
