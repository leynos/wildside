package external_dns.policy.plan

import rego.v1

# Helper to extract a Helm release resource change from an OpenTofu plan.
#
# Resource deletions may have `rc.change.after == null`, so this helper includes
# the guard to avoid runtime errors when policies evaluate deletes.
helm_release(rc) = {"name": name, "values": values} if {
	rc.type == "helm_release"
	after := rc.change.after
	after != null
	name := object.get(after, "name", "")
	values_list := object.get(after, "values", [])
	# Helm values are stored as a list of YAML strings; decode the first one
	count(values_list) > 0
	values := yaml.unmarshal(values_list[0])
}

# Helper to identify ExternalDNS Helm releases
external_dns_release(rc) = release if {
	release := helm_release(rc)
	release.name == "external-dns"
}

# Ensure ExternalDNS Helm release has domain filters configured.
deny contains msg if {
	rc := input.resource_changes[_]
	release := external_dns_release(rc)
	domain_filters := object.get(release.values, "domainFilters", [])
	count(domain_filters) == 0
	msg := "ExternalDNS Helm release must have domainFilters configured"
}

# Ensure ExternalDNS Helm release has txtOwnerId configured.
deny contains msg if {
	rc := input.resource_changes[_]
	release := external_dns_release(rc)
	txt_owner_id := object.get(release.values, "txtOwnerId", "")
	trim_space(txt_owner_id) == ""
	msg := "ExternalDNS Helm release must have txtOwnerId configured for ownership tracking"
}

# Ensure ExternalDNS Helm release has a valid policy.
deny contains msg if {
	rc := input.resource_changes[_]
	release := external_dns_release(rc)
	policy := object.get(release.values, "policy", "")
	policy != ""
	not policy in ["sync", "upsert-only"]
	msg := sprintf("ExternalDNS Helm release policy must be 'sync' or 'upsert-only', got '%s'", [policy])
}

# Ensure ExternalDNS Helm release uses Cloudflare provider.
deny contains msg if {
	rc := input.resource_changes[_]
	release := external_dns_release(rc)
	provider := object.get(release.values, "provider", {})
	provider_name := object.get(provider, "name", "")
	provider_name != ""
	provider_name != "cloudflare"
	msg := sprintf("ExternalDNS Helm release currently only supports Cloudflare provider, got '%s'", [provider_name])
}

# Ensure ExternalDNS Helm release has API token environment variable configured.
deny contains msg if {
	rc := input.resource_changes[_]
	release := external_dns_release(rc)
	env_vars := object.get(release.values, "env", [])
	not has_cf_api_token_env(env_vars)
	msg := "ExternalDNS Helm release must have CF_API_TOKEN environment variable configured"
}

# Helper to check if CF_API_TOKEN environment variable is configured
has_cf_api_token_env(env_vars) if {
	some env in env_vars
	object.get(env, "name", "") == "CF_API_TOKEN"
}

# Warn when using upsert-only policy (stale records will not be deleted).
warn contains msg if {
	rc := input.resource_changes[_]
	release := external_dns_release(rc)
	policy := object.get(release.values, "policy", "")
	policy == "upsert-only"
	msg := "ExternalDNS policy 'upsert-only' will not remove stale DNS records when Kubernetes resources are deleted"
}

# Warn when CRD is disabled (DNSEndpoint resources will not work).
warn contains msg if {
	rc := input.resource_changes[_]
	release := external_dns_release(rc)
	crd := object.get(release.values, "crd", {})
	crd_create := object.get(crd, "create", true)
	crd_create == false
	msg := "ExternalDNS CRD creation is disabled; DNSEndpoint resources will not be available"
}
