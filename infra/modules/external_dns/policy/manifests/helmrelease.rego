package external_dns.policy.manifests

import rego.v1

documents := input if is_array(input)
documents := [input] if not is_array(input)

helmreleases := [doc |
	doc := documents[_]
	doc.kind == "HelmRelease"
]

helmrepositories := [doc |
	doc := documents[_]
	doc.kind == "HelmRepository"
]

external_dns_helmreleases := [doc |
	doc := helmreleases[_]
	metadata := object.get(doc, "metadata", {})
	lower(object.get(metadata, "name", "")) == "external-dns"
]

deny contains msg if {
	doc := external_dns_helmreleases[_]
	version := object.get(object.get(object.get(doc.spec, "chart", {}), "spec", {}), "version", "")
	trim_space(version) == ""
	msg := "ExternalDNS HelmRelease must pin chart.spec.version"
}

deny contains msg if {
	doc := external_dns_helmreleases[_]
	source_ref := object.get(object.get(object.get(doc.spec, "chart", {}), "spec", {}), "sourceRef", {})
	trim_space(object.get(source_ref, "name", "")) == ""
	msg := "ExternalDNS HelmRelease must set chart.spec.sourceRef.name"
}

deny contains msg if {
	doc := external_dns_helmreleases[_]
	source_ref := object.get(object.get(object.get(doc.spec, "chart", {}), "spec", {}), "sourceRef", {})
	trim_space(object.get(source_ref, "namespace", "")) == ""
	msg := "ExternalDNS HelmRelease must set chart.spec.sourceRef.namespace"
}

deny contains msg if {
	doc := external_dns_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	domain_filters := object.get(values, "domainFilters", [])
	count(domain_filters) == 0
	msg := "ExternalDNS HelmRelease must set values.domainFilters with at least one domain"
}

deny contains msg if {
	doc := external_dns_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	txt_owner_id := object.get(values, "txtOwnerId", "")
	trim_space(txt_owner_id) == ""
	msg := "ExternalDNS HelmRelease must set values.txtOwnerId for ownership tracking"
}

deny contains msg if {
	doc := external_dns_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	policy := object.get(values, "policy", "")
	policy != ""
	not policy in ["sync", "upsert-only"]
	msg := sprintf("ExternalDNS HelmRelease values.policy must be 'sync' or 'upsert-only', got '%s'", [policy])
}

deny contains msg if {
	doc := external_dns_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	provider := object.get(values, "provider", {})
	provider_name := object.get(provider, "name", "")
	provider_name != ""
	provider_name != "cloudflare"
	msg := sprintf("ExternalDNS HelmRelease currently only supports Cloudflare provider, got '%s'", [provider_name])
}

warn contains msg if {
	doc := helmrepositories[_]
	url := object.get(object.get(doc, "spec", {}), "url", "")
	url != ""
	startswith(url, "http://")
	metadata := object.get(doc, "metadata", {})
	msg := sprintf("HelmRepository %s should use an https:// URL", [object.get(metadata, "name", "<unknown>")])
}

warn contains msg if {
	doc := external_dns_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	policy := object.get(values, "policy", "")
	policy == "upsert-only"
	msg := "ExternalDNS policy 'upsert-only' will not remove stale DNS records when Kubernetes resources are deleted"
}
