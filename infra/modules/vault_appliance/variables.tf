variable "name" {
  description = "Base name applied to Vault resources (droplets, firewall, load balancer, volumes)."
  type        = string

  validation {
    condition     = length(trimspace(var.name)) >= 3
    error_message = "name must contain at least three characters."
  }

  validation {
    condition     = can(regex("^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$", var.name))
    error_message = "name must be lowercase alphanumeric, may contain hyphens, and must not start or end with a hyphen."
  }

  validation {
    condition     = length(trimspace(var.name)) <= 63
    error_message = "name must be 63 characters or fewer."
  }
}

variable "region" {
  description = "DigitalOcean region slug (e.g., nyc1, sfo3)."
  type        = string

  validation {
    condition     = can(regex("^[a-z]{3}\\d$", trimspace(var.region)))
    error_message = "region must be a valid DigitalOcean region slug."
  }
}

variable "tags" {
  description = "Tags applied to droplets and ancillary resources."
  type        = list(string)
  default     = []
}

variable "ha_enabled" {
  description = "When true, create a high-availability pair of droplets behind the load balancer."
  type        = bool
  default     = false
}

variable "droplet_size" {
  description = "Droplet size slug (CPU and memory) for Vault nodes."
  type        = string
  default     = "s-2vcpu-4gb"

  validation {
    condition     = trimspace(var.droplet_size) != ""
    error_message = "droplet_size must not be empty."
  }
}

variable "droplet_image" {
  description = "Droplet image slug (e.g., ubuntu-22-04-x64)."
  type        = string
  default     = "ubuntu-22-04-x64"

  validation {
    condition     = trimspace(var.droplet_image) != ""
    error_message = "droplet_image must not be empty."
  }
}

variable "ssh_keys" {
  description = "List of SSH key fingerprints or IDs to inject into droplets."
  type        = list(string)
  default     = []
}

variable "user_data" {
  description = "Optional cloud-init user data applied to each droplet."
  type        = string
  default     = null
}

variable "monitoring_enabled" {
  description = "Enable DigitalOcean monitoring agent on droplets."
  type        = bool
  default     = true
}

variable "backups_enabled" {
  description = "Enable automated DigitalOcean backups for Vault droplets."
  type        = bool
  default     = false
}

variable "enable_ipv6" {
  description = "Enable IPv6 networking on droplets."
  type        = bool
  default     = true
}

variable "vpc_uuid" {
  description = "Optional VPC UUID for private networking."
  type        = string
  default     = null
}

variable "volume_size_gb" {
  description = "Size of each Vault data volume in gibibytes."
  type        = number
  default     = 50

  validation {
    condition     = var.volume_size_gb >= 25
    error_message = "volume_size_gb must be at least 25 GiB to accommodate Vault storage and logs."
  }
}

variable "volume_filesystem_type" {
  description = "Filesystem type created on the Vault data volume."
  type        = string
  default     = "ext4"

  validation {
    condition     = contains(["ext4", "xfs"], var.volume_filesystem_type)
    error_message = "volume_filesystem_type must be either ext4 or xfs."
  }
}

variable "allowed_ssh_cidrs" {
  description = "CIDR blocks permitted to reach the droplets over SSH. Leave empty to disable SSH ingress."
  type        = list(string)
  default     = []

  validation {
    condition = alltrue([
      for cidr in var.allowed_ssh_cidrs :
      can(cidrhost(cidr, 0))
    ])
    error_message = "allowed_ssh_cidrs must contain valid CIDR blocks."
  }
}

variable "load_balancer_size" {
  description = "DigitalOcean load balancer size slug (e.g., lb-small)."
  type        = string
  default     = "lb-small"

  validation {
    condition     = trimspace(var.load_balancer_size) != ""
    error_message = "load_balancer_size must not be empty."
  }
}

variable "load_balancer_algorithm" {
  description = "Load balancer balancing algorithm."
  type        = string
  default     = "round_robin"

  validation {
    condition     = contains(["round_robin", "least_connections"], var.load_balancer_algorithm)
    error_message = "load_balancer_algorithm must be round_robin or least_connections."
  }
}

variable "load_balancer_redirect_http_to_https" {
  description = "Automatically redirect HTTP traffic on port 80 to HTTPS on port 443."
  type        = bool
  default     = true
}

variable "load_balancer_enable_proxy_protocol" {
  description = "Enable proxy protocol support on the load balancer."
  type        = bool
  default     = false
}

variable "certificate_common_name" {
  description = "Common Name embedded in the Vault TLS certificate."
  type        = string

  validation {
    condition     = trimspace(var.certificate_common_name) != ""
    error_message = "certificate_common_name must not be empty."
  }
}

variable "certificate_dns_names" {
  description = "Additional DNS Subject Alternative Names for the Vault TLS certificate."
  type        = list(string)
  default     = []

  validation {
    condition = alltrue([
      for name in var.certificate_dns_names :
      can(regex("^(\\*\\.)?([a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\\.)+[a-z]{2,}$", lower(trimspace(name))))
    ])
    error_message = "certificate_dns_names must be valid DNS names (FQDNs); a leading wildcard (*.domain) is permitted."
  }
}

variable "certificate_ip_sans" {
  description = "IP Subject Alternative Names for the Vault TLS certificate."
  type        = list(string)
  default     = []

  validation {
    condition = alltrue([
      for ip in var.certificate_ip_sans :
      trimspace(ip) != "" &&
      can(
        cidrhost(
          format(
            "%s/%d",
            trimspace(ip),
            contains(trimspace(ip), ":") ? 128 : 32,
          ),
          0
        )
      )
    ])
    error_message = "certificate_ip_sans must contain valid IPv4 or IPv6 addresses."
  }
}

variable "certificate_validity_hours" {
  description = "Certificate validity period in hours."
  type        = number
  default     = 8760

  validation {
    condition     = var.certificate_validity_hours >= 720
    error_message = "certificate_validity_hours must be at least 720 hours (30 days)."
  }
}

variable "certificate_organisation" {
  description = "Organisation name embedded in the certificate subject."
  type        = string
  default     = "Wildside"
}

variable "recovery_shares" {
  description = "Number of recovery key shares to pre-generate."
  type        = number
  default     = 5

  validation {
    condition     = var.recovery_shares >= 1 && var.recovery_shares <= 10
    error_message = "recovery_shares must be between 1 and 10."
  }
}

variable "recovery_threshold" {
  description = "Number of shares required to reconstruct the recovery key."
  type        = number
  default     = 3

  validation {
    condition     = var.recovery_threshold >= 1
    error_message = "recovery_threshold must be at least 1."
  }

  validation {
    condition     = var.recovery_threshold <= var.recovery_shares
    error_message = "recovery_threshold cannot exceed recovery_shares."
  }
}

variable "recovery_key_length" {
  description = "Length of each generated recovery key."
  type        = number
  default     = 32

  validation {
    condition     = var.recovery_key_length >= 16 && var.recovery_key_length <= 64
    error_message = "recovery_key_length must be between 16 and 64 characters."
  }
}

variable "api_port" {
  description = "Vault API port exposed via the load balancer."
  type        = number
  default     = 8200

  validation {
    condition     = var.api_port >= 1 && var.api_port <= 65535
    error_message = "api_port must be between 1 and 65535."
  }
}

variable "cluster_port" {
  description = "Vault cluster port used for intra-node communication."
  type        = number
  default     = 8201

  validation {
    condition     = var.cluster_port >= 1 && var.cluster_port <= 65535
    error_message = "cluster_port must be between 1 and 65535."
  }

  validation {
    condition     = var.cluster_port != var.api_port
    error_message = "cluster_port must not equal api_port."
  }
}

variable "healthcheck_path" {
  description = "HTTP path probed by the load balancer health check."
  type        = string
  default     = "/v1/sys/health"

  validation {
    condition     = startswith(var.healthcheck_path, "/")
    error_message = "healthcheck_path must begin with a forward slash."
  }
}

variable "healthcheck_port" {
  description = "Optional override for the health check port. Defaults to api_port when null."
  type        = number
  default     = null
}

variable "healthcheck_interval_seconds" {
  description = "Interval between health checks."
  type        = number
  default     = 10

  validation {
    condition     = var.healthcheck_interval_seconds >= 5
    error_message = "healthcheck_interval_seconds must be at least 5 seconds."
  }
}

variable "healthcheck_timeout_seconds" {
  description = "Health check response timeout."
  type        = number
  default     = 5

  validation {
    condition     = var.healthcheck_timeout_seconds >= 3
    error_message = "healthcheck_timeout_seconds must be at least 3 seconds."
  }

  validation {
    condition     = var.healthcheck_timeout_seconds < var.healthcheck_interval_seconds
    error_message = "healthcheck_timeout_seconds must be less than healthcheck_interval_seconds."
  }
}

variable "healthcheck_unhealthy_threshold" {
  description = "Number of consecutive failures before marking a node unhealthy."
  type        = number
  default     = 3

  validation {
    condition     = var.healthcheck_unhealthy_threshold >= 2
    error_message = "healthcheck_unhealthy_threshold must be at least 2."
  }
}

variable "healthcheck_healthy_threshold" {
  description = "Number of successes required to mark a node healthy."
  type        = number
  default     = 5

  validation {
    condition     = var.healthcheck_healthy_threshold >= 2
    error_message = "healthcheck_healthy_threshold must be at least 2."
  }
}

variable "project_id" {
  description = "Optional DigitalOcean project ID to which all resources will be assigned."
  type        = string
  default     = null
}
