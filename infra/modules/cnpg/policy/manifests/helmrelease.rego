package cnpg.policy.manifests

import rego.v1

documents := input if is_array(input)
documents := [input] if not is_array(input)

metadata(doc) := object.get(doc, "metadata", {})

labels(doc) := object.get(metadata(doc), "labels", {})

is_cnpg(doc) if {
	object.get(labels(doc), "app.kubernetes.io/part-of", "") == "cloudnative-pg"
}

helmreleases := [doc |
	doc := documents[_]
	doc.kind == "HelmRelease"
	is_cnpg(doc)
]

helmrepositories := [doc |
	doc := documents[_]
	doc.kind == "HelmRepository"
	is_cnpg(doc)
]

chart_spec(doc) := object.get(object.get(object.get(doc, "spec", {}), "chart", {}), "spec", {})

chart_name(doc) := object.get(chart_spec(doc), "chart", "")

chart_version(doc) := object.get(chart_spec(doc), "version", "")

source_ref(doc) := object.get(chart_spec(doc), "sourceRef", {})

# HelmRelease must pin chart version
deny contains msg if {
	doc := helmreleases[_]
	trim_space(chart_version(doc)) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CloudNativePG HelmRelease %s must pin chart.spec.version", [name])
}

# HelmRelease must reference a source
deny contains msg if {
	doc := helmreleases[_]
	trim_space(object.get(source_ref(doc), "name", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CloudNativePG HelmRelease %s must set chart.spec.sourceRef.name", [name])
}

# HelmRelease sourceRef must include namespace
deny contains msg if {
	doc := helmreleases[_]
	trim_space(object.get(source_ref(doc), "namespace", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CloudNativePG HelmRelease %s must set chart.spec.sourceRef.namespace", [name])
}

# HelmRepository with OCI URL must set type=oci
deny contains msg if {
	doc := helmrepositories[_]
	url := object.get(object.get(doc, "spec", {}), "url", "")
	startswith(url, "oci://")
	object.get(object.get(doc, "spec", {}), "type", "") != "oci"
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("HelmRepository %s must set spec.type=oci for OCI URLs", [name])
}

# Warn if HelmRepository uses insecure HTTP
warn contains msg if {
	doc := helmrepositories[_]
	url := object.get(object.get(doc, "spec", {}), "url", "")
	url != ""
	startswith(url, "http://")
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("HelmRepository %s should use an https:// or oci:// URL", [name])
}
