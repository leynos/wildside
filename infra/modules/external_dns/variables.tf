variable "namespace" {
  description = "Namespace where ExternalDNS will be installed"
  type        = string
  default     = "external-dns"

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
    condition     = contains(["render", "apply"], trimspace(var.mode))
    error_message = "mode must be one of: render, apply"
  }
}

variable "create_namespace" {
  description = "Whether the module should create the ExternalDNS namespace"
  type        = bool
  default     = true
  nullable    = false
}

variable "chart_repository" {
  description = "Helm repository hosting the ExternalDNS chart"
  type        = string
  default     = "https://kubernetes-sigs.github.io/external-dns/"

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
  description = "Name of the Helm chart used to install ExternalDNS"
  type        = string
  default     = "external-dns"

  validation {
    condition     = var.chart_name != null && length(trimspace(var.chart_name)) > 0
    error_message = "chart_name must not be blank"
  }
}

variable "chart_version" {
  description = "Exact Helm chart version for ExternalDNS"
  type        = string
  default     = "1.19.0"

  validation {
    condition = (
      var.chart_version != null &&
      can(regex(
        "^[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?(\\+[0-9A-Za-z.-]+)?$",
        trimspace(var.chart_version)
      ))
    )
    error_message = "chart_version must be a semantic version (e.g., 1.16.1, 1.16.1-rc1)"
  }
}

variable "helm_release_name" {
  description = "Name assigned to the ExternalDNS Helm release"
  type        = string
  default     = "external-dns"

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
  description = "Inline YAML values passed to the ExternalDNS Helm release"
  type        = list(string)
  default     = []
  nullable    = false
}

variable "helm_values_files" {
  description = "Additional YAML files providing values for the ExternalDNS Helm release"
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

variable "domain_filters" {
  description = "List of DNS domains that ExternalDNS should manage (e.g., [\"example.com\", \"example.org\"])"
  type        = list(string)

  validation {
    condition = (
      var.domain_filters != null &&
      length(var.domain_filters) > 0 &&
      alltrue([
        for domain in var.domain_filters : (
          domain != null &&
          length(trimspace(domain)) > 0 &&
          can(regex("^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\\.)+[a-z]{2,}$", lower(trimspace(domain))))
        )
      ])
    )
    error_message = "domain_filters must contain at least one valid domain name"
  }
}

variable "txt_owner_id" {
  description = "Unique identifier for ExternalDNS ownership TXT records (prevents conflicts between instances)"
  type        = string

  validation {
    condition = (
      var.txt_owner_id != null &&
      length(trimspace(var.txt_owner_id)) > 0 &&
      length(trimspace(var.txt_owner_id)) <= 255 &&
      can(regex("^[a-zA-Z0-9]([a-zA-Z0-9._-]*[a-zA-Z0-9])?$", trimspace(var.txt_owner_id)))
    )
    error_message = "txt_owner_id must be a non-empty alphanumeric string (may contain dots, underscores, hyphens)"
  }
}

variable "policy" {
  description = "DNS record management policy: sync (create, update, delete) or upsert-only (create, update)"
  type        = string
  default     = "sync"
  nullable    = false

  validation {
    condition     = contains(["sync", "upsert-only"], trimspace(var.policy))
    error_message = "policy must be one of: sync, upsert-only"
  }
}

variable "sources" {
  description = "Kubernetes resource types that ExternalDNS should watch for DNS annotations"
  type        = list(string)
  default     = ["ingress", "service"]
  nullable    = false

  validation {
    condition = (
      length(var.sources) > 0 &&
      alltrue([
        for source in var.sources : contains(
          ["ingress", "service", "crd", "gateway-httproute", "gateway-grpcroute", "gateway-tlsroute", "gateway-tcproute", "gateway-udproute"],
          trimspace(source)
        )
      ])
    )
    error_message = "sources must contain valid ExternalDNS source types"
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

variable "cloudflare_proxied" {
  description = "Whether to enable Cloudflare proxy (orange cloud) for DNS records by default"
  type        = bool
  default     = false
  nullable    = false
}

variable "dns_records_per_page" {
  description = "Number of DNS records to fetch per API page (higher values reduce API calls)"
  type        = number
  default     = 5000
  nullable    = false

  validation {
    condition     = var.dns_records_per_page >= 100 && var.dns_records_per_page <= 5000
    error_message = "dns_records_per_page must be between 100 and 5000"
  }
}

variable "log_level" {
  description = "Log verbosity level for ExternalDNS"
  type        = string
  default     = "info"
  nullable    = false

  validation {
    condition     = contains(["debug", "info", "warning", "error"], trimspace(var.log_level))
    error_message = "log_level must be one of: debug, info, warning, error"
  }
}

variable "crd_enabled" {
  description = "Whether to enable the DNSEndpoint Custom Resource Definition"
  type        = bool
  default     = true
  nullable    = false
}

variable "service_monitor_enabled" {
  description = "Whether to create a ServiceMonitor for Prometheus Operator"
  type        = bool
  default     = false
  nullable    = false
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
  description = "Name of the Flux HelmRepository resource providing the ExternalDNS chart (used when rendering Flux manifests)"
  type        = string
  default     = "external-dns"
  nullable    = false

  validation {
    condition = (
      length(trimspace(var.flux_helm_repository_name)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.flux_helm_repository_name)))
    )
    error_message = "flux_helm_repository_name must be a valid Kubernetes resource name"
  }
}

variable "interval" {
  description = "Interval between DNS record synchronisation cycles"
  type        = string
  default     = "1m"
  nullable    = false

  validation {
    condition = (
      can(regex("^([0-9]+[smh])+$", trimspace(var.interval)))
    )
    error_message = "interval must be a valid duration (e.g., 1m, 5m, 1h, 1h30m)"
  }
}

variable "registry_type" {
  description = "Registry type for tracking DNS record ownership"
  type        = string
  default     = "txt"
  nullable    = false

  validation {
    condition     = contains(["txt", "noop", "dynamodb", "aws-sd"], trimspace(var.registry_type))
    error_message = "registry_type must be one of: txt, noop, dynamodb, aws-sd"
  }
}

variable "txt_prefix" {
  description = "Prefix for TXT ownership records"
  type        = string
  default     = ""
  nullable    = false
}

variable "txt_suffix" {
  description = "Suffix for TXT ownership records"
  type        = string
  default     = ""
  nullable    = false
}
