package tests

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

func TestValkeyModuleValidate(t *testing.T) {
	t.Parallel()
	_, opts := setup(t, testVars(t))
	terraform.InitAndValidate(t, opts)
}

func TestValkeyModuleRenderOutputs(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVars(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	require.NotEmpty(t, rendered, "expected rendered_manifests output to be non-empty")

	// Check operator HelmRelease
	helmRelease, ok := rendered["platform/redis/valkey-operator-helmrelease.yaml"]
	require.True(t, ok, "expected platform/redis/valkey-operator-helmrelease.yaml output key")
	require.True(
		t,
		strings.Contains(helmRelease, "kind: HelmRelease") ||
			strings.Contains(helmRelease, "\"kind\": \"HelmRelease\""),
		"expected HelmRelease manifest to contain kind HelmRelease",
	)
	require.True(
		t,
		strings.Contains(helmRelease, "valkey-operator") ||
			strings.Contains(helmRelease, "\"valkey-operator\""),
		"expected HelmRelease manifest to reference valkey-operator",
	)

	// Check HelmRepository
	_, ok = rendered["platform/sources/valkey-operator-repo.yaml"]
	require.True(t, ok, "expected platform/sources/valkey-operator-repo.yaml output key")

	// Check Valkey cluster
	cluster, ok := rendered["platform/redis/valkey-cluster.yaml"]
	require.True(t, ok, "expected cluster manifest output")
	require.True(
		t,
		strings.Contains(cluster, "kind: Valkey") ||
			strings.Contains(cluster, "\"kind\": \"Valkey\""),
		"expected Valkey manifest to contain kind Valkey",
	)
	require.True(
		t,
		strings.Contains(cluster, "hyperspike.io/v1") ||
			strings.Contains(cluster, "\"hyperspike.io/v1\""),
		"expected Valkey manifest to use hyperspike.io/v1 apiVersion",
	)

	// Check password secret (inline password provided)
	_, ok = rendered["platform/redis/password-secret.yaml"]
	require.True(t, ok, "expected password secret manifest with inline password")

	// Check namespaces
	_, ok = rendered["platform/redis/namespace-valkey-system.yaml"]
	require.True(t, ok, "expected operator namespace manifest")

	_, ok = rendered["platform/redis/namespace-valkey.yaml"]
	require.True(t, ok, "expected cluster namespace manifest")

	// Check kustomization
	_, ok = rendered["platform/redis/kustomization.yaml"]
	require.True(t, ok, "expected kustomization output")
}

func TestValkeyModuleRenderHA(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVarsHA(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	require.NotEmpty(t, rendered, "expected rendered_manifests output to be non-empty")

	// Check PDB for HA cluster (replicas > 0)
	_, ok := rendered["platform/redis/pdb-valkey.yaml"]
	require.True(t, ok, "expected PDB manifest for HA cluster")

	// Check cluster includes HA configuration
	cluster, ok := rendered["platform/redis/valkey-cluster.yaml"]
	require.True(t, ok, "expected cluster manifest output")
	require.True(
		t,
		strings.Contains(cluster, "replicas: 1") ||
			strings.Contains(cluster, "\"replicas\": 1"),
		"expected cluster to have replicas: 1 for HA",
	)
	require.True(
		t,
		strings.Contains(cluster, "nodes: 3") ||
			strings.Contains(cluster, "\"nodes\": 3"),
		"expected cluster to have nodes: 3",
	)
}

func TestValkeyModuleSyncPolicyContract(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVars(t))
	terraform.InitAndApply(t, opts)

	contract := terraform.OutputMap(t, opts, "sync_policy_contract")
	require.NotEmpty(t, contract, "expected sync_policy_contract output")

	// Verify required contract keys exist
	requiredKeys := []string{"cluster", "endpoints", "credentials", "tls", "persistence", "replication"}
	for _, key := range requiredKeys {
		_, ok := contract[key]
		require.True(t, ok, "sync_policy_contract missing required key: %s", key)
	}

	// Verify contract values match expected inputs
	require.Contains(t, contract["cluster"], "test-valkey", "cluster should contain expected name")
	require.Contains(t, contract["cluster"], "valkey", "cluster should contain expected namespace")

	require.Contains(t, contract["endpoints"], "test-valkey-primary", "endpoints should contain primary host")
	require.Contains(t, contract["endpoints"], "test-valkey-replicas", "endpoints should contain replica host")
	require.Contains(t, contract["endpoints"], "6379", "endpoints should contain port")

	require.Contains(t, contract["credentials"], "valkey-password", "credentials should contain secret name")
	require.Contains(t, contract["credentials"], "password", "credentials should contain secret key")

	require.Contains(t, contract["persistence"], "true", "persistence should be enabled")
	require.Contains(t, contract["persistence"], "1Gi", "persistence should contain storage size")

	require.Contains(t, contract["replication"], "1", "replication should show nodes count")
}

func TestValkeyModuleSyncPolicyContractAnonymous(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVarsAnonymous(t))
	terraform.InitAndApply(t, opts)

	contract := terraform.OutputMap(t, opts, "sync_policy_contract")
	require.NotEmpty(t, contract, "expected sync_policy_contract output")

	// Credentials should be null when anonymous_auth is true
	// OutputMap returns <nil> for null values
	require.Equal(t, "<nil>", contract["credentials"], "credentials should be null for anonymous auth")

	// Other required fields should still be present
	require.Contains(t, contract["cluster"], "test-valkey", "cluster should contain expected name")
	require.Contains(t, contract["endpoints"], "test-valkey-primary", "endpoints should contain primary host")
	require.Contains(t, contract["tls"], "enabled:false", "TLS should be disabled")
}

func TestValkeyModuleSyncPolicyContractWithTLS(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVarsWithTLS(t))
	terraform.InitAndApply(t, opts)

	contract := terraform.OutputMap(t, opts, "sync_policy_contract")
	require.NotEmpty(t, contract, "expected sync_policy_contract output")

	// TLS fields should reflect enabled state
	require.Contains(t, contract["tls"], "true", "TLS should be enabled")
	require.Contains(t, contract["tls"], "letsencrypt-staging", "TLS should contain cert issuer")

	// Credentials should be present (not anonymous)
	require.Contains(t, contract["credentials"], "valkey-password", "credentials should contain secret name")
}

func TestValkeyModuleSyncPolicyContractHA(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVarsHA(t))
	terraform.InitAndApply(t, opts)

	contract := terraform.OutputMap(t, opts, "sync_policy_contract")
	require.NotEmpty(t, contract, "expected sync_policy_contract output")

	// Replication fields should reflect HA configuration
	// OutputMap stringifies nested maps as "map[key:value ...]"
	require.Contains(t, contract["replication"], "nodes:3", "replication should show 3 nodes")
	require.Contains(t, contract["replication"], "replicas:1", "replication should show 1 replica")

	// Persistence should reflect HA storage size
	require.Contains(t, contract["persistence"], "2Gi", "persistence should contain HA storage size")

	// Cluster name should be the HA cluster name
	require.Contains(t, contract["cluster"], "test-valkey-ha", "cluster should contain HA cluster name")
}

func TestValkeyModuleEndpoints(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVars(t))
	terraform.InitAndApply(t, opts)

	primary := terraform.Output(t, opts, "primary_endpoint")
	require.NotEmpty(t, primary, "expected primary_endpoint output")
	require.Contains(t, primary, "-primary.", "expected primary endpoint to contain -primary suffix")

	replica := terraform.Output(t, opts, "replica_endpoint")
	require.NotEmpty(t, replica, "expected replica_endpoint output")
	require.Contains(t, replica, "-replicas.", "expected replica endpoint to contain -replicas suffix")
}

func TestValkeyModuleRenderWithESO(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVarsWithESO(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	require.NotEmpty(t, rendered, "expected rendered_manifests output to be non-empty")

	// Verify ExternalSecret for password is rendered
	passwordES, ok := rendered["platform/redis/external-secret-password.yaml"]
	require.True(t, ok, "expected external-secret-password.yaml when ESO enabled")
	require.Contains(t, passwordES, "ExternalSecret", "expected ExternalSecret kind")
	require.Contains(t, passwordES, "vault-backend", "expected ClusterSecretStore reference")

	// Verify NO inline password secret is rendered
	_, hasPasswordSecret := rendered["platform/redis/password-secret.yaml"]
	require.False(t, hasPasswordSecret, "should not render password-secret.yaml when ESO enabled")
}

func TestValkeyModuleRenderWithTLS(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVarsWithTLS(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	require.NotEmpty(t, rendered, "expected rendered_manifests output to be non-empty")

	// Verify cluster includes TLS configuration
	cluster, ok := rendered["platform/redis/valkey-cluster.yaml"]
	require.True(t, ok, "expected cluster manifest")
	require.True(
		t,
		strings.Contains(cluster, "tls: true") ||
			strings.Contains(cluster, "\"tls\": true"),
		"expected cluster to have TLS enabled",
	)
	require.Contains(t, cluster, "letsencrypt-staging", "expected cluster to reference cert issuer")
}

func TestValkeyModuleRenderAnonymous(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVarsAnonymous(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	require.NotEmpty(t, rendered, "expected rendered_manifests output to be non-empty")

	// Verify cluster has anonymous auth
	cluster, ok := rendered["platform/redis/valkey-cluster.yaml"]
	require.True(t, ok, "expected cluster manifest")
	require.True(
		t,
		strings.Contains(cluster, "anonymousAuth: true") ||
			strings.Contains(cluster, "\"anonymousAuth\": true"),
		"expected cluster to have anonymousAuth: true",
	)

	// Verify NO password secret is rendered
	_, hasPasswordSecret := rendered["platform/redis/password-secret.yaml"]
	require.False(t, hasPasswordSecret, "should not render password-secret.yaml when anonymous auth enabled")

	_, hasESPassword := rendered["platform/redis/external-secret-password.yaml"]
	require.False(t, hasESPassword, "should not render external-secret-password.yaml when anonymous auth enabled")
}

func TestValkeyModuleRenderPolicy(t *testing.T) {
	t.Parallel()
	requireBinary(t, binaryRequirement{Binary: "conftest", SkipMessage: "conftest not found; skipping policy test"})

	tfDir, opts := setupRender(t, renderVars(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	outDir := filepath.Join(tfDir, "rendered")
	require.NoError(t, os.MkdirAll(outDir, 0o755))
	t.Cleanup(func() { _ = os.RemoveAll(outDir) })

	for relPath, content := range rendered {
		dest := filepath.Join(outDir, relPath)
		require.NoError(t, os.MkdirAll(filepath.Dir(dest), 0o755))
		require.NoError(t, os.WriteFile(dest, []byte(content), 0o600))
	}

	policyPath := valkeyManifestsPolicyPath(tfDir)
	out, err := runConftest(t, conftestRun{
		InputPath:  outDir,
		PolicyPath: policyPath,
		Kubeconfig: "",
		ExtraArgs: []string{
			"--namespace",
			valkeyPolicyManifestsNamespace,
			"--combine",
		},
		Timeout: 60 * time.Second,
	})
	require.NoErrorf(t, err, "conftest failed: %s", string(out))
}

var renderPolicyRejectionTestCases = []struct {
	name            string
	manifest        string
	expectedMessage string
}{
	{
		name: "MissingChartVersion",
		manifest: `apiVersion: helm.toolkit.fluxcd.io/v2
kind: HelmRelease
metadata:
  name: valkey-operator
  namespace: valkey-system
  labels:
    app.kubernetes.io/part-of: valkey
spec:
  chart:
    spec:
      chart: valkey-operator
      sourceRef:
        kind: HelmRepository
        name: valkey-operator
        namespace: flux-system
`,
		expectedMessage: "must pin chart.spec.version",
	},
	{
		name: "ValkeyClusterMissingNodes",
		manifest: `apiVersion: hyperspike.io/v1
kind: Valkey
metadata:
  name: test-cluster
  namespace: valkey
  labels:
    app.kubernetes.io/part-of: valkey
spec:
  nodes: 0
  replicas: 0
  anonymousAuth: true
  clusterDomain: cluster.local
`,
		expectedMessage: "must set spec.nodes >= 1",
	},
	{
		name: "ValkeyClusterMissingStorageClass",
		manifest: `apiVersion: hyperspike.io/v1
kind: Valkey
metadata:
  name: test-cluster
  namespace: valkey
  labels:
    app.kubernetes.io/part-of: valkey
spec:
  nodes: 1
  replicas: 0
  anonymousAuth: true
  clusterDomain: cluster.local
  storage:
    resources:
      requests:
        storage: 1Gi
`,
		expectedMessage: "must set spec.storage.storageClassName",
	},
	{
		name: "ValkeyClusterTLSWithoutIssuer",
		manifest: `apiVersion: hyperspike.io/v1
kind: Valkey
metadata:
  name: test-cluster
  namespace: valkey
  labels:
    app.kubernetes.io/part-of: valkey
spec:
  nodes: 1
  replicas: 0
  anonymousAuth: true
  clusterDomain: cluster.local
  tls: true
`,
		expectedMessage: "has TLS enabled but no certIssuer specified",
	},
	{
		name: "ValkeyClusterNoPasswordWhenNotAnonymous",
		manifest: `apiVersion: hyperspike.io/v1
kind: Valkey
metadata:
  name: test-cluster
  namespace: valkey
  labels:
    app.kubernetes.io/part-of: valkey
spec:
  nodes: 1
  replicas: 0
  anonymousAuth: false
  clusterDomain: cluster.local
`,
		expectedMessage: "requires servicePassword.name when anonymousAuth is false",
	},
}

func TestValkeyModuleRenderPolicyRejections(t *testing.T) {
	t.Parallel()
	requireBinary(t, binaryRequirement{Binary: "conftest", SkipMessage: "conftest not found; skipping policy test"})

	tfDir, opts := setupRender(t, renderVars(t))
	terraform.Init(t, opts)
	policyPath := valkeyManifestsPolicyPath(tfDir)

	for _, tc := range renderPolicyRejectionTestCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			tmpDir := t.TempDir()
			manifestPath := filepath.Join(tmpDir, "manifest.yaml")
			require.NoError(t, os.WriteFile(manifestPath, []byte(tc.manifest), 0o600))

			out, err := runConftest(t, conftestRun{
				InputPath:  manifestPath,
				PolicyPath: policyPath,
				Kubeconfig: "",
				ExtraArgs: []string{
					"--fail-on-warn",
					"--namespace",
					valkeyPolicyManifestsNamespace,
				},
				Timeout: 60 * time.Second,
			})
			require.Error(t, err, "expected conftest to report a violation")
			require.Contains(t, string(out), tc.expectedMessage)
		})
	}
}
