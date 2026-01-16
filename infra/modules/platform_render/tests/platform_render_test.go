package tests

import (
	"regexp"
	"strings"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
	testutil "wildside/infra/testutil"
)

// testVars returns minimal valid inputs for the platform_render module.
func testVars(t *testing.T) map[string]interface{} {
	t.Helper()
	return map[string]interface{}{
		"cluster_name":                     "preview-test",
		"domain":                           "example.test",
		"acme_email":                       "admin@example.test",
		"cloudflare_api_token_secret_name": "cloudflare-api-token",
		"vault_address":                    "https://vault.example.test:8200",
		"vault_approle_role_id":            "test-role-id",
		"vault_approle_secret_id":          "test-secret-id",
		"traefik_enabled":                  true,
		"cert_manager_enabled":             true,
		"external_dns_enabled":             true,
		"vault_eso_enabled":                true,
		"cnpg_enabled":                     true,
		"valkey_enabled":                   false,
	}
}

// setup creates a temporary directory with the module and returns the path
// and terraform options.
func setup(t *testing.T, vars map[string]interface{}) (string, *terraform.Options) {
	t.Helper()
	return testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "..",
		TfSubDir:      "examples/full",
		Vars:          vars,
	})
}

func TestPlatformRenderModuleValidate(t *testing.T) {
	t.Parallel()
	_, opts := setup(t, testVars(t))
	terraform.InitAndValidate(t, opts)
}

func TestPlatformRenderModuleRendersManifests(t *testing.T) {
	t.Parallel()

	vars := testVars(t)
	_, opts := setup(t, vars)
	terraform.InitAndApply(t, opts)

	// Verify rendered_manifests output is not empty
	renderedRaw := terraform.Output(t, opts, "rendered_manifests")
	require.NotEmpty(t, renderedRaw, "expected rendered_manifests output to be non-empty")

	// Verify manifest_count is > 0
	count := terraform.Output(t, opts, "manifest_count")
	require.NotEmpty(t, count)
	require.NotEqual(t, "0", count, "expected manifest_count to be greater than 0")

	// Verify enabled_modules contains expected modules
	enabledModulesRaw := terraform.OutputList(t, opts, "enabled_modules")
	expectedModules := []string{"traefik", "cert_manager", "external_dns", "vault_eso", "cnpg"}
	for _, mod := range expectedModules {
		found := false
		for _, enabled := range enabledModulesRaw {
			if enabled == mod {
				found = true
				break
			}
		}
		require.True(t, found, "expected %s to be in enabled_modules", mod)
	}
}

func TestPlatformRenderModuleFeatureFlagsDisableModules(t *testing.T) {
	t.Parallel()

	vars := testVars(t)
	// Disable all modules except traefik
	vars["traefik_enabled"] = true
	vars["cert_manager_enabled"] = false
	vars["external_dns_enabled"] = false
	vars["vault_eso_enabled"] = false
	vars["cnpg_enabled"] = false

	_, opts := setup(t, vars)
	terraform.InitAndApply(t, opts)

	// Verify only traefik is enabled
	enabledModulesRaw := terraform.OutputList(t, opts, "enabled_modules")
	require.Contains(t, enabledModulesRaw, "traefik")
	require.NotContains(t, enabledModulesRaw, "cert_manager")
	require.NotContains(t, enabledModulesRaw, "external_dns")
	require.NotContains(t, enabledModulesRaw, "vault_eso")
	require.NotContains(t, enabledModulesRaw, "cnpg")
}

func TestPlatformRenderModuleOutputsTraefikIngressClassName(t *testing.T) {
	t.Parallel()

	vars := testVars(t)
	_, opts := setup(t, vars)
	terraform.InitAndApply(t, opts)

	ingressClass := terraform.Output(t, opts, "traefik_ingress_class_name")
	require.NotEmpty(t, ingressClass, "expected traefik_ingress_class_name to be non-empty")
	require.Equal(t, "traefik", ingressClass)
}

func TestPlatformRenderModuleOutputsCNPGEndpoint(t *testing.T) {
	t.Parallel()

	vars := testVars(t)
	_, opts := setup(t, vars)
	terraform.InitAndApply(t, opts)

	primaryEndpoint := terraform.Output(t, opts, "cnpg_primary_endpoint")
	require.NotEmpty(t, primaryEndpoint, "expected cnpg_primary_endpoint to be non-empty")
}

// inputValidationTestCases defines test cases for Terraform variable validation.
var inputValidationTestCases = []struct {
	name         string
	varName      string
	invalidValue interface{}
	errorPattern string
}{
	{
		name:         "BlankClusterName",
		varName:      "cluster_name",
		invalidValue: "   ",
		errorPattern: "cluster_name",
	},
	{
		name:         "InvalidClusterNameChars",
		varName:      "cluster_name",
		invalidValue: "Invalid_Cluster_Name",
		errorPattern: "cluster_name",
	},
	{
		name:         "InvalidDomain",
		varName:      "domain",
		invalidValue: "not-a-domain",
		errorPattern: "domain",
	},
	{
		name:         "InvalidEmail",
		varName:      "acme_email",
		invalidValue: "not-an-email",
		errorPattern: "acme_email",
	},
	{
		name:         "BlankCloudflareSecretName",
		varName:      "cloudflare_api_token_secret_name",
		invalidValue: "",
		errorPattern: "cloudflare_api_token_secret_name",
	},
	{
		name:         "InvalidVaultAddress",
		varName:      "vault_address",
		invalidValue: "http://vault.example.test",
		errorPattern: "vault_address",
	},
	{
		name:         "ValkeyEnabledNotAllowed",
		varName:      "valkey_enabled",
		invalidValue: true,
		errorPattern: "valkey",
	},
}

func TestPlatformRenderModuleInputValidation(t *testing.T) {
	t.Parallel()

	for _, tc := range inputValidationTestCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			vars := testVars(t)
			vars[tc.varName] = tc.invalidValue
			_, opts := setup(t, vars)
			_, err := terraform.InitAndPlanE(t, opts)
			require.Error(t, err, "expected validation error for %s", tc.varName)
			require.Regexp(t, regexp.MustCompile("(?i)"+tc.errorPattern), err.Error(),
				"expected error message to mention %s", tc.varName)
		})
	}
}

func TestPlatformRenderModuleManifestPathsContainExpectedKeys(t *testing.T) {
	t.Parallel()

	vars := testVars(t)
	_, opts := setup(t, vars)
	terraform.InitAndApply(t, opts)

	// Get manifest counts by module
	countsRaw := terraform.OutputMapOfObjects(t, opts, "manifest_counts_by_module")

	// Verify each enabled module contributed manifests
	expectedModules := []string{"traefik", "cert_manager", "external_dns", "vault_eso", "cnpg"}
	for _, mod := range expectedModules {
		count, exists := countsRaw[mod]
		require.True(t, exists, "expected manifest_counts_by_module to contain %s", mod)
		// The count should be a number > 0
		countStr := strings.TrimSpace(count.(string))
		require.NotEqual(t, "0", countStr, "expected %s to contribute manifests", mod)
	}
}
