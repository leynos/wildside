#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["plumbum"]
# ///

"""Rotate session signing key for the Wildside backend.

This script automates zero-downtime rotation of the session signing key used
by the backend for cookie-based sessions. It relies on rolling deployment
overlap rather than in-app dual-key validation.

Prerequisites:
  - kubectl configured with cluster access
  - At least 2 replicas running for zero-downtime rotation
  - Secret must already exist (use kubectl create secret first)

Usage:
  ./scripts/rotate_session_key.py --namespace wildside --secret-name wildside-session-key
  ./scripts/rotate_session_key.py -n wildside -s wildside-session-key --deployment wildside-backend

See docs/runbooks/session-key-rotation.md for the complete rotation procedure.
"""

from __future__ import annotations

import base64
import hashlib
import secrets
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

from plumbum import ProcessExecutionError, local

if TYPE_CHECKING:
    from collections.abc import Mapping

# Session key must be at least 64 bytes per backend requirements.
SESSION_KEY_LENGTH = 64
FINGERPRINT_BYTES = 8


@dataclass(frozen=True, slots=True)
class RotationConfig:
    """Configuration for session key rotation."""

    namespace: str
    secret_name: str
    secret_key: str
    deployment_name: str | None


@dataclass(frozen=True, slots=True)
class RotationResult:
    """Result of a session key rotation."""

    old_fingerprint: str | None
    new_fingerprint: str
    rollout_triggered: bool


def compute_fingerprint(key_bytes: bytes) -> str:
    """Compute truncated SHA-256 fingerprint of key material.

    Returns the first 8 bytes as a 16-character hex string, matching the
    backend's fingerprinting algorithm.
    """
    hasher = hashlib.sha256()
    hasher.update(key_bytes)
    return hasher.hexdigest()[: FINGERPRINT_BYTES * 2]


def generate_session_key() -> bytes:
    """Generate a cryptographically secure session key."""
    return secrets.token_bytes(SESSION_KEY_LENGTH)


def get_current_key(config: RotationConfig) -> bytes | None:
    """Retrieve the current session key from the Kubernetes secret."""
    kubectl = local["kubectl"]
    try:
        result = kubectl[
            "get",
            "secret",
            config.secret_name,
            "-n",
            config.namespace,
            "-o",
            f"jsonpath={{.data.{config.secret_key}}}",
        ]()
        if result.strip():
            return base64.b64decode(result.strip())
    except ProcessExecutionError:
        pass
    return None


def update_secret(config: RotationConfig, new_key: bytes) -> None:
    """Update the Kubernetes secret with the new session key."""
    kubectl = local["kubectl"]
    encoded_key = base64.b64encode(new_key).decode("ascii")

    # Use kubectl patch to update the secret data
    patch = f'{{"data": {{"{config.secret_key}": "{encoded_key}"}}}}'
    kubectl[
        "patch",
        "secret",
        config.secret_name,
        "-n",
        config.namespace,
        "--type=merge",
        "-p",
        patch,
    ]()


def trigger_rollout(config: RotationConfig) -> bool:
    """Trigger a rolling restart of the deployment.

    Returns True if a rollout was triggered, False if no deployment specified.
    """
    if not config.deployment_name:
        return False

    kubectl = local["kubectl"]
    kubectl[
        "rollout",
        "restart",
        f"deployment/{config.deployment_name}",
        "-n",
        config.namespace,
    ]()
    return True


def wait_for_rollout(config: RotationConfig) -> None:
    """Wait for the deployment rollout to complete."""
    if not config.deployment_name:
        return

    kubectl = local["kubectl"]
    kubectl[
        "rollout",
        "status",
        f"deployment/{config.deployment_name}",
        "-n",
        config.namespace,
        "--timeout=300s",
    ]()


def check_replica_count(config: RotationConfig) -> int:
    """Check the current replica count for safety validation."""
    if not config.deployment_name:
        return 0

    kubectl = local["kubectl"]
    try:
        result = kubectl[
            "get",
            "deployment",
            config.deployment_name,
            "-n",
            config.namespace,
            "-o",
            "jsonpath={.spec.replicas}",
        ]()
        return int(result.strip()) if result.strip() else 0
    except (ProcessExecutionError, ValueError):
        return 0


def rotate_session_key(config: RotationConfig) -> RotationResult:
    """Perform the session key rotation.

    Generates a new key, updates the secret, and optionally triggers a
    rolling restart of the deployment.
    """
    # Get current key for fingerprint comparison
    current_key = get_current_key(config)
    old_fingerprint = compute_fingerprint(current_key) if current_key else None

    # Generate and apply new key
    new_key = generate_session_key()
    new_fingerprint = compute_fingerprint(new_key)

    update_secret(config, new_key)

    # Trigger rollout if deployment specified
    rollout_triggered = trigger_rollout(config)

    return RotationResult(
        old_fingerprint=old_fingerprint,
        new_fingerprint=new_fingerprint,
        rollout_triggered=rollout_triggered,
    )


def validate_replica_count(config: RotationConfig) -> bool:
    """Check replica count and prompt for confirmation if below threshold.

    Returns True if rotation should proceed, False if cancelled.
    """
    if not config.deployment_name:
        return True

    replicas = check_replica_count(config)
    if replicas >= 2:
        return True

    print(
        f"WARNING: Deployment has {replicas} replica(s). "
        "Zero-downtime rotation requires at least 2 replicas.",
        file=sys.stderr,
    )
    print("Continue anyway? [y/N] ", end="", file=sys.stderr)
    response = input().strip().lower()
    if response not in ("y", "yes"):
        print("Rotation cancelled.", file=sys.stderr)
        return False
    return True


def print_rotation_summary(config: RotationConfig, result: RotationResult) -> None:
    """Print the rotation summary with old and new fingerprints."""
    print("\n=== Rotation Summary ===")
    if result.old_fingerprint:
        print(f"Old fingerprint: {result.old_fingerprint}")
    else:
        print("Old fingerprint: (none - new secret)")
    print(f"New fingerprint: {result.new_fingerprint}")


def handle_rollout(config: RotationConfig, result: RotationResult) -> None:
    """Handle rollout triggering, waiting, and status messages."""
    if result.rollout_triggered:
        print(f"\nTriggered rollout for deployment '{config.deployment_name}'")
        print("Waiting for rollout to complete...")
        try:
            wait_for_rollout(config)
            print("Rollout completed successfully.")
        except ProcessExecutionError as exc:
            print(f"warning: rollout status check failed: {exc}", file=sys.stderr)
            print("Check deployment status manually with:")
            print(f"  kubectl rollout status deployment/{config.deployment_name} "
                  f"-n {config.namespace}")
    else:
        print("\nNo deployment specified; rollout not triggered.")
        print("Trigger manually with:")
        print(f"  kubectl rollout restart deployment/<name> -n {config.namespace}")


def print_verification_instructions(
    config: RotationConfig, result: RotationResult
) -> None:
    """Print post-rotation verification instructions."""
    print("\n=== Post-Rotation Verification ===")
    print("Verify the new fingerprint appears in pod logs:")
    print(f'  kubectl logs -n {config.namespace} -l app.kubernetes.io/name=wildside '
          f'| grep "fingerprint={result.new_fingerprint}"')


def parse_args(argv: list[str]) -> RotationConfig:
    """Parse command-line arguments."""
    namespace = "default"
    secret_name = "wildside-session-key"
    secret_key = "session_key"
    deployment_name: str | None = None

    i = 1
    while i < len(argv):
        arg = argv[i]
        if arg in ("-n", "--namespace") and i + 1 < len(argv):
            namespace = argv[i + 1]
            i += 2
        elif arg in ("-s", "--secret-name") and i + 1 < len(argv):
            secret_name = argv[i + 1]
            i += 2
        elif arg in ("-k", "--secret-key") and i + 1 < len(argv):
            secret_key = argv[i + 1]
            i += 2
        elif arg in ("-d", "--deployment") and i + 1 < len(argv):
            deployment_name = argv[i + 1]
            i += 2
        elif arg in ("-h", "--help"):
            print(__doc__)
            print("\nOptions:")
            print("  -n, --namespace    Kubernetes namespace (default: default)")
            print("  -s, --secret-name  Secret name (default: wildside-session-key)")
            print("  -k, --secret-key   Key within secret (default: session_key)")
            print("  -d, --deployment   Deployment name for rollout (optional)")
            print("  -h, --help         Show this help message")
            raise SystemExit(0)
        else:
            print(f"Unknown argument: {arg}", file=sys.stderr)
            raise SystemExit(1)

    return RotationConfig(
        namespace=namespace,
        secret_name=secret_name,
        secret_key=secret_key,
        deployment_name=deployment_name,
    )


def main(argv: list[str] | None = None) -> int:
    """Entry point for session key rotation."""
    if argv is None:
        argv = sys.argv

    config = parse_args(argv)

    if not validate_replica_count(config):
        return 1

    print(f"Rotating session key in secret '{config.secret_name}' "
          f"(namespace: {config.namespace})")

    try:
        result = rotate_session_key(config)
    except ProcessExecutionError as exc:
        print(f"error: kubectl command failed: {exc}", file=sys.stderr)
        return 1

    print_rotation_summary(config, result)
    handle_rollout(config, result)
    print_verification_instructions(config, result)

    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
