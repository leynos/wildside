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

doc_spec(doc) := object.get(doc, "spec", {})

spec_provider(doc) := object.get(doc_spec(doc), "provider", {})

vault_provider(doc) := object.get(spec_provider(doc), "vault", {})

vault_server(doc) := object.get(vault_provider(doc), "server", "")

vault_path(doc) := object.get(vault_provider(doc), "path", "")

vault_ca_bundle(doc) := object.get(vault_provider(doc), "caBundle", "")

vault_auth(doc) := object.get(vault_provider(doc), "auth", {})

vault_approle_auth(doc) := object.get(vault_auth(doc), "appRole", {})

uses_approle_auth(doc) if {
	vault_approle_auth(doc) != {}
}

approle_ref(doc, ref_type) := object.get(vault_approle_auth(doc), ref_type, {})

missing_approle_field(doc, ref_type, field) if {
	uses_approle_auth(doc)
	ref := approle_ref(doc, ref_type)
	trim_space(object.get(ref, field, "")) == ""
}

approle_field_checks := [
	["roleRef", "name"],
	["roleRef", "namespace"],
	["roleRef", "key"],
	["secretRef", "name"],
	["secretRef", "namespace"],
	["secretRef", "key"],
]

deny contains msg if {
	doc := clustersecretstores[_]
	server := vault_server(doc)
	trim_space(server) == ""
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set provider.vault.server", [name])
}

deny contains msg if {
	doc := clustersecretstores[_]
	server := vault_server(doc)
	trim_space(server) != ""
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
	check := approle_field_checks[_]
	missing_approle_field(doc, check[0], check[1])
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("ClusterSecretStore %s must set appRole.%s.%s", [name, check[0], check[1]])
}
