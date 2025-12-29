variable "cluster_name" {
  description = "Name of the Valkey cluster resource"
  type        = string
  default     = "valkey"

  validation {
    condition = (
      var.cluster_name != null &&
      length(trimspace(var.cluster_name)) > 0 &&
      length(trimspace(var.cluster_name)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.cluster_name)))
    )
    error_message = "cluster_name must be a valid Kubernetes resource name"
  }
}

variable "nodes" {
  description = "Number of shards (Valkey cluster nodes)"
  type        = number
  default     = 1
  nullable    = false

  validation {
    condition     = var.nodes >= 1
    error_message = "nodes must be at least 1"
  }
}

variable "replicas" {
  description = "Number of replicas per shard (0 for standalone, 1+ for HA)"
  type        = number
  default     = 0
  nullable    = false

  validation {
    condition     = var.replicas >= 0
    error_message = "replicas must be 0 or more"
  }
}

variable "cluster_domain" {
  description = "Kubernetes cluster DNS domain"
  type        = string
  default     = "cluster.local"

  validation {
    condition     = var.cluster_domain != null && length(trimspace(var.cluster_domain)) > 0
    error_message = "cluster_domain must not be blank"
  }
}

variable "persistence_enabled" {
  description = "Enable persistent storage for Valkey data"
  type        = bool
  default     = true
  nullable    = false
}

variable "storage_size" {
  description = "PVC storage size for each Valkey instance"
  type        = string
  default     = "1Gi"

  validation {
    condition = (
      var.storage_size != null &&
      can(regex("^([0-9]+\\.?[0-9]*|[0-9]*\\.?[0-9]+)([eE][+-]?[0-9]+)?(Ki|Mi|Gi|Ti|Pi|Ei|k|M|G|T|P|E)?$", trimspace(var.storage_size)))
    )
    error_message = "storage_size must be a valid Kubernetes quantity (e.g., 1Gi, 500Mi, 1e9)"
  }
}

variable "storage_class" {
  description = "Kubernetes storage class for persistent volumes"
  type        = string
  default     = "do-block-storage"

  validation {
    condition     = var.storage_class != null && length(trimspace(var.storage_class)) > 0
    error_message = "storage_class must not be blank"
  }
}

variable "image" {
  description = "Custom Valkey container image (leave blank to use operator default)"
  type        = string
  default     = ""
}

variable "exporter_image" {
  description = "Custom Prometheus exporter image (leave blank to use operator default)"
  type        = string
  default     = ""
}

variable "resource_requests" {
  description = "Resource requests for Valkey containers"
  type = object({
    cpu    = string
    memory = string
  })
  default = {
    cpu    = "100m"
    memory = "128Mi"
  }
  nullable = false

  validation {
    condition = can(regex(
      "^([0-9]+\\.?[0-9]*|[0-9]*\\.?[0-9]+)([eE][+-]?[0-9]+)?m?$",
      trimspace(var.resource_requests.cpu)
    ))
    error_message = "resource_requests.cpu must be a valid Kubernetes CPU quantity (e.g., 100m, 0.5, 1)"
  }

  validation {
    condition = can(regex(
      "^([0-9]+\\.?[0-9]*|[0-9]*\\.?[0-9]+)([eE][+-]?[0-9]+)?(Ki|Mi|Gi|Ti|Pi|Ei|k|M|G|T|P|E)?$",
      trimspace(var.resource_requests.memory)
    ))
    error_message = "resource_requests.memory must be a valid Kubernetes memory quantity (e.g., 128Mi, 1Gi)"
  }
}

variable "resource_limits" {
  description = "Resource limits for Valkey containers"
  type = object({
    cpu    = string
    memory = string
  })
  default = {
    cpu    = "500m"
    memory = "512Mi"
  }
  nullable = false

  validation {
    condition = can(regex(
      "^([0-9]+\\.?[0-9]*|[0-9]*\\.?[0-9]+)([eE][+-]?[0-9]+)?m?$",
      trimspace(var.resource_limits.cpu)
    ))
    error_message = "resource_limits.cpu must be a valid Kubernetes CPU quantity (e.g., 500m, 1, 2)"
  }

  validation {
    condition = can(regex(
      "^([0-9]+\\.?[0-9]*|[0-9]*\\.?[0-9]+)([eE][+-]?[0-9]+)?(Ki|Mi|Gi|Ti|Pi|Ei|k|M|G|T|P|E)?$",
      trimspace(var.resource_limits.memory)
    ))
    error_message = "resource_limits.memory must be a valid Kubernetes memory quantity (e.g., 512Mi, 1Gi)"
  }
}

variable "prometheus_enabled" {
  description = "Enable Prometheus metrics endpoint"
  type        = bool
  default     = false
  nullable    = false
}

variable "service_monitor_enabled" {
  description = "Create Prometheus ServiceMonitor resource"
  type        = bool
  default     = false
  nullable    = false
}

variable "pdb_enabled" {
  description = "Create PodDisruptionBudget for the cluster (only effective when replicas > 0)"
  type        = bool
  default     = true
  nullable    = false
}

variable "pdb_min_available" {
  description = "Minimum number of pods that must remain available during disruptions"
  type        = number
  default     = 1
  nullable    = false

  validation {
    condition     = var.pdb_min_available >= 0
    error_message = "pdb_min_available must be 0 or more"
  }
}

variable "pdb_name" {
  description = "Name of the PodDisruptionBudget resource"
  type        = string
  default     = "pdb-valkey"

  validation {
    condition     = var.pdb_name != null && length(trimspace(var.pdb_name)) > 0
    error_message = "pdb_name must not be blank"
  }
}

variable "node_selector" {
  description = "Node selector for Valkey pods"
  type        = map(string)
  default     = {}
  nullable    = false
}

variable "tolerations" {
  description = "Tolerations for Valkey pods"
  type = list(object({
    key      = optional(string)
    operator = optional(string)
    value    = optional(string)
    effect   = optional(string)
  }))
  default  = []
  nullable = false
}
