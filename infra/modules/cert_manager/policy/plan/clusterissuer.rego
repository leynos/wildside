package cert_manager.policy.plan

import rego.v1

acme_config(spec) := object.get(spec, "acme", {})

acme_server(spec) := object.get(acme_config(spec), "server", "")

acme_email(spec) := object.get(acme_config(spec), "email", "")

acme_solvers(spec) := object.get(acme_config(spec), "solvers", [])

acme_webhook_solvers(spec) := [webhook |
	solver := acme_solvers(spec)[_]
	dns01 := object.get(solver, "dns01", {})
	webhook := object.get(dns01, "webhook", null)
	webhook != null
]


clusterissuer(rc) = {"manifest": manifest, "spec": spec} if {
	rc.type == "kubernetes_manifest"
	after := rc.change.after
	after != null
	manifest := object.get(after, "manifest", null)
	manifest != null
	manifest.kind == "ClusterIssuer"
	spec := object.get(manifest, "spec", {})
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	server := acme_server(spec)
	not startswith(server, "https://")
	msg := sprintf("ClusterIssuer %s must use HTTPS ACME server URL", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	email := acme_email(spec)
	trim_space(email) == ""
	msg := sprintf("ClusterIssuer %s must have a valid ACME email address", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	solvers := acme_solvers(spec)
	count(solvers) == 0
	msg := sprintf("ClusterIssuer %s must have at least one ACME solver configured", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	trim_space(object.get(webhook, "groupName", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook groupName", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	trim_space(object.get(webhook, "solverName", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook solverName", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_key_ref := object.get(config, "apiKeySecretRef", {})
	trim_space(object.get(api_key_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiKeySecretRef.name", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_key_ref := object.get(config, "apiKeySecretRef", {})
	trim_space(object.get(api_key_ref, "key", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiKeySecretRef.key", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_user_ref := object.get(config, "apiUserSecretRef", {})
	trim_space(object.get(api_user_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiUserSecretRef.name", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	webhook := acme_webhook_solvers(spec)[_]
	config := object.get(webhook, "config", {})
	api_user_ref := object.get(config, "apiUserSecretRef", {})
	trim_space(object.get(api_user_ref, "key", "")) == ""
	msg := sprintf("ClusterIssuer %s must set webhook apiUserSecretRef.key", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	private_key_ref := object.get(acme, "privateKeySecretRef", {})
	trim_space(object.get(private_key_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set privateKeySecretRef.name", [ci.manifest.metadata.name])
}

vault_config(spec) := object.get(spec, "vault", {})

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	vault := vault_config(spec)
	vault != {}
	server := object.get(vault, "server", "")
	not startswith(server, "https://")
	msg := sprintf("ClusterIssuer %s must set vault.server to https://", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	vault := vault_config(spec)
	vault != {}
	path := object.get(vault, "path", "")
	trim_space(path) == ""
	msg := sprintf("ClusterIssuer %s must set vault.path", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	vault := vault_config(spec)
	vault != {}
	auth := object.get(vault, "auth", {})
	token_ref := object.get(auth, "tokenSecretRef", {})
	trim_space(object.get(token_ref, "name", "")) == ""
	msg := sprintf("ClusterIssuer %s must set vault.auth.tokenSecretRef.name", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	vault := vault_config(spec)
	vault != {}
	auth := object.get(vault, "auth", {})
	token_ref := object.get(auth, "tokenSecretRef", {})
	trim_space(object.get(token_ref, "key", "")) == ""
	msg := sprintf("ClusterIssuer %s must set vault.auth.tokenSecretRef.key", [ci.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	vault := vault_config(spec)
	vault != {}
	ca_bundle := object.get(vault, "caBundle", "")
	trim_space(ca_bundle) == ""
	msg := sprintf("ClusterIssuer %s must set vault.caBundle", [ci.manifest.metadata.name])
}

acme_staging_servers := {
	"https://acme-staging-v02.api.letsencrypt.org/directory",
	"https://acme-staging.api.letsencrypt.org/directory",
}

warn contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	spec := ci.spec
	acme := acme_config(spec)
	acme != {}
	server := acme_server(spec)
	server in acme_staging_servers
	msg := sprintf(
		"ClusterIssuer %s uses ACME staging server - certificates will not be trusted",
		[ci.manifest.metadata.name],
	)
}
