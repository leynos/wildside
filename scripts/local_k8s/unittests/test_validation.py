"""Unit tests for local preview validation helpers."""

from __future__ import annotations

import pytest

from local_k8s.deployment import image_repository_and_tag
from local_k8s.validation import LocalK8sError, require_tools, validate_port


def test_validate_port_uses_default_for_missing_value() -> None:
    assert validate_port(None, default=8088, name="WILDSIDE_K3D_PORT") == 8088


@pytest.mark.parametrize("raw_value", ["1", "8088", "65535"])
def test_validate_port_accepts_valid_tcp_ports(raw_value: str) -> None:
    assert validate_port(raw_value, default=8088, name="WILDSIDE_K3D_PORT") == int(raw_value)


@pytest.mark.parametrize(
    ("raw_value", "message_pattern"),
    [
        ("0", r"must be between 1 and 65535"),
        ("65536", r"must be between 1 and 65535"),
        ("not-a-port", r"must be an integer TCP port"),
    ],
)
def test_validate_port_rejects_invalid_values(raw_value: str, message_pattern: str) -> None:
    with pytest.raises(LocalK8sError, match=message_pattern):
        validate_port(raw_value, default=8088, name="WILDSIDE_K3D_PORT")


def test_require_tools_reports_missing_executables() -> None:
    with pytest.raises(LocalK8sError, match="definitely-not-a-wildside-tool"):
        require_tools(("definitely-not-a-wildside-tool",))


@pytest.mark.parametrize(
    ("image_name", "expected"),
    [
        ("wildside-backend:local", ("wildside-backend", "local")),
        ("registry.example.test:5000/wildside/backend:preview", ("registry.example.test:5000/wildside/backend", "preview")),
    ],
)
def test_image_repository_and_tag_accepts_tagged_images(image_name: str, expected: tuple[str, str]) -> None:
    assert image_repository_and_tag(image_name) == expected


@pytest.mark.parametrize("image_name", ["wildside-backend", "registry.example.test:5000/wildside/backend", ":local"])
def test_image_repository_and_tag_rejects_untagged_images(image_name: str) -> None:
    with pytest.raises(LocalK8sError, match="WILDSIDE_IMAGE"):
        image_repository_and_tag(image_name)
