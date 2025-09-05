package main

deny contains msg if {
  input.resource_changes[_].type == "digitalocean_kubernetes_cluster"
  input.resource_changes[_].change.after.ha != true
  msg := "control plane must be configured for high availability"
}
