"""Contract tests for the repository's CodeScene rule overrides.

`.codescene/code-health-rules.json` sat in this repository for months in a
shape CodeScene does not accept, so the exemption it declared never applied.
Nothing caught it: the file is valid JSON, the CLI's own diagnostic blames
JSON syntax, and the warning it prints goes to a log nobody reads. CodeScene's
verdicts simply carried on without the override.

`cs rules-config validate` is the authoritative check and the developers'
guide points at it, but the CodeScene CLI is not installed on the CI runners,
so these tests hold the same line where the gates run.

Every assertion here exists to catch a rule set that CodeScene reads as
declaring nothing: a shape it rejects outright, a rule name it does not
recognize, an empty override list, or a path that matches no file. Each of
those looks like a deliberate exemption in review and is a no-op in practice.
"""

from __future__ import annotations

import json
import typing as typ
from pathlib import Path

import pytest

type JsonValue = (
    str | int | float | bool | list[JsonValue] | dict[str, JsonValue] | None
)

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
RULES_PATH = REPOSITORY_ROOT / ".codescene" / "code-health-rules.json"

#: Keys `cs docs code-health-rules-template` emits for a rule set. The `_doc`
#: suffixes are documentation the template carries itself.
RULE_SET_KEYS = frozenset({
    "matching_content_path",
    "matching_content_path_doc",
    "content_filter",
    "rules",
    "thresholds",
})

#: Every rule CodeScene will honour, from
#: `cs docs code-health-rules-template`. An unrecognized name is not an error
#: to CodeScene; it is ignored, which is the failure this file exists to
#: prevent, so the names are pinned rather than pattern-matched. Refresh this
#: set from the template when CodeScene adds a rule.
KNOWN_RULES = frozenset({
    "Brain Method",
    "Bumpy Road Ahead",
    "Code Duplication",
    "Complex Conditional",
    "Complex Method",
    "Constructor Over-Injection",
    "Deep, Global Nested Complexity",
    "Deep, Nested Complexity",
    "Duplicated Assertion Blocks",
    "Excess Number of Function Arguments",
    "Global Conditionals",
    "Large Assertion Blocks",
    "Large Embedded Code Block",
    "Large Method",
    "Lines of Code in a Single File",
    "Lines of Declarations in a Single File",
    "Low Cohesion",
    "Missing Arguments Abstractions",
    "Number of Functions in a Single Module",
    "Overall Code Complexity",
    "Overall Function Size",
    "Primitive Obsession",
    "String Heavy Function Arguments",
})


def _rule_sets(path: Path | None = None) -> list[dict[str, JsonValue]]:
    """Return the file's rule sets, failing if the top-level shape is wrong."""
    document: JsonValue = json.loads((path or RULES_PATH).read_text(encoding="utf-8"))
    assert isinstance(document, dict), "the rule file must be a JSON object"
    rule_sets = document.get("rule_sets")
    assert isinstance(rule_sets, list), (
        "the rule file must carry a top-level 'rule_sets' array; a top-level "
        "'rules' object is the shape CodeScene rejects, and it rejects it "
        "with a message about JSON syntax that sends the reader elsewhere"
    )
    assert rule_sets, "an empty rule_sets array declares no override at all"
    for rule_set in rule_sets:
        assert isinstance(rule_set, dict), "each rule set must be an object"
    return typ.cast("list[dict[str, JsonValue]]", rule_sets)


def _assert_rule_is_honoured(rule: JsonValue) -> None:
    """Fail unless CodeScene would recognize and apply ``rule``."""
    assert isinstance(rule, dict), f"a rule override must be an object: {rule!r}"
    assert set(rule) == {"name", "weight"}, (
        f"a rule override carries exactly a name and a weight: {rule}"
    )

    name = rule["name"]
    assert isinstance(name, str), f"a rule's name must be a string: {name!r}"
    assert name in KNOWN_RULES, (
        f"{name!r} is not a CodeScene rule; unrecognized names are ignored "
        "rather than rejected, so the override would silently do nothing. "
        "Prose names come from `cs docs code-health-rules-template`, not from "
        "a hyphenated slug"
    )

    weight = rule["weight"]
    assert isinstance(weight, (int, float)), (
        f"a rule's weight must be a number: {weight!r}"
    )
    # `bool` is a subclass of `int`, so `True` would otherwise pass as 1.0.
    assert not isinstance(weight, bool), (
        f"a rule's weight must be a number rather than a boolean: {weight!r}"
    )
    assert 0.0 <= weight <= 1.0, (
        "a rule's weight is a relative multiplier between 0.0, which disables "
        f"the rule, and 1.0, which is the default. It is not a threshold: {weight!r}"
    )


def test_the_rule_file_uses_the_documented_schema() -> None:
    """Every rule set and rule must use the keys and names CodeScene reads.

    Asserting the names matters more than it looks. The shape this replaces
    used hyphenated keys, which CodeScene's parser reads as namespaced
    keywords and refuses, so a typo here fails the same way: silently, with
    the override ignored.
    """
    for rule_set in _rule_sets():
        unexpected = set(rule_set) - RULE_SET_KEYS
        assert not unexpected, (
            f"unrecognized rule set keys {sorted(unexpected)}; CodeScene "
            "ignores what it does not recognize"
        )
        rules = rule_set.get("rules", [])
        assert isinstance(rules, list), "a rule set's 'rules' must be an array"
        for rule in rules:
            _assert_rule_is_honoured(rule)


def test_every_rule_set_actually_overrides_something() -> None:
    """A rule set with no rules and no thresholds changes nothing.

    Deleting the `rules` key, or leaving it empty, produces a rule set that
    validates, reads as a deliberate exemption, and has no effect whatever.
    That is the same silent no-op as the shape this file replaces, so it is
    asserted rather than assumed.
    """
    for rule_set in _rule_sets():
        overrides = rule_set.get("rules", []), rule_set.get("thresholds", [])
        assert any(overrides), (
            "each rule set must declare at least one rule or threshold "
            f"override: {rule_set.get('matching_content_path')!r}"
        )


def test_every_exemption_is_justified() -> None:
    """Each rule set explains itself, so a reader can judge it.

    An exemption without a stated reason is indistinguishable from one nobody
    revisited, which is how a narrow allowance becomes a permanent blind spot.
    """
    for rule_set in _rule_sets():
        justification = rule_set.get("matching_content_path_doc", "")
        assert isinstance(justification, str), (
            "matching_content_path_doc must be a string"
        )
        assert len(justification) > 80, (
            "each rule set needs a matching_content_path_doc saying why the "
            f"exemption is deliberate: {rule_set.get('matching_content_path')!r}"
        )


def test_every_exemption_still_matches_a_file() -> None:
    """A rule set's glob must match at least one file in the repository.

    A path left behind by a rename is an exemption that quietly stops
    applying, which is the same failure this file's previous shape had: the
    configuration reads as though it does something and does nothing.

    Directories are excluded because `Path.glob` yields them too, and a rule
    set matching only a directory grants nothing.
    """
    for rule_set in _rule_sets():
        pattern = rule_set.get("matching_content_path")
        assert isinstance(pattern, str), (
            "each rule set must declare a matching_content_path"
        )
        assert pattern, "matching_content_path must not be empty"
        matched = next(
            (
                candidate
                for candidate in REPOSITORY_ROOT.glob(pattern)
                if candidate.is_file()
            ),
            None,
        )
        assert matched is not None, (
            f"no file matches {pattern!r}; an exemption that matches nothing "
            "is either stale or was never right"
        )


#: Shapes that validate as JSON, read in review as a deliberate exemption, and
#: apply nothing. Each is a way this contract could pass while asserting
#: nothing, so each is asserted to fail.
NO_OP_SHAPES = (
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
        {
            "rule_sets": [
                {"rules": [{"name": "String Heavy Function Arguments", "weight": 100}]}
            ]
        },
        id="a-threshold-where-a-weight-belongs",
    ),
    pytest.param(
        {"rule_sets": [{"rules": [{"name": "string-heavy-arguments", "weight": 0.0}]}]},
        id="a-slug-where-a-prose-name-belongs",
    ),
    pytest.param(
        {"rule_sets": [{"rules": [{"name": "Strung Heavy Arguments", "weight": 0.0}]}]},
        id="a-rule-name-codescene-does-not-know",
    ),
)


@pytest.mark.parametrize("document", NO_OP_SHAPES)
def test_the_schema_assertions_reject_known_bad_shapes(
    document: dict[str, JsonValue], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The schema checks must fail on the shapes that caused this.

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


@pytest.mark.parametrize(
    "document",
    [
        pytest.param(
            {"rule_sets": [{"matching_content_path": "**/*.rs"}]}, id="no-keys"
        ),
        pytest.param(
            {"rule_sets": [{"matching_content_path": "**/*.rs", "rules": []}]},
            id="empty-rules",
        ),
    ],
)
def test_a_rule_set_that_overrides_nothing_is_rejected(
    document: dict[str, JsonValue], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An exemption that applies nothing must fail, not pass quietly."""
    path = tmp_path / "code-health-rules.json"
    path.write_text(json.dumps(document), encoding="utf-8")
    monkeypatch.setattr(f"{__name__}.RULES_PATH", path)

    with pytest.raises(AssertionError):
        test_every_rule_set_actually_overrides_something()


def test_the_domain_exemption_is_the_one_this_repository_intends() -> None:
    """The committed override must be the specific exemption #487 restores.

    The schema checks above are generic: they would pass just as happily for
    an unrelated rule set, or for none at all once this one was replaced.
    This pins what the repository actually means to exempt, so removing or
    retargeting the domain exemption is a deliberate edit here rather than a
    silent change of policy.
    """
    domain = [
        rule_set
        for rule_set in _rule_sets()
        if rule_set.get("matching_content_path") == "**/domain/*.rs"
    ]
    assert len(domain) == 1, (
        "exactly one rule set must target '**/domain/*.rs'; that is the "
        "exemption #229 intended and #487 restores"
    )

    rules = domain[0].get("rules")
    assert isinstance(rules, list), "the domain rule set must declare rules"
    weights = {
        rule["name"]: rule["weight"]
        for rule in rules
        if isinstance(rule, dict) and isinstance(rule.get("name"), str)
    }
    assert weights == {"String Heavy Function Arguments": 0.0}, (
        "the domain rule set exists to disable String Heavy Function "
        f"Arguments, and nothing else: {weights}"
    )
