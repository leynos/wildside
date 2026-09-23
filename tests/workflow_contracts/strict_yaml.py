"""Parses YAML the way the contracts need, refusing a repeated key.

PyYAML keeps the last of two identical keys in one mapping and says
nothing. A workflow declaring `runs-on` twice therefore parses into a
document that has already discarded the first value, so a lane can carry
a paid label in the discarded half, read as hosted, and pass every
placement contract while it bills. The same holds for a repeated
`secrets`, `if` or `env`: the contract asserts on whichever value PyYAML
happened to keep. GitHub itself rejects such a workflow, so the reader
refusing it too costs nothing.

A key is also compared by the text it is written with, because PyYAML
resolves YAML 1.1 and GitHub does not. A bare `on` constructs to `True`
here and a quoted `"on"` to `"on"`, two different keys to PyYAML, while
GitHub reads both as `on` and merges them into one trigger mapping. A
reader then sees only one half, and `workflow_inventory.triggers_of`
reads the `True` entry alone. So two keys written with the same text are
a repeat whatever they construct to, and a workflow declaring both
spellings of `on` is refused.

Every contract reading a workflow goes through :func:`load`. Nothing here
knows what a workflow is.
"""

from __future__ import annotations

import collections.abc as cabc
import typing as typ

import yaml
from yaml.constructor import ConstructorError


class _StrictLoader(yaml.SafeLoader):
    """A safe loader that refuses a mapping declaring one key twice."""

    @typ.override
    def construct_mapping(
        self, node: yaml.MappingNode, deep: bool = False
    ) -> dict[object, object]:
        """Build a mapping, failing on the first repeated key.

        The check runs before the base constructor, which would collapse
        the repetition. Two keys are a repeat when they construct to the
        same value under YAML 1.1, as a bare `on` and a bare `true` do
        (both `True`), or when they are written with the same scalar text,
        as a bare `on` and a quoted `"on"` are, which GitHub reads as one
        key.

        Parameters
        ----------
        node : yaml.MappingNode
            The mapping node to construct.
        deep : bool
            Whether nested values are constructed eagerly.

        Returns
        -------
        dict[object, object]
            The constructed mapping.

        Raises
        ------
        ConstructorError
            If a key appears twice.
        """
        seen: set[object] = set()
        for key_node, _ in node.value:
            key = self.construct_object(key_node, deep=True)
            # An unhashable key is the base constructor's to refuse, with
            # its own message; it cannot be a repeat this check could see.
            if not isinstance(key, cabc.Hashable):
                continue
            identities = {("value", key), *_written_text(key_node)}
            if identities & seen:
                message = f"found the key {key_node.value!r} twice in one mapping"
                raise ConstructorError(None, None, message, key_node.start_mark)
            seen |= identities
        return super().construct_mapping(node, deep=deep)


def _written_text(key_node: yaml.Node) -> set[tuple[str, object]]:
    """Return a scalar key's identity as written, or nothing for another node."""
    if isinstance(key_node, yaml.ScalarNode):
        return {("text", key_node.value)}
    return set()


def load(text: str) -> typ.Any:  # noqa: ANN401 - the same contract as yaml.safe_load.
    r"""Parse one YAML document, refusing a mapping that repeats a key.

    The result is typed as ``yaml.safe_load``'s is, so a caller moving to
    this loader keeps the narrowing it already does at its own boundary.

    Parameters
    ----------
    text : str
        The document's text.

    Returns
    -------
    typing.Any
        The parsed document.

    Raises
    ------
    yaml.YAMLError
        If the text is not YAML, or a mapping in it repeats a key.

    Examples
    --------
    >>> load("jobs: {build: {runs-on: ubuntu-latest}}")
    {'jobs': {'build': {'runs-on': 'ubuntu-latest'}}}
    >>> try:
    ...     load("runs-on: a\nruns-on: b\n")
    ... except yaml.YAMLError as error:
    ...     print(error.problem)
    found the key 'runs-on' twice in one mapping
    """
    return yaml.load(text, Loader=_StrictLoader)  # noqa: S506 - a SafeLoader subclass.
