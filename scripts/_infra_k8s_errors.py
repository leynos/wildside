"""Exception hierarchy for wildside-infra-k8s utilities."""

from __future__ import annotations


class InfraK8sError(Exception):
    """Base error for wildside-infra-k8s orchestration helpers."""


class TofuCommandError(InfraK8sError):
    """Raised when an OpenTofu command fails."""

