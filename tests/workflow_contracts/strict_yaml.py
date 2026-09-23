"""Parses YAML the way the contracts need, refusing a repeated key.

PyYAML keeps the last of two identical keys in one mapping and says
nothing. A workflow declaring `runs-on` twice therefore parses into a
document that has already discarded the first value, so a lane can carry
a paid label in the discarded half, read as hosted, and pass every
placement contract while it bills. The same holds for a repeated
`secrets`, `if` or `env`: the contract asserts on whichever value PyYAML
happened to keep. GitHub itself rejects such a workflow, so the reader
refusing it too costs nothing.

Keys are compared as PyYAML constructs them, which is YAML 1.1, and that
leaves one disagreement with GitHub standing. A bare `on` and a quoted
`"on"` construct to `True` and `"on"`, two different keys, so a workflow
declaring both still loads, and `workflow_inventory.triggers_of` reads the
`True` entry only, where GitHub treats the two spellings as one key.
Closing that is a behaviour change for a separate decision, not a
docstring correction.

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
        the repetition. Keys are compared as constructed under YAML 1.1,
        so a bare `on` and a bare `true` are the same key (both `True`) and
        are refused as a repeat, while a bare `on` and a quoted `"on"` are
        different keys (`True` and `"on"`) and both survive.

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
            if key in seen:
                message = f"found the key {key!r} twice in one mapping"
                raise ConstructorError(None, None, message, key_node.start_mark)
            seen.add(key)
        return super().construct_mapping(node, deep=deep)


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
