package traefik.policy

import future.keywords.contains
import future.keywords.if

# Helper to extract ACME config from ClusterIssuer
acme_config(spec) = object.get(spec, "acme", {})

acme_server(spec) = object.get(acme_config(spec), "server", "")

acme_email(spec) = object.get(acme_config(spec), "email", "")

acme_solvers(spec) = object.get(acme_config(spec), "solvers", [])

# Check if a solver has DNS01 configured
has_dns01_solver(solvers) if {
	some solver in solvers
	object.get(solver, "dns01", null) != null
}

# Ensure ClusterIssuer uses HTTPS ACME server.
deny contains msg if {
	rc := input.resource_changes[_]
	rc.type == "kubernetes_manifest"
	manifest := rc.change.after.manifest
	manifest.kind == "ClusterIssuer"
	spec := manifest.spec
	server := acme_server(spec)
	not startswith(server, "https://")
	msg := sprintf("ClusterIssuer %s must use HTTPS ACME server URL", [manifest.metadata.name])
}

# Ensure ClusterIssuer has a valid email address.
deny contains msg if {
	rc := input.resource_changes[_]
	rc.type == "kubernetes_manifest"
	manifest := rc.change.after.manifest
	manifest.kind == "ClusterIssuer"
	spec := manifest.spec
	email := acme_email(spec)
	email == ""
	msg := sprintf("ClusterIssuer %s must have a valid ACME email address", [manifest.metadata.name])
}

# Ensure ClusterIssuer has at least one ACME solver.
deny contains msg if {
	rc := input.resource_changes[_]
	rc.type == "kubernetes_manifest"
	manifest := rc.change.after.manifest
	manifest.kind == "ClusterIssuer"
	spec := manifest.spec
	solvers := acme_solvers(spec)
	count(solvers) == 0
	msg := sprintf("ClusterIssuer %s must have at least one ACME solver configured", [manifest.metadata.name])
}

# Ensure ClusterIssuer uses DNS01 solver (required for wildcard certificates).
deny contains msg if {
	rc := input.resource_changes[_]
	rc.type == "kubernetes_manifest"
	manifest := rc.change.after.manifest
	manifest.kind == "ClusterIssuer"
	spec := manifest.spec
	solvers := acme_solvers(spec)
	count(solvers) > 0
	not has_dns01_solver(solvers)
	msg := sprintf("ClusterIssuer %s must use DNS01 solver for wildcard certificate support", [manifest.metadata.name])
}

# Warn when using ACME staging server (certificates will not be trusted).
warn contains msg if {
	rc := input.resource_changes[_]
	rc.type == "kubernetes_manifest"
	manifest := rc.change.after.manifest
	manifest.kind == "ClusterIssuer"
	spec := manifest.spec
	server := acme_server(spec)
	contains(server, "staging")
	msg := sprintf(
		"ClusterIssuer %s uses ACME staging server - certificates will not be trusted",
		[manifest.metadata.name],
	)
}

# Ensure Traefik Helm release has privateKeySecretRef set in ClusterIssuer.
deny contains msg if {
	rc := input.resource_changes[_]
	rc.type == "kubernetes_manifest"
	manifest := rc.change.after.manifest
	manifest.kind == "ClusterIssuer"
	spec := manifest.spec
	acme := acme_config(spec)
	private_key_ref := object.get(acme, "privateKeySecretRef", {})
	object.get(private_key_ref, "name", "") == ""
	msg := sprintf("ClusterIssuer %s must have privateKeySecretRef.name set", [manifest.metadata.name])
}
