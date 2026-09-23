"""Reads the labels a ``runs-on`` expression can evaluate to, or nothing.

An expression is read as GitHub evaluates it. `||` returns its first truthy
operand and `&&` its last, so each top-level disjunct can yield only its final
conjunct. Every such result must be a quoted label. The last disjunct must be
a bare label, because a falsy condition there would itself be the result.
Anything else could select a label no literal names: in
``${{ matrix.os || 'ubuntu-latest' }}``, ``matrix.os`` may be a paid label.
So the reading answers None rather than the literals the text happens to
contain.

Split from ``runner_shapes`` so each module stays simple enough to read at a
glance; ``runner_shapes`` owns the three ``runs-on`` forms and the refusal,
this module only the expression grammar.
"""

from __future__ import annotations

import re
import typing as typ

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc

#: A whole ``runs-on`` expression. Text around the braces would make the
#: label a concatenation this reader does not evaluate, so it must not match.
_EXPRESSION = re.compile(r"\$\{\{(?P<body>.*)\}\}", re.DOTALL)

#: A single-quoted label, the only operand accepted as a result.
_LABEL = re.compile(r"'(?P<label>[^']*)'")


def expression_labels(runner: str) -> frozenset[str] | None:
    """Return every label an expression can evaluate to, or None.

    Parameters
    ----------
    runner : str
        A ``runs-on`` value containing ``${{``.

    Returns
    -------
    frozenset[str] or None
        The labels, or None when any possible result is not a quoted label,
        or when text surrounds the expression.

    Examples
    --------
    >>> sorted(expression_labels("${{ x && 'a' || 'b' }}"))
    ['a', 'b']
    >>> expression_labels("${{ matrix.os || 'ubuntu-latest' }}") is None
    True
    """
    expression = _EXPRESSION.fullmatch(runner.strip())
    if expression is None:
        return None
    labels = [_label(operand) for operand in _results(expression["body"])]
    if not labels or None in labels:
        return None
    return frozenset(label for label in labels if label is not None)


def _results(body: str) -> list[str]:
    """Return each operand the expression can yield, or nothing when unsafe."""
    disjuncts = [split_top_level(part, "&&") for part in split_top_level(body, "||")]
    if len(disjuncts[-1]) != 1:
        return []
    return [conjuncts[-1] for conjuncts in disjuncts]


def _label(operand: str) -> str | None:
    """Return the label a quoted operand names, or None for anything else."""
    match = _LABEL.fullmatch(operand)
    return match["label"] if match else None


def split_top_level(text: str, operator: str) -> list[str]:
    """Split on ``operator`` outside quoted literals and parentheses.

    Parameters
    ----------
    text : str
        An expression body.
    operator : str
        ``&&`` or ``||``.

    Returns
    -------
    list[str]
        The operands, stripped of surrounding whitespace.

    Examples
    --------
    >>> split_top_level("a == 'x && y' && (b && c) && d", "&&")
    ["a == 'x && y'", '(b && c)', 'd']
    """
    cuts: list[int] = []
    for index in _top_level_indices(text):
        clear = not cuts or index >= cuts[-1] + len(operator)
        if clear and text.startswith(operator, index):
            cuts.append(index)
    starts = [0, *(cut + len(operator) for cut in cuts)]
    ends = [*cuts, len(text)]
    return [text[start:end].strip() for start, end in zip(starts, ends, strict=True)]


def _top_level_indices(text: str) -> cabc.Iterator[int]:
    """Yield each index outside quotes at parenthesis depth zero."""
    quoted, depth = False, 0
    for index, char in enumerate(text):
        if char == "'":
            quoted = not quoted
        elif not quoted:
            depth += {"(": 1, ")": -1}.get(char, 0)
            if depth == 0:
                yield index
