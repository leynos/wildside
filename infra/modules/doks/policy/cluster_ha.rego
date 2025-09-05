package doks

deny[msg] {
  cluster := input.resource.digitalocean_kubernetes_cluster.this
  cluster.ha != true
  msg := "high availability must be enabled"
}
