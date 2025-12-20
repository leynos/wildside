package tests

import (
	"regexp"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

func TestCertManagerExampleRejectsBlankKubeconfigPath(t *testing.T) {
	t.Parallel()
	testKubeconfigPathValidation(t, "")
}

func TestCertManagerExampleRejectsWhitespaceKubeconfig(t *testing.T) {
	t.Parallel()
	testKubeconfigPathValidation(t, " \n\t ")
}

func TestCertManagerExampleRejectsMissingKubeconfig(t *testing.T) {
	t.Parallel()
	testKubeconfigPathValidation(t, "/nonexistent/kubeconfig")
}

func TestCertManagerModuleInvalidAcmeEmail(t *testing.T) {
	t.Parallel()
	vars := renderVars(t)
	vars["acme_email"] = "not-an-email"
	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`acme_email`), err.Error())
}

func TestCertManagerModuleMissingNamecheapSecret(t *testing.T) {
	t.Parallel()
	vars := renderVars(t)
	vars["namecheap_api_secret_name"] = ""
	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`namecheap_api_secret_name`), err.Error())
}

func TestCertManagerModuleInvalidVaultServer(t *testing.T) {
	t.Parallel()
	vars := renderVars(t)
	vars["vault_server"] = "http://vault.example.test"
	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`vault_server`), err.Error())
}

func TestCertManagerModuleMissingVaultCaBundle(t *testing.T) {
	t.Parallel()
	vars := renderVars(t)
	vars["vault_ca_bundle_pem"] = " "
	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`vault_ca_bundle_pem`), err.Error())
}

func TestCertManagerModuleInvalidNamespace(t *testing.T) {
	t.Parallel()
	vars := renderVars(t)
	vars["namespace"] = "Invalid_Namespace"
	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`namespace`), err.Error())
}

func TestCertManagerModuleInvalidChartVersion(t *testing.T) {
	t.Parallel()
	vars := renderVars(t)
	vars["chart_version"] = "invalid"
	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`chart_version`), err.Error())
}
