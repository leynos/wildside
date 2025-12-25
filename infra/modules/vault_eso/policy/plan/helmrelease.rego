package vault_eso.policy.plan

import rego.v1

helm_release(rc) = {"name": name, "values": values_map} if {
	rc.type == "helm_release"
	after := rc.change.after
	after != null
	name := object.get(after, "name", "")
	values_raw := object.get(after, "values", [])
	values_map := merge_values(values_raw)
}

merge_values(raw) := result if {
	is_array(raw)
	decoded := [yaml.unmarshal(v) | v := raw[_]]
	result := object.union_n(decoded)
}

merge_values(raw) := {} if {
	not is_array(raw)
}

deny contains msg if {
	rc := input.resource_changes[_]
	hr := helm_release(rc)
	values := hr.values
	webhook := object.get(values, "webhook", {})
	replicas := object.get(webhook, "replicaCount", 0)
	replicas <= 0
	msg := sprintf("Helm release %s must set webhook.replicaCount > 0", [hr.name])
}

warn contains msg if {
	rc := input.resource_changes[_]
	hr := helm_release(rc)
	values := hr.values
	install_crds := object.get(values, "installCRDs", true)
	not install_crds
	msg := sprintf("Helm release %s has installCRDs=false - ensure CRDs are managed separately", [hr.name])
}
