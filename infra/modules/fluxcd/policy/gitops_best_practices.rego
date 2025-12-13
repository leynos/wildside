package fluxcd.policy

import rego.v1

manifest_after(rc) = manifest if {
  after := rc.change.after
  after != null
  manifest := object.get(after, "manifest", null)
  manifest != null
}

branch(spec) = object.get(object.get(spec, "ref", {}), "branch", "")

url(spec) = object.get(spec, "url", "")

allowed_repo_url(repo_url) if {
  startswith(repo_url, "https://")
}

allowed_repo_url(repo_url) if {
  startswith(repo_url, "ssh://")
}

allowed_repo_url(repo_url) if {
  startswith(repo_url, "git@")
}

allowed_repo_url(repo_url) if {
  object.get(data, "allow_file_scheme", false)
  startswith(repo_url, "file://")
}

interval(spec) = object.get(spec, "interval", "")

path(spec) = object.get(spec, "path", "")

invalid_kustomization_path(p) if {
  startswith(p, "/")
}

invalid_kustomization_path(p) if {
  regex.match(`(^|/|\\)\.\.($|/|\\)`, p)
}

source_kind(spec) = object.get(object.get(spec, "sourceRef", {}), "kind", "")

prune(spec) = object.get(spec, "prune", false)

suspend(spec) = object.get(spec, "suspend", false)

# Ensure GitRepository URLs use supported schemes.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  manifest := manifest_after(rc)
  manifest.kind == "GitRepository"
  spec := manifest.spec
  repo_url := url(spec)
  not allowed_repo_url(repo_url)
  msg := sprintf("GitRepository %s must use an HTTPS, SSH, or git@ URL", [manifest.metadata.name])
}

# GitRepository resources must declare a branch to avoid drifting refs.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  manifest := manifest_after(rc)
  manifest.kind == "GitRepository"
  spec := manifest.spec
  branch(spec) == ""
  msg := sprintf("GitRepository %s must set spec.ref.branch", [manifest.metadata.name])
}

# Clamp reconciliation intervals to avoid excessively slow drift detection.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  manifest := manifest_after(rc)
  manifest.kind == "GitRepository"
  spec := manifest.spec
  reconcile := interval(spec)
  not regex.match(`^(?:[1-9][0-9]*m|[1-9][0-9]*s)$`, reconcile)
  msg := sprintf("GitRepository %s interval %q must be expressed in seconds or minutes", [manifest.metadata.name, reconcile])
}

# Require Kustomization to prune resources for deterministic reconciliation.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  manifest := manifest_after(rc)
  manifest.kind == "Kustomization"
  spec := manifest.spec
  prune(spec) == false
  msg := sprintf("Kustomization %s must enable prune", [manifest.metadata.name])
}

# Kustomization paths must stay relative to the repository root.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  manifest := manifest_after(rc)
  manifest.kind == "Kustomization"
  spec := manifest.spec
  p := path(spec)
  invalid_kustomization_path(p)
  msg := sprintf("Kustomization %s path %q must stay relative to the repository root", [manifest.metadata.name, p])
}

# Enforce GitRepository as the source kind.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  manifest := manifest_after(rc)
  manifest.kind == "Kustomization"
  spec := manifest.spec
  source_kind(spec) != "GitRepository"
  msg := sprintf("Kustomization %s must reference a GitRepository", [manifest.metadata.name])
}

# Prevent accidental suspension, which would halt reconciliation silently.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  manifest := manifest_after(rc)
  manifest.kind == "Kustomization"
  spec := manifest.spec
  suspend(spec)
  msg := sprintf("Kustomization %s must not be created in a suspended state", [manifest.metadata.name])
}
