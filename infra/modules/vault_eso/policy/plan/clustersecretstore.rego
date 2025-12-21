package vault_eso.policy.plan

import rego.v1

clustersecretstore(rc) = {"manifest": manifest, "spec": spec} if {
	rc.type == "kubernetes_manifest"
	after := rc.change.after
	after != null
	manifest := object.get(after, "manifest", null)
	manifest != null
	manifest.kind == "ClusterSecretStore"
	spec := object.get(manifest, "spec", {})
}

vault_provider(spec) := object.get(object.get(spec, "provider", {}), "vault", {})

vault_server(spec) := object.get(vault_provider(spec), "server", "")

vault_path(spec) := object.get(vault_provider(spec), "path", "")

vault_ca_bundle(spec) := object.get(vault_provider(spec), "caBundle", "")

vault_auth(spec) := object.get(vault_provider(spec), "auth", {})

vault_approle_auth(spec) := object.get(vault_auth(spec), "appRole", {})

approle_role_ref(spec) := object.get(vault_approle_auth(spec), "roleRef", {})

approle_secret_ref(spec) := object.get(vault_approle_auth(spec), "secretRef", {})

deny contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	vault := vault_provider(spec)
	vault != {}
	server := vault_server(spec)
	not startswith(server, "https://")
	msg := sprintf("ClusterSecretStore %s must use HTTPS Vault server URL", [css.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	vault := vault_provider(spec)
	vault != {}
	path := vault_path(spec)
	trim_space(path) == ""
	msg := sprintf("ClusterSecretStore %s must set provider.vault.path", [css.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	vault := vault_provider(spec)
	vault != {}
	ca_bundle := vault_ca_bundle(spec)
	trim_space(ca_bundle) == ""
	msg := sprintf("ClusterSecretStore %s must set provider.vault.caBundle", [css.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	approle := vault_approle_auth(spec)
	approle != {}
	role_ref := approle_role_ref(spec)
	trim_space(object.get(role_ref, "name", "")) == ""
	msg := sprintf("ClusterSecretStore %s must set appRole.roleRef.name", [css.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	approle := vault_approle_auth(spec)
	approle != {}
	role_ref := approle_role_ref(spec)
	trim_space(object.get(role_ref, "namespace", "")) == ""
	msg := sprintf("ClusterSecretStore %s must set appRole.roleRef.namespace", [css.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	approle := vault_approle_auth(spec)
	approle != {}
	secret_ref := approle_secret_ref(spec)
	trim_space(object.get(secret_ref, "name", "")) == ""
	msg := sprintf("ClusterSecretStore %s must set appRole.secretRef.name", [css.manifest.metadata.name])
}

deny contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	approle := vault_approle_auth(spec)
	approle != {}
	secret_ref := approle_secret_ref(spec)
	trim_space(object.get(secret_ref, "namespace", "")) == ""
	msg := sprintf("ClusterSecretStore %s must set appRole.secretRef.namespace", [css.manifest.metadata.name])
}

retry_settings(spec) := object.get(spec, "retrySettings", {})

warn contains msg if {
	rc := input.resource_changes[_]
	css := clustersecretstore(rc)
	spec := css.spec
	retry := retry_settings(spec)
	retry == {}
	msg := sprintf("ClusterSecretStore %s should configure retrySettings", [css.manifest.metadata.name])
}
