package valkey.policy.manifests

import rego.v1

valkey_clusters := [doc |
	doc := documents[_]
	doc.kind == "Valkey"
	doc.apiVersion == "hyperspike.io/v1"
]

pdbs := [doc |
	doc := documents[_]
	doc.kind == "PodDisruptionBudget"
	is_valkey(doc)
]

external_secrets := [doc |
	doc := documents[_]
	doc.kind == "ExternalSecret"
	is_valkey(doc)
]

cluster_spec(doc) := object.get(doc, "spec", {})

cluster_nodes(doc) := object.get(cluster_spec(doc), "nodes", 0)

cluster_replicas(doc) := object.get(cluster_spec(doc), "replicas", 0)

cluster_storage(doc) := object.get(cluster_spec(doc), "storage", {})

cluster_tls(doc) := object.get(cluster_spec(doc), "tls", false)

cluster_anonymous(doc) := object.get(cluster_spec(doc), "anonymousAuth", false)

cluster_service_password(doc) := object.get(cluster_spec(doc), "servicePassword", {})

has_pdb_for_cluster(cluster_name) if {
	some pdb in pdbs
	selector := object.get(object.get(pdb, "spec", {}), "selector", {})
	match_labels := object.get(selector, "matchLabels", {})
	object.get(match_labels, "valkey.hyperspike.io/cluster", "") == cluster_name
}

# Cluster must specify nodes >= 1
deny contains msg if {
	doc := valkey_clusters[_]
	cluster_nodes(doc) < 1
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Valkey cluster %s must set spec.nodes >= 1", [name])
}

# Cluster must specify storage class when storage is configured
deny contains msg if {
	doc := valkey_clusters[_]
	storage := cluster_storage(doc)
	count(storage) > 0
	trim_space(object.get(storage, "storageClassName", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Valkey cluster %s must set spec.storage.storageClassName", [name])
}

# Cluster must specify storage size when storage is configured
deny contains msg if {
	doc := valkey_clusters[_]
	storage := cluster_storage(doc)
	count(storage) > 0
	resources := object.get(storage, "resources", {})
	requests := object.get(resources, "requests", {})
	trim_space(object.get(requests, "storage", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Valkey cluster %s must set spec.storage.resources.requests.storage", [name])
}

# TLS enabled requires cert issuer
deny contains msg if {
	doc := valkey_clusters[_]
	cluster_tls(doc) == true
	trim_space(object.get(cluster_spec(doc), "certIssuer", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Valkey cluster %s has TLS enabled but no certIssuer specified", [name])
}

# Non-anonymous auth requires password reference
deny contains msg if {
	doc := valkey_clusters[_]
	not cluster_anonymous(doc)
	password := cluster_service_password(doc)
	trim_space(object.get(password, "name", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Valkey cluster %s requires servicePassword.name when anonymousAuth is false", [name])
}

# HA clusters (replicas > 0) require PodDisruptionBudget
deny contains msg if {
	doc := valkey_clusters[_]
	cluster_replicas(doc) > 0
	name := object.get(metadata(doc), "name", "")
	name != ""
	not has_pdb_for_cluster(name)
	msg := sprintf("Valkey cluster %s with replicas requires a PodDisruptionBudget", [name])
}

# Warn if running with anonymous auth in production-like config
warn contains msg if {
	doc := valkey_clusters[_]
	cluster_anonymous(doc) == true
	cluster_replicas(doc) > 0
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Valkey cluster %s uses anonymousAuth with replicas - consider enabling auth for production", [name])
}

# Warn if TLS not enabled for HA clusters
warn contains msg if {
	doc := valkey_clusters[_]
	cluster_replicas(doc) > 0
	not cluster_tls(doc)
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Valkey cluster %s has replicas but TLS is not enabled - consider enabling for production", [name])
}
