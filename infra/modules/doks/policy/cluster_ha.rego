package policy

import rego.v1

# Deny if any DOKS cluster ends up with HA disabled in the planned state.
deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_kubernetes_cluster"
  rc.change.after != null
  rc.change.after.ha != true
  msg := "high availability must be enabled"
}
