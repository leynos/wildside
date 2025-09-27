output "public_endpoint" {
  description = "Public endpoint served by the Vault load balancer."
  value = {
    name = digitalocean_loadbalancer.vault.name
    ip   = digitalocean_loadbalancer.vault.ip
    port = 443
  }
}

output "load_balancer_id" {
  description = "Identifier of the DigitalOcean load balancer."
  value       = digitalocean_loadbalancer.vault.id
}

output "droplet_ids" {
  description = "Identifiers of the Vault droplets."
  value       = digitalocean_droplet.vault[*].id
}

output "droplet_ipv4_addresses" {
  description = "Public IPv4 addresses assigned to the Vault droplets."
  value       = digitalocean_droplet.vault[*].ipv4_address
}

output "droplet_private_ipv4_addresses" {
  description = "Private IPv4 addresses of the droplets inside the VPC."
  value       = digitalocean_droplet.vault[*].ipv4_address_private
}

output "ca_certificate" {
  description = "PEM-encoded certificate authority bundle used to sign the Vault server certificate."
  value       = tls_self_signed_cert.ca.cert_pem
}

output "server_certificate" {
  description = "PEM-encoded Vault server certificate signed by the module CA."
  value       = tls_locally_signed_cert.server.cert_pem
}

output "server_private_key" {
  description = "PEM-encoded private key matching the Vault server certificate."
  value       = tls_private_key.server.private_key_pem
  sensitive   = true
}

output "recovery_keys" {
  description = "Pre-generated recovery keys for Vault initialisation."
  value       = random_password.recovery_keys[*].result
  sensitive   = true
}

output "recovery_threshold" {
  description = "Number of recovery keys required to unseal Vault."
  value       = var.recovery_threshold
}
