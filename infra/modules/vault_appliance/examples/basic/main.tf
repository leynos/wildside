module "vault_appliance" {
  source = "../.."

  name                    = var.name
  region                  = var.region
  tags                    = var.tags
  ha_enabled              = var.ha_enabled
  allowed_ssh_cidrs       = var.allowed_ssh_cidrs
  certificate_common_name = var.certificate_common_name
  certificate_dns_names   = var.certificate_dns_names
  certificate_ip_sans     = var.certificate_ip_sans
  project_id              = var.project_id
  recovery_shares         = var.recovery_shares
  recovery_threshold      = var.recovery_threshold
  recovery_key_length     = var.recovery_key_length
  ssh_keys                = var.ssh_keys
}

variable "name" {
  type        = string
  description = "Base name applied to Vault resources"
}

variable "region" {
  type        = string
  description = "DigitalOcean region (e.g., nyc1, sfo3)"
}

variable "tags" {
  type        = list(string)
  description = "Tags applied to the droplets and load balancer"
  default     = []
}

variable "ha_enabled" {
  type        = bool
  description = "Provision a high-availability pair of droplets"
  default     = false
}

variable "allowed_ssh_cidrs" {
  type        = list(string)
  description = "CIDR ranges permitted to access SSH"
  default     = []
}

variable "certificate_common_name" {
  type        = string
  description = "Common Name embedded in the Vault TLS certificate"
}

variable "certificate_dns_names" {
  type        = list(string)
  description = "Additional DNS SANs for the Vault TLS certificate"
  default     = []
}

variable "certificate_ip_sans" {
  type        = list(string)
  description = "IP SANs for the Vault TLS certificate"
  default     = []
}

variable "project_id" {
  type        = string
  description = "Optional DigitalOcean project ID"
  default     = null
}

variable "recovery_shares" {
  type        = number
  description = "Number of recovery key shares to generate"
  default     = 5
}

variable "recovery_threshold" {
  type        = number
  description = "Shares required to unseal Vault"
  default     = 3
}

variable "recovery_key_length" {
  type        = number
  description = "Length of each generated recovery key"
  default     = 32
}

variable "ssh_keys" {
  type        = list(string)
  description = "SSH key fingerprints or IDs to inject into droplets"
  default     = ["example-ssh-key-fingerprint"]
}

output "public_endpoint" {
  description = "Load balancer endpoint"
  value       = module.vault_appliance.public_endpoint
}

output "ca_certificate" {
  description = "Vault CA certificate"
  value       = module.vault_appliance.ca_certificate
}

output "recovery_keys" {
  description = "Generated recovery keys"
  value       = module.vault_appliance.recovery_keys
  sensitive   = true
}
