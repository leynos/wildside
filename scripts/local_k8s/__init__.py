"""Local Kubernetes preview helpers for Wildside.

Extended Summary
----------------
This package provides lifecycle commands (up, down, status, and logs) that
manage a local k3d cluster, import the backend Docker image, and install
the Wildside Helm chart.  It is designed for fast development feedback
without a remote Kubernetes environment.

Notes
-----
The CLI entry point is ``scripts/local_k8s.py``.  Each command loads
``PreviewConfig`` from environment variables before delegating to the
modules in this package.

Examples
--------
.. code-block:: sh

   uv run scripts/local_k8s.py up
   make local-k8s-up
"""
