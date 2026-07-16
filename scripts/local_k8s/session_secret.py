"""Manage the Wildside local preview session signing key Secret."""

from __future__ import annotations

import base64
import logging
import secrets
from collections.abc import Callable

from .commands import run
from .config import PreviewConfig
from .validation import LocalK8sError

SESSION_SECRET_KEY_NAME = "session_key"  # noqa: S105 - Secret data key name, not secret material.
SESSION_SECRET_NAME = "wildside-session-key"  # noqa: S105 - Secret resource name, not secret material.

# kubectl reports a genuine create conflict as
# ``Error from server (AlreadyExists): secrets "..." already exists``. The
# ``(AlreadyExists)`` server reason is the structured signal we key on, rather
# than a bare "already exists" substring that could appear incidentally in an
# unrelated failure message.
_ALREADY_EXISTS_SERVER_REASON = "(AlreadyExists)"

logger = logging.getLogger(__name__)


def _is_already_exists_conflict(exc: LocalK8sError) -> bool:
    """Return True only for a genuine kubectl ``AlreadyExists`` server conflict.

    Classification relies on the preserved raw ``stderr`` carrying kubectl's
    structured ``(AlreadyExists)`` server reason, so a failure whose message
    merely mentions "already exists" incidentally is not misclassified.
    """
    return exc.stderr is not None and _ALREADY_EXISTS_SERVER_REASON in exc.stderr


def _fetch_existing_session_key(config: PreviewConfig) -> str:
    """Return the existing local session key payload, when Kubernetes has one."""
    existing_key = run(
        "kubectl",
        [
            "--context",
            config.kube_context,
            "-n",
            config.namespace,
            "get",
            "secret",
            SESSION_SECRET_NAME,
            "--ignore-not-found",
            f"-o=jsonpath={{.data.{SESSION_SECRET_KEY_NAME}}}",
        ],
    )
    return existing_key.stdout.strip()


def _render_session_secret_manifest(config: PreviewConfig, encoded_key: str) -> str:
    """Render the Kubernetes Secret manifest for the generated session key."""
    return f"""\
apiVersion: v1
kind: Secret
metadata:
  name: {SESSION_SECRET_NAME}
  namespace: {config.namespace}
type: Opaque
data:
  {SESSION_SECRET_KEY_NAME}: {encoded_key}
"""


def _log_session_secret_event(config: PreviewConfig, event: str) -> None:
    """Log a session Secret lifecycle event with the standard preview fields."""
    logger.info(
        event,
        extra={
            "cluster": config.cluster_name,
            "namespace": config.namespace,
            "secret": SESSION_SECRET_NAME,
        },
    )


def _apply_session_secret_manifest(config: PreviewConfig, manifest: str) -> None:
    """Create the session Secret, reconciling a concurrently created one.

    On the normal path the Secret is created from ``manifest``. If another deploy
    created it first (a kubectl ``AlreadyExists`` server conflict), the existing
    Secret is validated: when it already carries a non-empty ``session_key`` it
    is reused; otherwise it is repaired by replacing it with the fresh key
    material in ``manifest``. A Secret that still lacks key material after repair
    raises ``LocalK8sError``.
    """
    _log_session_secret_event(config, "local_k8s_session_secret_apply")
    try:
        run(
            "kubectl",
            ["--context", config.kube_context, "create", "-f", "-"],
            input_text=manifest,
        )
    except LocalK8sError as exc:
        if not _is_already_exists_conflict(exc):
            raise
    else:
        return

    _reconcile_existing_session_secret(config, manifest)


def _reconcile_existing_session_secret(config: PreviewConfig, manifest: str) -> None:
    """Reuse a concurrently created Secret, repairing it if it lacks a key."""
    if _fetch_existing_session_key(config):
        _log_session_secret_event(config, "local_k8s_session_secret_reuse")
        return

    # The existing Secret carries no session_key; apply fresh key material.
    _log_session_secret_event(config, "local_k8s_session_secret_repair")
    run(
        "kubectl",
        ["--context", config.kube_context, "apply", "-f", "-"],
        input_text=manifest,
    )
    if not _fetch_existing_session_key(config):
        error_message = (
            f"session Secret {SESSION_SECRET_NAME!r} still lacks "
            f"{SESSION_SECRET_KEY_NAME} after repair"
        )
        raise LocalK8sError(error_message)
    _log_session_secret_event(config, "local_k8s_session_secret_reuse")


def ensure_session_secret(
    config: PreviewConfig,
    *,
    key_generator: Callable[[int], bytes] = secrets.token_bytes,
) -> None:
    """Create the local preview session signing key Secret when absent.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the kube context, namespace, and
        Secret location used by the deployment.
    key_generator : Callable[[int], bytes], optional
        Injectable source of key bytes. Tests use this seam to make the
        rendered Secret deterministic; production uses ``secrets.token_bytes``.

    Returns
    -------
    None
        The function applies a Kubernetes Secret only when the expected key is
        missing. Existing key material is reused to avoid rotating local
        preview sessions on every deployment.
    """

    if _fetch_existing_session_key(config):
        _log_session_secret_event(config, "local_k8s_session_secret_reuse")
        return

    key = key_generator(96)
    encoded_key = base64.b64encode(key).decode("ascii")
    manifest = _render_session_secret_manifest(config, encoded_key)
    _apply_session_secret_manifest(config, manifest)
