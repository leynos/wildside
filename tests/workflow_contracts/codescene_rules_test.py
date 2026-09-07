"""Contract tests for the repository's CodeScene rule overrides.

`.codescene/code-health-rules.json` sat in this repository for months in a
shape CodeScene does not accept, so the exemption it declared never applied.
Nothing caught it: the file is valid JSON, the CLI's own diagnostic blames
JSON syntax, and the warning it prints goes to a log nobody reads. CodeScene's
verdicts simply carried on without the override.

`cs rules-config validate` is the authoritative check and the developers'
guide points at it, but the CodeScene CLI is not installed on the CI runners,
so these tests hold the same line where the gates actually run. They assert
the documented schema rather than merely that the file parses as JSON, and
they assert each exemption still matches something, since a glob that has gone
stale is an exemption that silently stops applying.
"""

from __future__ import annotations

import json
import typing as typ
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
RULES_PATH = REPOSITORY_ROOT / ".codescene" / "code-health-rules.json"

#: Keys `cs docs code-health-rules-template` emits for a rule set. `usage` and
#: the `_doc` suffixes are documentation the template carries itself, so they
#: are allowed rather than required.
RULE_SET_KEYS = frozenset({
    "matching_content_path",
    "matching_content_path_doc",
    "content_filter",
    "rules",
    "thresholds",
})


def _rule_sets() -> list[dict[str, typ.Any]]:
    """Return the file's rule sets, failing if the top-level shape is wrong."""
    document = json.loads(RULES_PATH.read_text(encoding="utf-8"))
    assert isinstance(document, dict), "the rule file must be a JSON object"
    rule_sets = document.get("rule_sets")
    assert isinstance(rule_sets, list), (
        "the rule file must carry a top-level 'rule_sets' array; a top-level "
        "'rules' object is the shape CodeScene rejects, and it rejects it "
        "with a message about JSON syntax that sends the reader elsewhere"
    )
    assert rule_sets, "an empty rule_sets array declares no override at all"
    return typ.cast("list[dict[str, typ.Any]]", rule_sets)


def test_the_rule_file_uses_the_documented_schema() -> None:
    """Every rule set and rule must use the keys CodeScene reads.

    Asserting the key names matters more than it looks. The shape this
    replaces used hyphenated keys, which CodeScene's parser reads as
    namespaced keywords and refuses, so a typo here fails the same way:
    silently, with the override ignored.
    """
    for rule_set in _rule_sets():
        assert isinstance(rule_set, dict), "each rule set must be an object"
        unexpected = set(rule_set) - RULE_SET_KEYS
        assert not unexpected, (
            f"unrecognized rule set keys {sorted(unexpected)}; CodeScene "
            "ignores what it does not recognize"
        )
        for rule in rule_set.get("rules", []):
            assert set(rule) == {"name", "weight"}, (
                f"a rule override carries exactly a name and a weight: {rule}"
            )
            assert isinstance(rule["name"], str) and " " in rule["name"], (
                "rules are named in prose, as in 'String Heavy Function "
                f"Arguments', not as a hyphenated slug: {rule['name']!r}"
            )
            weight = rule["weight"]
            assert isinstance(weight, (int, float)) and 0.0 <= weight <= 1.0, (
                "a rule's weight is a relative multiplier between 0.0 and "
                f"1.0, not a threshold: {weight!r}"
            )


def test_every_exemption_is_justified() -> None:
    """Each rule set explains itself, so a reader can judge it.

    An exemption without a stated reason is indistinguishable from one nobody
    revisited, which is how a narrow allowance becomes a permanent blind spot.
    """
    for rule_set in _rule_sets():
        justification = rule_set.get("matching_content_path_doc", "")
        assert len(justification) > 80, (
            "each rule set needs a matching_content_path_doc saying why the "
            f"exemption is deliberate: {rule_set.get('matching_content_path')!r}"
        )


def test_every_exemption_still_matches_something() -> None:
    """A rule set's glob must match at least one file in the repository.

    A path that has been renamed leaves an exemption that quietly stops
    applying, which is the same failure this file's previous shape had: the
    configuration reads as though it does something and does nothing.
    """
    for rule_set in _rule_sets():
        pattern = rule_set.get("matching_content_path")
        assert isinstance(pattern, str) and pattern, (
            "each rule set must declare a matching_content_path"
        )
        matched = next(REPOSITORY_ROOT.glob(pattern), None)
        assert matched is not None, (
            f"no file matches {pattern!r}; an exemption that matches nothing "
            "is either stale or was never right"
        )


@pytest.mark.parametrize(
    "document",
    [
        pytest.param(
            {
                "rules": {
                    "string-heavy-function-arguments": {
                        "threshold-by-pattern": {"**/domain/*.rs": 100}
                    }
                }
            },
            id="the-shape-codescene-rejected",
        ),
        pytest.param(
            {"rule_sets": [{"rules": [{"name": "String Heavy", "weight": 100}]}]},
            id="a-threshold-where-a-weight-belongs",
        ),
        pytest.param(
            {
                "rule_sets": [
                    {"rules": [{"name": "string-heavy-arguments", "weight": 0.0}]}
                ]
            },
            id="a-slug-where-a-prose-name-belongs",
        ),
    ],
)
def test_the_schema_assertions_reject_known_bad_shapes(
    document: dict[str, typ.Any], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The checks above must fail on the shapes that caused this.

    Without this the tests could assert nothing and still pass, which is the
    defect they exist to catch, one level up.
    """
    path = tmp_path / "code-health-rules.json"
    path.write_text(json.dumps(document), encoding="utf-8")
    # `raising` is left at its default on purpose: if this module is ever
    # imported under a different name the patch must fail loudly rather than
    # create a new attribute and let the test pass having patched nothing.
    monkeypatch.setattr(f"{__name__}.RULES_PATH", path)

    with pytest.raises(AssertionError):
        test_the_rule_file_uses_the_documented_schema()
