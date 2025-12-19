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

func TestCertManagerModuleValidate(t *testing.T) {
	t.Parallel()
	_, opts := setup(t, testVars(t))
	terraform.InitAndValidate(t, opts)
}

func TestCertManagerModuleRenderOutputs(t *testing.T) {
	t.Parallel()

	_, opts := setupRender(t, renderVars(t))
	terraform.InitAndApply(t, opts)

	rendered := terraform.OutputMap(t, opts, "rendered_manifests")
	require.NotEmpty(t, rendered, "expected rendered_manifests output to be non-empty")

	helmRelease, ok := rendered["platform/cert-manager/helmrelease.yaml"]
	require.True(t, ok, "expected platform/cert-manager/helmrelease.yaml output key")
	require.True(
		t,
		strings.Contains(helmRelease, "kind: HelmRelease") ||
			strings.Contains(helmRelease, "\"kind\": \"HelmRelease\""),
		"expected HelmRelease manifest to contain kind HelmRelease",
	)
	require.True(
		t,
		strings.Contains(helmRelease, "cert-manager") ||
			strings.Contains(helmRelease, "\"cert-manager\""),
		"expected HelmRelease manifest to reference cert-manager",
	)

	_, ok = rendered["platform/sources/cert-manager-repo.yaml"]
	require.True(t, ok, "expected platform/sources/cert-manager-repo.yaml output key")

	_, ok = rendered["platform/cert-manager/cluster-issuer-acme-staging.yaml"]
	require.True(t, ok, "expected ACME staging ClusterIssuer output")

	_, ok = rendered["platform/cert-manager/cluster-issuer-acme-production.yaml"]
	require.True(t, ok, "expected ACME production ClusterIssuer output")

	_, ok = rendered["platform/cert-manager/cluster-issuer-vault.yaml"]
	require.True(t, ok, "expected Vault ClusterIssuer output")

	_, ok = rendered["platform/cert-manager/kustomization.yaml"]
	require.True(t, ok, "expected kustomization output")
}

func TestCertManagerModuleRenderPolicy(t *testing.T) {
	t.Parallel()
	requireBinary(t, "conftest", "conftest not found; skipping policy test")

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

	policyPath := certManagerManifestsPolicyPath(tfDir)
	out, err := runConftest(t, conftestRun{
		InputPath:  outDir,
		PolicyPath: policyPath,
		Kubeconfig: "",
		ExtraArgs: []string{
			"--namespace",
			certManagerPolicyManifestsNamespace,
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
  name: cert-manager
  namespace: cert-manager
  labels:
    app.kubernetes.io/part-of: cert-manager
spec:
  chart:
    spec:
      chart: cert-manager
      sourceRef:
        kind: HelmRepository
        name: jetstack
        namespace: flux-system
  install:
    crds: CreateReplace
  values:
    replicaCount: 3
    webhook:
      replicaCount: 3
    cainjector:
      replicaCount: 3
`,
		expectedMessage: "must pin chart.spec.version",
	},
	{
		name: "MissingWebhookGroupName",
		manifest: `apiVersion: helm.toolkit.fluxcd.io/v2
kind: HelmRelease
metadata:
  name: cert-manager-webhook-namecheap
  namespace: cert-manager
  labels:
    app.kubernetes.io/part-of: cert-manager
spec:
  chart:
    spec:
      chart: cert-manager-webhook-namecheap
      version: "0.2.0"
      sourceRef:
        kind: HelmRepository
        name: private-helm-repo
        namespace: flux-system
  values:
    replicaCount: 2
`,
		expectedMessage: "must set values.groupName",
	},
	{
		name: "MissingVaultCaBundle",
		manifest: `apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: vault-issuer
  labels:
    app.kubernetes.io/part-of: cert-manager
spec:
  vault:
    server: https://vault.example.test:8200
    path: pki/sign/example
    auth:
      tokenSecretRef:
        name: vault-token
        key: token
`,
		expectedMessage: "must set vault.caBundle",
	},
}

func TestCertManagerModuleRenderPolicyRejections(t *testing.T) {
	t.Parallel()
	requireBinary(t, "conftest", "conftest not found; skipping policy test")

	tfDir, _ := setupRender(t, renderVars(t))
	policyPath := certManagerManifestsPolicyPath(tfDir)

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
					certManagerPolicyManifestsNamespace,
				},
				Timeout: 60 * time.Second,
			})
			require.Error(t, err, "expected conftest to report a violation")
			require.Contains(t, string(out), tc.expectedMessage)
		})
	}
}
