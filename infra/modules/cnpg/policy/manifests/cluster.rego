package cnpg.policy.manifests

import rego.v1

clusters := [doc |
	doc := documents[_]
	doc.kind == "Cluster"
	doc.apiVersion == "postgresql.cnpg.io/v1"
]

pdbs := [doc |
	doc := documents[_]
	doc.kind == "PodDisruptionBudget"
	is_cnpg(doc)
]

cluster_spec(doc) := object.get(doc, "spec", {})

cluster_instances(doc) := object.get(cluster_spec(doc), "instances", 0)

cluster_storage(doc) := object.get(cluster_spec(doc), "storage", {})

cluster_bootstrap(doc) := object.get(cluster_spec(doc), "bootstrap", {})

cluster_backup(doc) := object.get(cluster_spec(doc), "backup", {})

cluster_image(doc) := object.get(cluster_spec(doc), "imageName", "")

has_pdb_for_cluster(cluster_name) if {
	some pdb in pdbs
	selector := object.get(object.get(pdb, "spec", {}), "selector", {})
	match_labels := object.get(selector, "matchLabels", {})
	object.get(match_labels, "cnpg.io/cluster", "") == cluster_name
}

# Cluster must specify instances > 0
deny contains msg if {
	doc := clusters[_]
	cluster_instances(doc) <= 0
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s must set spec.instances > 0", [name])
}

# Cluster must specify storage class
deny contains msg if {
	doc := clusters[_]
	storage := cluster_storage(doc)
	trim_space(object.get(storage, "storageClass", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s must set spec.storage.storageClass", [name])
}

# Cluster must specify storage size
deny contains msg if {
	doc := clusters[_]
	storage := cluster_storage(doc)
	trim_space(object.get(storage, "size", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s must set spec.storage.size", [name])
}

# Cluster must have bootstrap configuration
deny contains msg if {
	doc := clusters[_]
	bootstrap := cluster_bootstrap(doc)
	count(bootstrap) == 0
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s must have spec.bootstrap configuration", [name])
}

# Backup with barmanObjectStore requires S3 credentials secret
deny contains msg if {
	doc := clusters[_]
	backup := cluster_backup(doc)
	barman := object.get(backup, "barmanObjectStore", {})
	count(barman) > 0
	s3_creds := object.get(barman, "s3Credentials", {})
	access_key := object.get(s3_creds, "accessKeyId", {})
	trim_space(object.get(access_key, "name", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s backup must reference S3 credentials secret", [name])
}

# Backup destination path required when backup configured
deny contains msg if {
	doc := clusters[_]
	backup := cluster_backup(doc)
	barman := object.get(backup, "barmanObjectStore", {})
	count(barman) > 0
	trim_space(object.get(barman, "destinationPath", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s must set backup.barmanObjectStore.destinationPath", [name])
}

# HA clusters (instances > 1) require PodDisruptionBudget
deny contains msg if {
	doc := clusters[_]
	cluster_instances(doc) > 1
	name := object.get(metadata(doc), "name", "")
	name != ""
	not has_pdb_for_cluster(name)
	msg := sprintf("CNPG Cluster %s with multiple instances requires a PodDisruptionBudget", [name])
}

# Warn if not using PostGIS image when postgis may be expected
warn contains msg if {
	doc := clusters[_]
	image := cluster_image(doc)
	image != ""
	not contains(image, "postgis")
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s is not using a PostGIS image", [name])
}

# Warn if using unsupervised update strategy in production
warn contains msg if {
	doc := clusters[_]
	strategy := object.get(cluster_spec(doc), "primaryUpdateStrategy", "")
	strategy == "unsupervised"
	instances := cluster_instances(doc)
	instances >= 3
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("CNPG Cluster %s uses unsupervised primaryUpdateStrategy with %d instances", [name, instances])
}
