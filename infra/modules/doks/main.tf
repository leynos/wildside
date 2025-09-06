resource "digitalocean_kubernetes_cluster" "this" {
  name    = var.cluster_name
  region  = var.region
  version = var.kubernetes_version
  tags    = var.tags

  dynamic "node_pool" {
    for_each = var.node_pools
    content {
      name       = node_pool.value.name
      size       = node_pool.value.size
      node_count = node_pool.value.node_count
      auto_scale = node_pool.value.auto_scale
      min_nodes  = node_pool.value.min_nodes
      max_nodes  = node_pool.value.max_nodes
      tags       = coalesce(node_pool.value.tags, [])
    }
  }
}
