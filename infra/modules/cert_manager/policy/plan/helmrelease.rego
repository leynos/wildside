package cert_manager.policy.plan

import rego.v1

# Merge a list of YAML strings into a single object, with later entries
# overriding earlier ones (matching Helm's merge order).
merge_helm_values(values_list) = merged if {
	parsed := [yaml.unmarshal(v) | some v in values_list]
	merged := object.union_n(parsed)
}

merge_helm_values(values_list) = {} if {
	count(values_list) == 0
}

helm_release(rc) = {
	"name": name,
	"values": values,
	"repository": repository,
	"chart": chart,
	"version": version,
} if {
	rc.type == "helm_release"
	after := rc.change.after
	after != null
	name := object.get(after, "name", "")
	values_list := object.get(after, "values", [])
	values := merge_helm_values(values_list)
	repository := object.get(after, "repository", "")
	chart := object.get(after, "chart", "")
	version := object.get(after, "version", "")
}

is_main_release(release) if {
	is_jetstack_repository(release)
	values := release.values
	object.get(values, "webhook", null) != null
	object.get(values, "cainjector", null) != null
}

is_main_release(release) if {
	is_jetstack_repository(release)
	lower(release.chart) == "cert-manager"
}

is_jetstack_repository(release) if {
	repository := object.get(release, "repository", "")
	contains(lower(repository), "jetstack")
}

is_webhook_release(release) if {
	contains(lower(release.chart), "webhook")
}

is_webhook_release(release) if {
	group_name := object.get(release.values, "groupName", "")
	group_name != ""
}

webhook_replica_count(values) := object.get(object.get(values, "webhook", {}), "replicaCount", 0)

cainjector_replica_count(values) := object.get(object.get(values, "cainjector", {}), "replicaCount", 0)

pdbs := [manifest |
	rc := input.resource_changes[_]
	rc.type == "kubernetes_manifest"
	after := rc.change.after
	after != null
	manifest := object.get(after, "manifest", null)
	manifest != null
	manifest.kind == "PodDisruptionBudget"
]

has_pdb(component, instance) if {
	some pdb in pdbs
	selector := object.get(object.get(pdb, "spec", {}), "selector", {})
	match_labels := object.get(selector, "matchLabels", {})
	object.get(match_labels, "app.kubernetes.io/name", "") == component
	object.get(match_labels, "app.kubernetes.io/instance", "") == instance
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	trim_space(release.version) == ""
	msg := sprintf("Helm release %s must pin chart version", [release.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	is_main_release(release)
	object.get(release.values, "replicaCount", null) == null
	msg := sprintf("Cert-manager Helm release %s must set values.replicaCount", [release.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	is_main_release(release)
	webhook_replica_count(release.values) <= 0
	msg := sprintf("Cert-manager Helm release %s must set values.webhook.replicaCount", [release.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	is_main_release(release)
	cainjector_replica_count(release.values) <= 0
	msg := sprintf("Cert-manager Helm release %s must set values.cainjector.replicaCount", [release.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	is_main_release(release)
	webhook_replica_count(release.values) > 1
	not has_pdb("webhook", release.name)
	msg := "Cert-manager webhook replicas > 1 require a PodDisruptionBudget"
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	is_main_release(release)
	cainjector_replica_count(release.values) > 1
	not has_pdb("cainjector", release.name)
	msg := "Cert-manager cainjector replicas > 1 require a PodDisruptionBudget"
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	is_webhook_release(release)
	group_name := object.get(release.values, "groupName", "")
	trim_space(group_name) == ""
	msg := sprintf("Namecheap webhook Helm release %s must set values.groupName", [release.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	is_webhook_release(release)
	replicas := object.get(release.values, "replicaCount", 0)
	replicas <= 0
	msg := sprintf("Namecheap webhook Helm release %s must set values.replicaCount", [release.name])
}

warn contains msg if {
	rc := input.resource_changes[_]
	release := helm_release(rc)
	startswith(release.repository, "http://")
	msg := sprintf("Helm release %s should use an https:// or oci:// repository URL", [release.name])
}
