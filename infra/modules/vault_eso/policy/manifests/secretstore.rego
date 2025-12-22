package vault_eso.policy.manifests

import rego.v1

clustersecretstores := [doc |
	doc := documents[_]
	doc.kind == "ClusterSecretStore"
	is_external_secrets(doc)
]

secrets := [doc |
	doc := documents[_]
	doc.kind == "Secret"
	is_external_secrets(doc)
]

vault_provider(doc) := object.get(object.get(object.get(doc, "spec", {}), "provider", {}), "vault", {})

vault_server(doc) := object.get(vault_provider(doc), "server", "")

vault_path(doc) := object.get(vault_provider(doc), "path", "")

vault_ca_bundle(doc) := object.get(vault_provider(doc), "caBundle", "")

vault_auth(doc) := object.get(vault_provider(doc), "auth", {})

vault_approle_auth(doc) := object.get(vault_auth(doc), "appRole", {})

approle_role_ref(doc) := object.get(vault_approle_auth(doc), "roleRef", {})

approle_secret_ref(doc) := object.get(vault_approle_auth(doc), "secretRef", {})

uses_approle_auth(doc) if {
	vault_approle_auth(doc) != {}
}

deny contains msg if {
	doc := clustersecretstores[_]
	server := vault_server(doc)
	not startswith(server, "https://")
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must use HTTPS Vault server URL", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	path := vault_path(doc)
	trim_space(path) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set provider.vault.path", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	ca_bundle := vault_ca_bundle(doc)
	trim_space(ca_bundle) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set provider.vault.caBundle", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	uses_approle_auth(doc)
	role_ref := approle_role_ref(doc)
	trim_space(object.get(role_ref, "name", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set appRole.roleRef.name", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	uses_approle_auth(doc)
	role_ref := approle_role_ref(doc)
	trim_space(object.get(role_ref, "namespace", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set appRole.roleRef.namespace", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	uses_approle_auth(doc)
	role_ref := approle_role_ref(doc)
	trim_space(object.get(role_ref, "key", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set appRole.roleRef.key", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	uses_approle_auth(doc)
	secret_ref := approle_secret_ref(doc)
	trim_space(object.get(secret_ref, "name", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set appRole.secretRef.name", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	uses_approle_auth(doc)
	secret_ref := approle_secret_ref(doc)
	trim_space(object.get(secret_ref, "namespace", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set appRole.secretRef.namespace", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	uses_approle_auth(doc)
	secret_ref := approle_secret_ref(doc)
	trim_space(object.get(secret_ref, "key", "")) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set appRole.secretRef.key", [name])
}
