package traefik.policy.manifests

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

traefik_helmreleases := [doc |
	doc := helmreleases[_]
	metadata := object.get(doc, "metadata", {})
	lower(object.get(metadata, "name", "")) == "traefik"
]

deny contains msg if {
	doc := traefik_helmreleases[_]
	version := object.get(object.get(object.get(doc.spec, "chart", {}), "spec", {}), "version", "")
	trim_space(version) == ""
	msg := "Traefik HelmRelease must pin chart.spec.version"
}

deny contains msg if {
	doc := traefik_helmreleases[_]
	source_ref := object.get(object.get(object.get(doc.spec, "chart", {}), "spec", {}), "sourceRef", {})
	trim_space(object.get(source_ref, "name", "")) == ""
	msg := "Traefik HelmRelease must set chart.spec.sourceRef.name"
}

deny contains msg if {
	doc := traefik_helmreleases[_]
	source_ref := object.get(object.get(object.get(doc.spec, "chart", {}), "spec", {}), "sourceRef", {})
	trim_space(object.get(source_ref, "namespace", "")) == ""
	msg := "Traefik HelmRelease must set chart.spec.sourceRef.namespace"
}

deny contains msg if {
	doc := traefik_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	service := object.get(values, "service", {})
	trim_space(object.get(service, "type", "")) == ""
	msg := "Traefik HelmRelease must set values.service.type"
}

deny contains msg if {
	doc := traefik_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	service := object.get(values, "service", {})
	object.get(service, "type", "") == "LoadBalancer"
	spec := object.get(service, "spec", {})
	external_traffic_policy := object.get(spec, "externalTrafficPolicy", "")
	not is_string(external_traffic_policy) or trim_space(external_traffic_policy) == ""
	msg := "Traefik HelmRelease LoadBalancer service must set service.spec.externalTrafficPolicy"
}

deny contains msg if {
	doc := traefik_helmreleases[_]
	values := object.get(doc.spec, "values", {})
	dashboard := object.get(values, "dashboard", {})
	object.get(dashboard, "enabled", false)
	route := object.get(object.get(values, "ingressRoute", {}), "dashboard", {})
	match := object.get(route, "matchRule", "")
	not contains(match, "Host(`")
	msg := "Traefik dashboard requires ingressRoute.dashboard.matchRule with Host(`...`)"
}

warn contains msg if {
	doc := helmrepositories[_]
	url := object.get(object.get(doc, "spec", {}), "url", "")
	url != ""
	startswith(url, "http://")
	metadata := object.get(doc, "metadata", {})
	msg := sprintf("HelmRepository %s should use an https:// URL", [object.get(metadata, "name", "<unknown>")])
}
