package tests

import (
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

// validationTestCases tests that the module accepts valid configurations.
// Note: Invalid configuration tests are challenging to run reliably via
// terratest's InitAndValidateE due to how OpenTofu surfaces validation
// errors. The module's variable validation rules are defined in the
// variables-*.tf files and are exercised by OpenTofu at plan/apply time.
var validationTestCases = []struct {
	name string
	vars map[string]interface{}
}{
	{
		name: "ValidMinimalConfig",
		vars: map[string]interface{}{
			"cluster_name":    "test-valkey",
			"nodes":           1,
			"replicas":        0,
			"storage_size":    "1Gi",
			"password_inline": "test-password",
		},
	},
	{
		name: "ValidHAConfig",
		vars: map[string]interface{}{
			"cluster_name":    "test-valkey-ha",
			"nodes":           3,
			"replicas":        1,
			"storage_size":    "2Gi",
			"password_inline": "test-password",
		},
	},
	{
		name: "ValidAnonymousConfig",
		vars: map[string]interface{}{
			"cluster_name":   "test-valkey",
			"nodes":          1,
			"replicas":       0,
			"storage_size":   "1Gi",
			"anonymous_auth": true,
		},
	},
	{
		name: "ValidWithTLS",
		vars: map[string]interface{}{
			"cluster_name":     "test-valkey",
			"nodes":            1,
			"replicas":         0,
			"storage_size":     "1Gi",
			"password_inline":  "test-password",
			"tls_enabled":      true,
			"cert_issuer_name": "letsencrypt-staging",
			"cert_issuer_type": "ClusterIssuer",
		},
	},
	{
		name: "ValidWithESO",
		vars: map[string]interface{}{
			"cluster_name":                  "test-valkey",
			"nodes":                         1,
			"replicas":                      0,
			"storage_size":                  "1Gi",
			"eso_enabled":                   true,
			"eso_cluster_secret_store_name": "vault-backend",
			"password_vault_path":           "secret/data/valkey/password",
		},
	},
}

func TestValkeyModuleValidation(t *testing.T) {
	t.Parallel()

	for _, tc := range validationTestCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			_, opts := setupRender(t, tc.vars)

			_, err := terraform.InitAndValidateE(t, opts)
			require.NoError(t, err, "expected validation to pass")
		})
	}
}
