package main

deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_kubernetes_cluster"
  np := rc.change.after.node_pool[_]
  not np.auto_scale
  np.node_count < 2
  msg := sprintf("node pool %s must have node_count >= 2 when auto_scale is disabled", [np.name])
}

deny contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_kubernetes_cluster"
  np := rc.change.after.node_pool[_]
  np.auto_scale
  np.min_nodes < 2
  msg := sprintf("node pool %s must have min_nodes >= 2 when auto_scale is enabled", [np.name])
}
