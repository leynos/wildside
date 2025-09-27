locals {
  name_lower = lower(trimspace(var.name))
  name_chars = regexall(".", local.name_lower)
  raw_name_chars = [
    for ch in local.name_chars : can(regex("[a-z0-9-]", ch)) ? ch : "-"
  ]
  deduped_name_chars = [
    for idx in range(length(local.raw_name_chars)) : (
      local.raw_name_chars[idx] == "-" && idx > 0 && local.raw_name_chars[idx - 1] == "-" ? "" : local.raw_name_chars[idx]
    )
  ]
  sanitized_name = join("", compact(local.deduped_name_chars))
  base_candidate = trim(local.sanitized_name, "-")
  base_name      = substr(local.base_candidate != "" ? local.base_candidate : "vault", 0, 40)

  droplet_count = var.ha_enabled ? 2 : 1
  droplet_names = [
    for idx in range(local.droplet_count) :
    substr(format("%s-%02d", local.base_name, idx + 1), 0, 63)
  ]

  tags = distinct(compact(concat([
    local.base_name,
    "vault",
  ], [for tag in var.tags : trimspace(lower(tag))])))

  volume_name_prefix = substr(format("%s-data", local.base_name), 0, 55)
  volume_names = [
    for idx in range(local.droplet_count) :
    substr(format("%s-%02d", local.volume_name_prefix, idx + 1), 0, 63)
  ]

  firewall_name      = substr(format("%s-fw", local.base_name), 0, 50)
  load_balancer_name = substr(format("%s-lb", local.base_name), 0, 32)
  certificate_name   = substr(format("%s-cert", local.base_name), 0, 63)

  healthcheck_port = coalesce(var.healthcheck_port, var.api_port)

  certificate_dns = distinct(compact(concat(var.certificate_dns_names, [var.certificate_common_name])))
  certificate_ips = distinct(var.certificate_ip_sans)

  ca_validity_hours = max(var.certificate_validity_hours + 8760, var.certificate_validity_hours * 2)
  server_early_renewal_hours = min(
    var.certificate_validity_hours - 24,
    max(24, floor(var.certificate_validity_hours / 10))
  )
}

resource "digitalocean_droplet" "vault" {
  count = local.droplet_count

  name       = local.droplet_names[count.index]
  region     = var.region
  size       = var.droplet_size
  image      = var.droplet_image
  ssh_keys   = var.ssh_keys
  user_data  = var.user_data
  monitoring = var.monitoring_enabled
  backups    = var.backups_enabled
  ipv6       = var.enable_ipv6
  vpc_uuid   = var.vpc_uuid
  tags       = local.tags
}

resource "digitalocean_volume" "vault_data" {
  count = local.droplet_count

  region          = var.region
  name            = local.volume_names[count.index]
  size            = var.volume_size_gb
  description     = "Vault data volume"
  filesystem_type = var.volume_filesystem_type
}

resource "digitalocean_volume_attachment" "vault" {
  count = local.droplet_count

  droplet_id = digitalocean_droplet.vault[count.index].id
  volume_id  = digitalocean_volume.vault_data[count.index].id
}

resource "tls_private_key" "ca" {
  algorithm   = "ECDSA"
  ecdsa_curve = "P256"
}

resource "tls_self_signed_cert" "ca" {
  private_key_pem = tls_private_key.ca.private_key_pem

  subject {
    common_name  = "${var.certificate_common_name} Root CA"
    organization = var.certificate_organisation
  }

  is_ca_certificate     = true
  validity_period_hours = local.ca_validity_hours
  early_renewal_hours   = local.server_early_renewal_hours

  allowed_uses = [
    "cert_signing",
    "crl_signing",
  ]
}

resource "tls_private_key" "server" {
  algorithm = "RSA"
  rsa_bits  = 4096
}

resource "tls_cert_request" "server" {
  private_key_pem = tls_private_key.server.private_key_pem

  subject {
    common_name  = var.certificate_common_name
    organization = var.certificate_organisation
  }

  dns_names    = local.certificate_dns
  ip_addresses = local.certificate_ips
}

resource "tls_locally_signed_cert" "server" {
  cert_request_pem   = tls_cert_request.server.cert_request_pem
  ca_private_key_pem = tls_private_key.ca.private_key_pem
  ca_cert_pem        = tls_self_signed_cert.ca.cert_pem

  validity_period_hours = var.certificate_validity_hours
  early_renewal_hours   = local.server_early_renewal_hours

  allowed_uses = [
    "digital_signature",
    "key_encipherment",
    "server_auth",
    "client_auth",
  ]
}

resource "digitalocean_certificate" "vault" {
  name              = local.certificate_name
  private_key       = tls_private_key.server.private_key_pem
  leaf_certificate  = tls_locally_signed_cert.server.cert_pem
  certificate_chain = tls_self_signed_cert.ca.cert_pem
  type              = "custom"
}

resource "digitalocean_loadbalancer" "vault" {
  name      = local.load_balancer_name
  region    = var.region
  size      = var.load_balancer_size
  algorithm = var.load_balancer_algorithm

  vpc_uuid               = var.vpc_uuid
  droplet_ids            = digitalocean_droplet.vault[*].id
  redirect_http_to_https = var.load_balancer_redirect_http_to_https
  enable_proxy_protocol  = var.load_balancer_enable_proxy_protocol

  forwarding_rule {
    entry_protocol  = "https"
    entry_port      = 443
    target_protocol = "http"
    target_port     = var.api_port
    certificate_id  = digitalocean_certificate.vault.id
    tls_passthrough = false
  }

  healthcheck {
    protocol                 = "http"
    port                     = local.healthcheck_port
    path                     = var.healthcheck_path
    check_interval_seconds   = var.healthcheck_interval_seconds
    response_timeout_seconds = var.healthcheck_timeout_seconds
    unhealthy_threshold      = var.healthcheck_unhealthy_threshold
    healthy_threshold        = var.healthcheck_healthy_threshold
  }

  sticky_sessions {
    type = "none"
  }

  depends_on = [digitalocean_certificate.vault]
}

resource "digitalocean_firewall" "vault" {
  name        = local.firewall_name
  droplet_ids = digitalocean_droplet.vault[*].id
  tags        = local.tags

  dynamic "inbound_rule" {
    for_each = length(var.allowed_ssh_cidrs) > 0 ? [var.allowed_ssh_cidrs] : []
    content {
      protocol         = "tcp"
      port_range       = "22"
      source_addresses = inbound_rule.value
    }
  }

  inbound_rule {
    protocol                  = "tcp"
    port_range                = tostring(var.api_port)
    source_load_balancer_uids = [digitalocean_loadbalancer.vault.id]
  }

  dynamic "inbound_rule" {
    for_each = local.healthcheck_port != var.api_port ? [local.healthcheck_port] : []
    content {
      protocol                  = "tcp"
      port_range                = tostring(inbound_rule.value)
      source_load_balancer_uids = [digitalocean_loadbalancer.vault.id]
    }
  }

  inbound_rule {
    protocol           = "tcp"
    port_range         = tostring(var.cluster_port)
    source_droplet_ids = digitalocean_droplet.vault[*].id
  }

  outbound_rule {
    protocol              = "tcp"
    port_range            = "all"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }

  outbound_rule {
    protocol              = "udp"
    port_range            = "all"
    destination_addresses = ["0.0.0.0/0", "::/0"]
  }
}

resource "random_password" "recovery_keys" {
  count = var.recovery_shares

  length  = var.recovery_key_length
  upper   = true
  lower   = true
  numeric = true
  special = true
}

resource "digitalocean_project_resources" "vault" {
  count = var.project_id == null ? 0 : 1

  project = var.project_id
  resources = concat(
    [digitalocean_loadbalancer.vault.urn],
    [for d in digitalocean_droplet.vault : d.urn],
    [for v in digitalocean_volume.vault_data : v.urn]
  )
}
