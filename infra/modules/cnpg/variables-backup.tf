variable "backup_enabled" {
  description = "Whether to enable S3-compatible backups for the cluster"
  type        = bool
  default     = false
  nullable    = false
}

variable "backup_destination_path" {
  description = "S3 bucket path for backups (e.g., s3://bucket-name/backups/)"
  type        = string
  default     = ""

  validation {
    condition = (
      var.backup_destination_path == "" ||
      can(regex("^s3://[a-z0-9][a-z0-9.-]*[a-z0-9](/.*)?$", trimspace(var.backup_destination_path)))
    )
    error_message = "backup_destination_path must be a valid S3 URI (e.g., s3://bucket-name/path/)"
  }
}

variable "backup_endpoint_url" {
  description = "S3-compatible endpoint URL (e.g., https://nyc3.digitaloceanspaces.com)"
  type        = string
  default     = ""

  validation {
    condition = (
      var.backup_endpoint_url == "" ||
      can(regex("^https://", trimspace(var.backup_endpoint_url)))
    )
    error_message = "backup_endpoint_url must be an https:// URL"
  }
}

variable "backup_retention_policy" {
  description = "Backup retention policy (e.g., 30d for 30 days)"
  type        = string
  default     = "30d"

  validation {
    condition = (
      var.backup_retention_policy != null &&
      can(regex("^[0-9]+d$", trimspace(var.backup_retention_policy)))
    )
    error_message = "backup_retention_policy must be in the format Nd (e.g., 30d)"
  }
}

variable "backup_schedule" {
  description = "Backup schedule in cron format (e.g., 0 0 * * * for daily at midnight)"
  type        = string
  default     = "0 0 * * *"

  validation {
    condition = (
      var.backup_schedule != null &&
      length(trimspace(var.backup_schedule)) > 0
    )
    error_message = "backup_schedule must not be blank"
  }
}

variable "backup_s3_credentials_secret_name" {
  description = "Kubernetes Secret name containing S3 access credentials"
  type        = string
  default     = "cnpg-s3-credentials"

  validation {
    condition = (
      var.backup_s3_credentials_secret_name != null &&
      length(trimspace(var.backup_s3_credentials_secret_name)) > 0
    )
    error_message = "backup_s3_credentials_secret_name must not be blank"
  }
}

variable "backup_s3_access_key_id" {
  description = "S3 access key ID for backups"
  type        = string
  default     = ""
  sensitive   = true
}

variable "backup_s3_secret_access_key" {
  description = "S3 secret access key for backups"
  type        = string
  default     = ""
  sensitive   = true
}

variable "wal_compression" {
  description = "Compression algorithm for WAL archiving (gzip, bzip2, snappy, lz4, zstd)"
  type        = string
  default     = "gzip"
  nullable    = false

  validation {
    condition     = contains(["gzip", "bzip2", "snappy", "lz4", "zstd"], trimspace(var.wal_compression))
    error_message = "wal_compression must be one of: gzip, bzip2, snappy, lz4, zstd"
  }
}

variable "scheduled_backup_name" {
  description = "Name for the scheduled backup resource"
  type        = string
  default     = "daily-backup"

  validation {
    condition = (
      var.scheduled_backup_name != null &&
      length(trimspace(var.scheduled_backup_name)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.scheduled_backup_name)))
    )
    error_message = "scheduled_backup_name must be a valid Kubernetes resource name"
  }
}
