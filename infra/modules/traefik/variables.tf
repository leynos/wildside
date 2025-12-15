variable "namespace" {
  description = "Namespace where Traefik will be installed"
  type        = string
  default     = "traefik"

  validation {
    condition = (
      var.namespace != null &&
      length(trimspace(var.namespace)) > 0 &&
      length(trimspace(var.namespace)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.namespace)))
    )
    error_message = "namespace must be a valid Kubernetes namespace name"
  }
}

variable "mode" {
  description = "Whether the module should render Flux manifests (render) or apply resources directly (apply)"
  type        = string
  default     = "render"
  nullable    = false

  validation {
    condition     = contains(["render", "apply"], var.mode)
    error_message = "mode must be one of: render, apply"
  }
}

variable "create_namespace" {
  description = "Whether the module should create the Traefik namespace"
  type        = bool
  default     = true
  nullable    = false
}

variable "chart_repository" {
  description = "Helm repository hosting the Traefik chart"
  type        = string
  default     = "https://traefik.github.io/charts"

  validation {
    condition = (
      var.chart_repository != null &&
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
    condition     = var.chart_name != null && length(trimspace(var.chart_name)) > 0
    error_message = "chart_name must not be blank"
  }
}

variable "chart_version" {
  description = "Exact Helm chart version for Traefik"
  type        = string
  default     = "37.4.0"

  validation {
    condition = (
      var.chart_version != null &&
      can(regex(
        "^[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?(\\+[0-9A-Za-z.-]+)?$",
        trimspace(var.chart_version)
      ))
    )
    error_message = "chart_version must be a semantic version (e.g., 37.4.0, 37.4.0-rc1, 37.4.0+build.1)"
  }
}

variable "helm_release_name" {
  description = "Name assigned to the Traefik Helm release"
  type        = string
  default     = "traefik"

  validation {
    condition     = var.helm_release_name != null && length(trimspace(var.helm_release_name)) > 0
    error_message = "helm_release_name must not be blank"
  }
}

variable "helm_wait" {
  description = "Whether to wait for the Helm release to report success"
  type        = bool
  default     = true
  nullable    = false
}

variable "helm_timeout" {
  description = "Timeout (in seconds) for the Helm release operation"
  type        = number
  default     = 600
  nullable    = false

  validation {
    condition     = var.helm_timeout > 0
    error_message = "helm_timeout must be a positive number of seconds"
  }
}

variable "helm_values" {
  description = "Inline YAML values passed to the Traefik Helm release"
  type        = list(string)
  default     = []
  nullable    = false
}

variable "helm_values_files" {
  description = "Additional YAML files providing values for the Traefik Helm release"
  type        = list(string)
  default     = []

  validation {
    condition = (
      var.helm_values_files != null &&
      alltrue([
        for path in var.helm_values_files : path != null && length(trimspace(path)) > 0
      ])
    )
    error_message = "helm_values_files must not contain blank file paths"
  }
}

variable "service_type" {
  description = "Kubernetes service type for Traefik (LoadBalancer, ClusterIP, or NodePort)"
  type        = string
  default     = "LoadBalancer"
  nullable    = false

  validation {
    condition     = contains(["LoadBalancer", "ClusterIP", "NodePort"], var.service_type)
    error_message = "service_type must be LoadBalancer, ClusterIP, or NodePort"
  }
}

variable "service_annotations" {
  description = "Service annotations to apply to Traefik's Service (cloud-provider load balancer configuration)"
  type        = map(string)
  default     = {}
  nullable    = false

  validation {
    condition = alltrue([
      for key, value in var.service_annotations : (
        key != null &&
        trimspace(key) != "" &&
        value != null &&
        trimspace(value) != ""
      )
    ])
    error_message = "service_annotations keys and values must not be blank"
  }
}

variable "external_traffic_policy" {
  description = "External traffic policy for LoadBalancer service (Local preserves client IPs)"
  type        = string
  default     = "Local"
  nullable    = false

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
    condition     = var.ingress_class_name != null && length(trimspace(var.ingress_class_name)) > 0
    error_message = "ingress_class_name must not be blank"
  }
}

variable "ingress_class_default" {
  description = "Whether the Traefik IngressClass should be the cluster default"
  type        = bool
  default     = false
  nullable    = false
}

variable "dashboard_enabled" {
  description = "Whether to enable the Traefik dashboard"
  type        = bool
  default     = false
  nullable    = false
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

variable "flux_namespace" {
  description = "Namespace where Flux controllers and sources are installed (used when rendering Flux manifests)"
  type        = string
  default     = "flux-system"
  nullable    = false

  validation {
    condition = (
      length(trimspace(var.flux_namespace)) > 0 &&
      length(trimspace(var.flux_namespace)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.flux_namespace)))
    )
    error_message = "flux_namespace must be a valid Kubernetes namespace name"
  }
}

variable "flux_helm_repository_name" {
  description = "Name of the Flux HelmRepository resource providing the Traefik chart (used when rendering Flux manifests)"
  type        = string
  default     = "traefik"
  nullable    = false

  validation {
    condition = (
      length(trimspace(var.flux_helm_repository_name)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.flux_helm_repository_name)))
    )
    error_message = "flux_helm_repository_name must be a valid Kubernetes resource name"
  }
}

variable "http_to_https_redirect" {
  description = "Whether to redirect HTTP traffic to HTTPS"
  type        = bool
  default     = true
  nullable    = false
}

variable "prometheus_metrics_enabled" {
  description = "Whether to enable Prometheus metrics endpoint"
  type        = bool
  default     = true
  nullable    = false
}

variable "service_monitor_enabled" {
  description = "Whether to create a ServiceMonitor for Prometheus Operator"
  type        = bool
  default     = true
  nullable    = false

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
  nullable = false
}

variable "acme_email" {
  description = "Email address registered with ACME certificate authority for notifications"
  type        = string

  validation {
    condition     = var.acme_email != null && can(regex("^[^@]+@[^@]+\\.[^@]+$", trimspace(var.acme_email)))
    error_message = "acme_email must be a valid email address"
  }
}

variable "acme_server" {
  description = "ACME server URL (production or staging)"
  type        = string
  default     = "https://acme-v02.api.letsencrypt.org/directory"

  validation {
    condition = (
      var.acme_server != null &&
      contains([
        "https://acme-v02.api.letsencrypt.org/directory",
        "https://acme-staging-v02.api.letsencrypt.org/directory"
      ], trimspace(var.acme_server))
    )
    error_message = "acme_server must be a valid Let's Encrypt production or staging URL"
  }
}

variable "cluster_issuer_name" {
  description = "Name of the ClusterIssuer resource"
  type        = string
  default     = "letsencrypt-prod"

  validation {
    condition = (
      var.cluster_issuer_name != null &&
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
    condition = (
      var.cloudflare_api_token_secret_name != null &&
      length(trimspace(var.cloudflare_api_token_secret_name)) > 0 &&
      length(trimspace(var.cloudflare_api_token_secret_name)) <= 253 &&
      can(regex(
        "^[a-z0-9]([-.a-z0-9]*[a-z0-9])?$",
        trimspace(var.cloudflare_api_token_secret_name)
      ))
    )
    error_message = "cloudflare_api_token_secret_name must be a valid Kubernetes Secret name"
  }
}

variable "cloudflare_api_token_secret_key" {
  description = "Key within the Cloudflare API token secret that holds the token value"
  type        = string
  default     = "token"

  validation {
    condition = (
      var.cloudflare_api_token_secret_key != null &&
      length(trimspace(var.cloudflare_api_token_secret_key)) > 0
    )
    error_message = "cloudflare_api_token_secret_key must not be blank"
  }
}
