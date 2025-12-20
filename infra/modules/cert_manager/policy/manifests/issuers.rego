package cert_manager.policy.manifests

import rego.v1

documents := input if is_array(input)
documents := [input] if not is_array(input)

metadata(doc) := object.get(doc, "metadata", {})

labels(doc) := object.get(metadata(doc), "labels", {})

is_cert_manager(doc) if {
	object.get(labels(doc), "app.kubernetes.io/part-of", "") == "cert-manager"
}

issuers := [doc |
	doc := documents[_]
	doc.kind == "ClusterIssuer"
	is_cert_manager(doc)
]

secrets := [doc |
	doc := documents[_]
	doc.kind == "Secret"
	is_cert_manager(doc)
]

acme_config(spec) := object.get(spec, "acme", {})

acme_solvers(spec) := object.get(acme_config(spec), "solvers", [])

acme_webhook_solvers(spec) := [webhook |
	solver := acme_solvers(spec)[_]
	dns01 := object.get(solver, "dns01", {})
	webhook := object.get(dns01, "webhook", null)
	webhook != null
]


deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	server := object.get(acme, "server", "")
	not startswith(server, "https://")
	msg := sprintf("ClusterIssuer %s must use HTTPS ACME server URL", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	email := object.get(acme, "email", "")
	trim_space(email) == ""
	msg := sprintf("ClusterIssuer %s must set acme.email", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	private_key_ref := object.get(acme, "privateKeySecretRef", {})
	trim_space(object.get(private_key_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set acme.privateKeySecretRef.name", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	solvers := acme_solvers(spec)
	count(solvers) == 0
	msg := sprintf("ClusterIssuer %s must define at least one ACME solver", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	trim_space(object.get(webhook, "groupName", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook groupName", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	trim_space(object.get(webhook, "solverName", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook solverName", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_key_ref := object.get(config, "apiKeySecretRef", {})
	trim_space(object.get(api_key_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiKeySecretRef.name", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_key_ref := object.get(config, "apiKeySecretRef", {})
	trim_space(object.get(api_key_ref, "key", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiKeySecretRef.key", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_user_ref := object.get(config, "apiUserSecretRef", {})
	trim_space(object.get(api_user_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiUserSecretRef.name", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_user_ref := object.get(config, "apiUserSecretRef", {})
	trim_space(object.get(api_user_ref, "key", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiUserSecretRef.key", [object.get(metadata(doc), "name", "<unknown>")])
}

vault_config(spec) := object.get(spec, "vault", {})

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	vault := vault_config(spec)
	vault != {}
	server := object.get(vault, "server", "")
	not startswith(server, "https://")
	msg := sprintf("ClusterIssuer %s must set vault.server to https://", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	vault := vault_config(spec)
	vault != {}
	path := object.get(vault, "path", "")
	trim_space(path) == ""
	msg := sprintf("ClusterIssuer %s must set vault.path", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	vault := vault_config(spec)
	vault != {}
	auth := object.get(vault, "auth", {})
	token_ref := object.get(auth, "tokenSecretRef", {})
	trim_space(object.get(token_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set vault.auth.tokenSecretRef.name", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	vault := vault_config(spec)
	vault != {}
	auth := object.get(vault, "auth", {})
	token_ref := object.get(auth, "tokenSecretRef", {})
	trim_space(object.get(token_ref, "key", "")) == ""
	msg := sprintf("ClusterIssuer %s must set vault.auth.tokenSecretRef.key", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	vault := vault_config(spec)
	vault != {}
	ca_bundle := object.get(vault, "caBundle", "")
	trim_space(ca_bundle) == ""
	msg := sprintf("ClusterIssuer %s must set vault.caBundle", [object.get(metadata(doc), "name", "<unknown>")])
}

deny contains msg if {
	doc := secrets[_]
	secret_data := object.get(doc, "data", {})
	count(secret_data) == 0
	name := object.get(metadata(doc), "name", "<unknown>")
	msg := sprintf("Secret %s must include data entries", [name])
}

acme_staging_servers := {
	"https://acme-staging-v02.api.letsencrypt.org/directory",
	"https://acme-staging.api.letsencrypt.org/directory",
}

warn contains msg if {
	doc := issuers[_]
	spec := object.get(doc, "spec", {})
	acme := acme_config(spec)
	acme != {}
	server := object.get(acme, "server", "")
	server in acme_staging_servers
	msg := sprintf(
		"ClusterIssuer %s uses ACME staging server - certificates will not be trusted",
		[object.get(metadata(doc), "name", "<unknown>")],
	)
}
