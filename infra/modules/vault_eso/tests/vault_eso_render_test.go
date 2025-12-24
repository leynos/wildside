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

func TestVaultESOModuleValidate(t *testing.T) {
	t.Parallel()
	_, opts := setup(t, testVars(t))
	terraform.InitAndValidate(t, opts)
}

func TestVaultESOModuleRenderOutputs(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVars(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	require.NotEmpty(t, rendered, "expected rendered_manifests output to be non-empty")

	helmRelease, ok := rendered["platform/vault/helmrelease.yaml"]
	require.True(t, ok, "expected platform/vault/helmrelease.yaml output key")
	require.True(
		t,
		strings.Contains(helmRelease, "kind: HelmRelease") ||
			strings.Contains(helmRelease, "\"kind\": \"HelmRelease\""),
		"expected HelmRelease manifest to contain kind HelmRelease",
	)
	require.True(
		t,
		strings.Contains(helmRelease, "external-secrets") ||
			strings.Contains(helmRelease, "\"external-secrets\""),
		"expected HelmRelease manifest to reference external-secrets",
	)

	_, ok = rendered["platform/sources/external-secrets-repo.yaml"]
	require.True(t, ok, "expected platform/sources/external-secrets-repo.yaml output key")

	_, ok = rendered["platform/vault/cluster-secret-store-kv.yaml"]
	require.True(t, ok, "expected KV ClusterSecretStore output")

	_, ok = rendered["platform/vault/approle-auth-secret.yaml"]
	require.True(t, ok, "expected approle-auth-secret output")

	_, ok = rendered["platform/vault/kustomization.yaml"]
	require.True(t, ok, "expected kustomization output")
}

func TestVaultESOModuleRenderPolicy(t *testing.T) {
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

	policyPath := vaultESOManifestsPolicyPath(tfDir)
	out, err := runConftest(t, conftestRun{
		InputPath:  outDir,
		PolicyPath: policyPath,
		Kubeconfig: "",
		ExtraArgs: []string{
			"--namespace",
			vaultESOPolicyManifestsNamespace,
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
  name: external-secrets
  namespace: external-secrets
  labels:
    app.kubernetes.io/part-of: external-secrets
spec:
  chart:
    spec:
      chart: external-secrets
      sourceRef:
        kind: HelmRepository
        name: external-secrets
        namespace: flux-system
  install:
    crds: CreateReplace
  values:
    webhook:
      replicaCount: 2
`,
		expectedMessage: "must pin chart.spec.version",
	},
	{
		name: "MissingVaultCaBundle",
		manifest: `apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: vault-kv
  labels:
    app.kubernetes.io/part-of: external-secrets
spec:
  provider:
    vault:
      server: https://vault.example.test:8200
      path: secret
      version: v2
      auth:
        appRole:
          path: approle
          roleRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: role_id
          secretRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: secret_id
`,
		expectedMessage: "must set provider.vault.caBundle",
	},
	{
		name: "NonHTTPSVaultServer",
		manifest: `apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: vault-kv
  labels:
    app.kubernetes.io/part-of: external-secrets
spec:
  provider:
    vault:
      server: http://vault.example.test:8200
      path: secret
      version: v2
      caBundle: dGVzdA==
      auth:
        appRole:
          path: approle
          roleRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: role_id
          secretRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: secret_id
`,
		expectedMessage: "must use HTTPS Vault server URL",
	},
	{
		name: "MissingVaultPath",
		manifest: `apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: vault-kv
  labels:
    app.kubernetes.io/part-of: external-secrets
spec:
  provider:
    vault:
      server: https://vault.example.test:8200
      version: v2
      caBundle: dGVzdA==
      auth:
        appRole:
          path: approle
          roleRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: role_id
          secretRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: secret_id
`,
		expectedMessage: "must set provider.vault.path",
	},
	{
		name: "MissingAppRoleSecretRefName",
		manifest: `apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: vault-kv
  labels:
    app.kubernetes.io/part-of: external-secrets
spec:
  provider:
    vault:
      server: https://vault.example.test:8200
      path: secret
      version: v2
      caBundle: dGVzdA==
      auth:
        appRole:
          path: approle
          roleRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: role_id
          secretRef:
            namespace: external-secrets
            key: secret_id
`,
		expectedMessage: "must set appRole.secretRef.name",
	},
	{
		name: "MissingAppRoleRoleRefKey",
		manifest: `apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: vault-kv
  labels:
    app.kubernetes.io/part-of: external-secrets
spec:
  provider:
    vault:
      server: https://vault.example.test:8200
      path: secret
      version: v2
      caBundle: dGVzdA==
      auth:
        appRole:
          path: approle
          roleRef:
            name: vault-approle-credentials
            namespace: external-secrets
          secretRef:
            name: vault-approle-credentials
            namespace: external-secrets
            key: secret_id
`,
		expectedMessage: "must set appRole.roleRef.key",
	},
}

func TestVaultESOModuleRenderPolicyRejections(t *testing.T) {
	t.Parallel()
	requireBinary(t, binaryRequirement{Binary: "conftest", SkipMessage: "conftest not found; skipping policy test"})

	tfDir, opts := setupRender(t, renderVars(t))
	terraform.Init(t, opts)
	policyPath := vaultESOManifestsPolicyPath(tfDir)

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
					vaultESOPolicyManifestsNamespace,
				},
				Timeout: 60 * time.Second,
			})
			require.Error(t, err, "expected conftest to report a violation")
			require.Contains(t, string(out), tc.expectedMessage)
		})
	}
}
