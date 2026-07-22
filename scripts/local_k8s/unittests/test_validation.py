"""Unit tests for local preview validation helpers."""

from __future__ import annotations

import pytest
from local_k8s.deployment import image_repository_and_tag
from local_k8s.validation import LocalK8sError, require_tools, validate_port


def test_validate_port_uses_default_for_missing_value() -> None:
    """Verify absent port environment variables use the configured default."""
    assert validate_port(None, default=8088, name="WILDSIDE_K3D_PORT") == 8088, (
        "an unset port must fall back to the configured default"
    )


@pytest.mark.parametrize("raw_value", ["1", "8088", "65535"])
def test_validate_port_accepts_valid_tcp_ports(raw_value: str) -> None:
    """Verify valid TCP port bounds are accepted."""
    assert validate_port(raw_value, default=8088, name="WILDSIDE_K3D_PORT") == int(
        raw_value
    )


@pytest.mark.parametrize(
    ("raw_value", "message_pattern"),
    [
        ("0", r"must be between 1 and 65535"),
        ("65536", r"must be between 1 and 65535"),
        ("not-a-port", r"must be an integer TCP port"),
    ],
)
def test_validate_port_rejects_invalid_values(
    raw_value: str, message_pattern: str
) -> None:
    """Verify invalid TCP port values fail with actionable messages."""
    with pytest.raises(LocalK8sError, match=message_pattern):
        validate_port(raw_value, default=8088, name="WILDSIDE_K3D_PORT")


def test_require_tools_reports_missing_executables() -> None:
    """Verify missing command-line tools are reported by name."""
    with pytest.raises(LocalK8sError, match="definitely-not-a-wildside-tool"):
        require_tools(("definitely-not-a-wildside-tool",))


@pytest.mark.parametrize(
    ("image_name", "expected"),
    [
        ("wildside-backend:local", ("wildside-backend", "local")),
        (
            "registry.example.test:5000/wildside/backend:preview",
            ("registry.example.test:5000/wildside/backend", "preview"),
        ),
    ],
)
def test_image_repository_and_tag_accepts_tagged_images(
    image_name: str, expected: tuple[str, str]
) -> None:
    """Verify tagged image references split into Helm repository and tag."""
    assert image_repository_and_tag(image_name) == expected, (
        "a tagged image must split into its Helm repository and tag"
    )


@pytest.mark.parametrize(
    "image_name",
    ["wildside-backend", "registry.example.test:5000/wildside/backend", ":local"],
)
def test_image_repository_and_tag_rejects_untagged_images(image_name: str) -> None:
    """Verify untagged image references fail before Helm rendering."""
    with pytest.raises(LocalK8sError, match="WILDSIDE_IMAGE"):
        image_repository_and_tag(image_name)


@pytest.mark.parametrize(
    "image_name",
    [
        # A comma or ``=`` in the tag would smuggle extra Helm chart values.
        "wildside-backend:local,ingress.enabled=true",
        "wildside-backend:local=oops",
        # The same characters must be rejected in the repository component.
        "wildside,ingress.enabled=true/backend:local",
        "wildside-backend:with space",
    ],
)
def test_image_repository_and_tag_rejects_helm_set_metacharacters(
    image_name: str,
) -> None:
    """Verify image references cannot inject Helm ``--set`` chart values."""
    with pytest.raises(LocalK8sError, match="WILDSIDE_IMAGE"):
        image_repository_and_tag(image_name)
