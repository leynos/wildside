"""Shared GitOps error types.

This module defines the exception hierarchy used by GitOps workflows, covering
validation failures, git command errors, clone failures, and manifest sync
issues.

Exceptions
----------
GitOpsError
GitCommandError
GitCloneError
GitValidationError
GitOpsValidationError
GitSyncError
"""

from __future__ import annotations


class GitOpsError(Exception):
    """Base error for GitOps manifest operations."""


class GitCommandError(GitOpsError):
    """Raised when a git command fails."""


class GitCloneError(GitOpsError):
    """Raised when cloning the GitOps repository fails."""


class GitValidationError(GitOpsError):
    """Raised when GitOps paths are unsafe or invalid."""


class GitOpsValidationError(GitValidationError):
    """Raised when GitOps paths are unsafe or invalid."""


class GitSyncError(GitOpsError):
    """Raised when syncing manifests fails."""
