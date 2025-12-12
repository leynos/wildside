variable "namespace" {
  description = "Namespace where Traefik will be installed"
  type        = string
  default     = "traefik"

  validation {
    condition = (
      length(trimspace(var.namespace)) > 0 &&
      length(trimspace(var.namespace)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.namespace)))
    )
    error_message = "namespace must be a valid Kubernetes namespace name"
  }
}

variable "create_namespace" {
  description = "Whether the module should create the Traefik namespace"
  type        = bool
  default     = true
}

variable "chart_repository" {
  description = "Helm repository hosting the Traefik chart"
  type        = string
  default     = "https://traefik.github.io/charts"

  validation {
    condition = (
      length(trimspace(var.chart_repository)) > 0 &&
      can(regex("^https://", trimspace(var.chart_repository)))
    )
    error_message = "chart_repository must be an HTTPS URL"
  }
}

variable "chart_name" {
  description = "Name of the Helm chart used to install Traefik"
  type        = string
  default     = "traefik"

  validation {
    condition     = length(trimspace(var.chart_name)) > 0
    error_message = "chart_name must not be blank"
  }
}

variable "chart_version" {
  description = "Exact Helm chart version for Traefik"
  type        = string
  default     = "25.0.3"

  validation {
    condition     = can(regex("^[0-9]+\\.[0-9]+\\.[0-9]+(-[a-zA-Z0-9.]+)?$", trimspace(var.chart_version)))
    error_message = "chart_version must be a semantic version (e.g., 25.0.3)"
  }
}

variable "helm_release_name" {
  description = "Name assigned to the Traefik Helm release"
  type        = string
  default     = "traefik"

  validation {
    condition     = length(trimspace(var.helm_release_name)) > 0
    error_message = "helm_release_name must not be blank"
  }
}

variable "helm_wait" {
  description = "Whether to wait for the Helm release to report success"
  type        = bool
  default     = true
}

variable "helm_timeout" {
  description = "Timeout (in seconds) for the Helm release operation"
  type        = number
  default     = 600

  validation {
    condition     = var.helm_timeout > 0
    error_message = "helm_timeout must be a positive number of seconds"
  }
}

variable "helm_values" {
  description = "Inline YAML values passed to the Traefik Helm release"
  type        = list(string)
  default     = []
}

variable "helm_values_files" {
  description = "Additional YAML files providing values for the Traefik Helm release"
  type        = list(string)
  default     = []

  validation {
    condition = alltrue([
      for path in var.helm_values_files : length(trimspace(path)) > 0
    ])
    error_message = "helm_values_files must not contain blank file paths"
  }
}

variable "service_type" {
  description = "Kubernetes service type for Traefik (LoadBalancer, ClusterIP, or NodePort)"
  type        = string
  default     = "LoadBalancer"

  validation {
    condition     = contains(["LoadBalancer", "ClusterIP", "NodePort"], var.service_type)
    error_message = "service_type must be LoadBalancer, ClusterIP, or NodePort"
  }
}

variable "external_traffic_policy" {
  description = "External traffic policy for LoadBalancer service (Local preserves client IPs)"
  type        = string
  default     = "Local"

  validation {
    condition     = contains(["Local", "Cluster"], var.external_traffic_policy)
    error_message = "external_traffic_policy must be Local or Cluster"
  }
}

variable "ingress_class_name" {
  description = "Name of the IngressClass created by Traefik"
  type        = string
  default     = "traefik"

  validation {
    condition     = length(trimspace(var.ingress_class_name)) > 0
    error_message = "ingress_class_name must not be blank"
  }
}

variable "ingress_class_default" {
  description = "Whether the Traefik IngressClass should be the cluster default"
  type        = bool
  default     = false
}

variable "dashboard_enabled" {
  description = "Whether to enable the Traefik dashboard"
  type        = bool
  default     = false
}

variable "dashboard_hostname" {
  description = "Hostname for the Traefik dashboard IngressRoute (required when dashboard_enabled)"
  type        = string
  default     = null

  validation {
    condition = (
      (
        var.dashboard_enabled == false &&
        var.dashboard_hostname == null
      ) ||
      (
        var.dashboard_hostname != null &&
        can(regex(
          "^([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\\.)+[a-z]{2,}$",
          lower(trimspace(var.dashboard_hostname))
        ))
      )
    )
    error_message = "dashboard_hostname must be a valid FQDN, and must be set when dashboard_enabled is true"
  }
}

variable "http_to_https_redirect" {
  description = "Whether to redirect HTTP traffic to HTTPS"
  type        = bool
  default     = true
}

variable "prometheus_metrics_enabled" {
  description = "Whether to enable Prometheus metrics endpoint"
  type        = bool
  default     = true
}

variable "service_monitor_enabled" {
  description = "Whether to create a ServiceMonitor for Prometheus Operator"
  type        = bool
  default     = true

  validation {
    condition     = var.service_monitor_enabled == false || var.prometheus_metrics_enabled == true
    error_message = "service_monitor_enabled requires prometheus_metrics_enabled to be true"
  }
}

variable "tolerations" {
  description = "Tolerations for Traefik pod scheduling"
  type = list(object({
    key      = string
    operator = string
    effect   = string
    value    = optional(string)
  }))
  default = [{
    key      = "CriticalAddonsOnly"
    operator = "Exists"
    effect   = "NoSchedule"
  }]
}

variable "acme_email" {
  description = "Email address registered with ACME certificate authority for notifications"
  type        = string

  validation {
    condition     = can(regex("^[^@]+@[^@]+\\.[^@]+$", trimspace(var.acme_email)))
    error_message = "acme_email must be a valid email address"
  }
}

variable "acme_server" {
  description = "ACME server URL (production or staging)"
  type        = string
  default     = "https://acme-v02.api.letsencrypt.org/directory"

  validation {
    condition = contains([
      "https://acme-v02.api.letsencrypt.org/directory",
      "https://acme-staging-v02.api.letsencrypt.org/directory"
    ], trimspace(var.acme_server))
    error_message = "acme_server must be a valid Let's Encrypt production or staging URL"
  }
}

variable "cluster_issuer_name" {
  description = "Name of the ClusterIssuer resource"
  type        = string
  default     = "letsencrypt-prod"

  validation {
    condition = (
      length(trimspace(var.cluster_issuer_name)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.cluster_issuer_name)))
    )
    error_message = "cluster_issuer_name must be a valid Kubernetes resource name"
  }
}

variable "cloudflare_api_token_secret_name" {
  description = "Name of the Kubernetes secret containing the Cloudflare API token"
  type        = string

  validation {
    condition     = length(trimspace(var.cloudflare_api_token_secret_name)) > 0
    error_message = "cloudflare_api_token_secret_name must not be blank"
  }
}

variable "cloudflare_api_token_secret_key" {
  description = "Key within the Cloudflare API token secret that holds the token value"
  type        = string
  default     = "token"

  validation {
    condition     = length(trimspace(var.cloudflare_api_token_secret_key)) > 0
    error_message = "cloudflare_api_token_secret_key must not be blank"
  }
}
