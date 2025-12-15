package traefik.policy.plan

import rego.v1

# Helper to extract ACME config from ClusterIssuer
acme_config(spec) = object.get(spec, "acme", {})

acme_server(spec) = object.get(acme_config(spec), "server", "")

acme_email(spec) = object.get(acme_config(spec), "email", "")

acme_solvers(spec) = object.get(acme_config(spec), "solvers", [])

# Helper to extract a ClusterIssuer resource change from an OpenTofu plan.
#
# Resource deletions may have `rc.change.after == null`, so this helper includes
# the guard to avoid runtime errors when policies evaluate deletes.
clusterissuer(rc) = {"manifest": manifest, "spec": spec} if {
	rc.type == "kubernetes_manifest"
	after := rc.change.after
	after != null
	manifest := object.get(after, "manifest", null)
	manifest != null
	manifest.kind == "ClusterIssuer"
	spec := object.get(manifest, "spec", {})
}

# Check if a solver has DNS01 configured
has_dns01_solver(solvers) if {
	some solver in solvers
	object.get(solver, "dns01", null) != null
}

# Ensure ClusterIssuer uses HTTPS ACME server.
deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	manifest := ci.manifest
	spec := ci.spec
	server := acme_server(spec)
	not startswith(server, "https://")
	msg := sprintf("ClusterIssuer %s must use HTTPS ACME server URL", [manifest.metadata.name])
}

# Ensure ClusterIssuer has a valid email address.
deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	manifest := ci.manifest
	spec := ci.spec
	email := acme_email(spec)
	email == ""
	msg := sprintf("ClusterIssuer %s must have a valid ACME email address", [manifest.metadata.name])
}

# Ensure ClusterIssuer has at least one ACME solver.
deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	manifest := ci.manifest
	spec := ci.spec
	solvers := acme_solvers(spec)
	count(solvers) == 0
	msg := sprintf("ClusterIssuer %s must have at least one ACME solver configured", [manifest.metadata.name])
}

# Ensure ClusterIssuer uses DNS01 solver.
deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	manifest := ci.manifest
	spec := ci.spec
	solvers := acme_solvers(spec)
	count(solvers) > 0
	not has_dns01_solver(solvers)
	msg := sprintf("ClusterIssuer %s must use DNS01 solver", [manifest.metadata.name])
}

# Known Let's Encrypt staging server URLs
acme_staging_servers := {
	"https://acme-staging-v02.api.letsencrypt.org/directory",
	"https://acme-staging.api.letsencrypt.org/directory",
}

# Warn when using ACME staging server (certificates will not be trusted).
warn contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	manifest := ci.manifest
	spec := ci.spec
	server := acme_server(spec)
	server in acme_staging_servers
	msg := sprintf(
		"ClusterIssuer %s uses ACME staging server - certificates will not be trusted",
		[manifest.metadata.name],
	)
}

# Ensure ClusterIssuer has privateKeySecretRef.name set.
deny contains msg if {
	rc := input.resource_changes[_]
	ci := clusterissuer(rc)
	manifest := ci.manifest
	spec := ci.spec
	private_key_ref := object.get(acme_config(spec), "privateKeySecretRef", {})
	object.get(private_key_ref, "name", "") == ""
	msg := sprintf("ClusterIssuer %s must have privateKeySecretRef.name set", [manifest.metadata.name])
}
