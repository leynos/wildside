package fluxcd.policy

import future.keywords.contains
import future.keywords.if

branch(spec) = object.get(object.get(spec, "ref", {}), "branch", "")

url(spec) = object.get(spec, "url", "")

interval(spec) = object.get(spec, "interval", "")

path(spec) = object.get(spec, "path", "")

source_kind(spec) = object.get(object.get(spec, "sourceRef", {}), "kind", "")

prune(spec) = object.get(spec, "prune", false)

suspend(spec) = object.get(spec, "suspend", false)

# Ensure GitRepository URLs use supported schemes.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  rc.change.after.manifest.kind == "GitRepository"
  spec := rc.change.after.manifest.spec
  repo_url := url(spec)
  not startswith(repo_url, "https://")
  not startswith(repo_url, "ssh://")
  not startswith(repo_url, "git@")
  msg := sprintf("GitRepository %s must use an HTTPS, SSH, or git@ URL", [rc.change.after.manifest.metadata.name])
}

# GitRepository resources must declare a branch to avoid drifting refs.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  rc.change.after.manifest.kind == "GitRepository"
  spec := rc.change.after.manifest.spec
  branch(spec) == ""
  msg := sprintf("GitRepository %s must set spec.ref.branch", [rc.change.after.manifest.metadata.name])
}

# Clamp reconciliation intervals to avoid excessively slow drift detection.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  rc.change.after.manifest.kind == "GitRepository"
  spec := rc.change.after.manifest.spec
  reconcile := interval(spec)
  not regex.match(`^(?:[1-9][0-9]*m|[1-9][0-9]*s)$`, reconcile)
  msg := sprintf("GitRepository %s interval %q must be expressed in seconds or minutes", [rc.change.after.manifest.metadata.name, reconcile])
}

# Require Kustomization to prune resources for deterministic reconciliation.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  rc.change.after.manifest.kind == "Kustomization"
  spec := rc.change.after.manifest.spec
  prune(spec) == false
  msg := sprintf("Kustomization %s must enable prune", [rc.change.after.manifest.metadata.name])
}

# Kustomization paths must stay relative to the repository root.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  rc.change.after.manifest.kind == "Kustomization"
  spec := rc.change.after.manifest.spec
  p := path(spec)
  startswith(p, "/")
  msg := sprintf("Kustomization %s path %q must stay relative to the repository root", [rc.change.after.manifest.metadata.name, p])
}

# Enforce GitRepository as the source kind.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  rc.change.after.manifest.kind == "Kustomization"
  spec := rc.change.after.manifest.spec
  source_kind(spec) != "GitRepository"
  msg := sprintf("Kustomization %s must reference a GitRepository", [rc.change.after.manifest.metadata.name])
}

# Prevent accidental suspension, which would halt reconciliation silently.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "kubernetes_manifest"
  rc.change.after.manifest.kind == "Kustomization"
  spec := rc.change.after.manifest.spec
  suspend(spec)
  msg := sprintf("Kustomization %s must not be created in a suspended state", [rc.change.after.manifest.metadata.name])
}
