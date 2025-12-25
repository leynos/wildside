package vault_eso.policy.manifests

import rego.v1

documents := input if is_array(input)
documents := [input] if not is_array(input)

metadata(doc) := object.get(doc, "metadata", {})

labels(doc) := object.get(metadata(doc), "labels", {})

is_external_secrets(doc) if {
	object.get(labels(doc), "app.kubernetes.io/part-of", "") == "external-secrets"
}

helmreleases := [doc |
	doc := documents[_]
	doc.kind == "HelmRelease"
	is_external_secrets(doc)
]

helmrepositories := [doc |
	doc := documents[_]
	doc.kind == "HelmRepository"
	is_external_secrets(doc)
]

pdbs := [doc |
	doc := documents[_]
	doc.kind == "PodDisruptionBudget"
	is_external_secrets(doc)
]

chart_spec(doc) := object.get(object.get(object.get(doc, "spec", {}), "chart", {}), "spec", {})

chart_name(doc) := object.get(chart_spec(doc), "chart", "")

chart_version(doc) := object.get(chart_spec(doc), "version", "")

source_ref(doc) := object.get(chart_spec(doc), "sourceRef", {})

values(doc) := object.get(object.get(doc, "spec", {}), "values", {})

is_eso_release(doc) if {
	chart := lower(chart_name(doc))
	chart == "external-secrets"
}

is_eso_release(doc) if {
	values_map := values(doc)
	object.get(values_map, "webhook", null) != null
}

webhook_replica_count(doc) := object.get(object.get(values(doc), "webhook", {}), "replicaCount", 0)

has_pdb(component, instance) if {
	some pdb in pdbs
	selector := object.get(object.get(pdb, "spec", {}), "selector", {})
	match_labels := object.get(selector, "matchLabels", {})
	object.get(match_labels, "app.kubernetes.io/name", "") == component
	object.get(match_labels, "app.kubernetes.io/instance", "") == instance
}

deny contains msg if {
	doc := helmreleases[_]
	trim_space(chart_version(doc)) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("External Secrets HelmRelease %s must pin chart.spec.version", [name])
}

deny contains msg if {
	doc := helmreleases[_]
	trim_space(object.get(source_ref(doc), "name", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("External Secrets HelmRelease %s must set chart.spec.sourceRef.name", [name])
}

deny contains msg if {
	doc := helmreleases[_]
	trim_space(object.get(source_ref(doc), "namespace", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("External Secrets HelmRelease %s must set chart.spec.sourceRef.namespace", [name])
}

deny contains msg if {
	doc := helmreleases[_]
	spec := object.get(doc, "spec", {})
	install := object.get(spec, "install", {})
	trim_space(object.get(install, "crds", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("External Secrets HelmRelease %s must set install.crds", [name])
}

deny contains msg if {
	doc := helmreleases[_]
	is_eso_release(doc)
	webhook_replica_count(doc) <= 0
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("External Secrets HelmRelease %s must set values.webhook.replicaCount", [name])
}

deny contains msg if {
	doc := helmreleases[_]
	is_eso_release(doc)
	webhook_replica_count(doc) > 1
	instance := object.get(metadata(doc), "name", "")
	not has_pdb("external-secrets-webhook", instance)
	msg := "External Secrets webhook replicas > 1 require a PodDisruptionBudget"
}

deny contains msg if {
	doc := helmrepositories[_]
	url := object.get(object.get(doc, "spec", {}), "url", "")
	startswith(url, "oci://")
	object.get(object.get(doc, "spec", {}), "type", "") != "oci"
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("HelmRepository %s must set spec.type=oci for OCI URLs", [name])
}

warn contains msg if {
	doc := helmrepositories[_]
	url := object.get(object.get(doc, "spec", {}), "url", "")
	url != ""
	startswith(url, "http://")
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("HelmRepository %s should use an https:// or oci:// URL", [name])
}
