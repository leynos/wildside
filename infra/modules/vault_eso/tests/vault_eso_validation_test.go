package tests

import (
	"strings"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

var validationErrorCases = []struct {
	name           string
	varName        string
	varValue       interface{}
	expectedSubstr string
}{
	{
		name:           "InvalidVaultAddressNotHTTPS",
		varName:        "vault_address",
		varValue:       "http://vault.example.test:8200",
		expectedSubstr: "vault_address must be an https:// URL",
	},
	{
		name:           "EmptyVaultAddress",
		varName:        "vault_address",
		varValue:       "",
		expectedSubstr: "vault_address must be an https:// URL",
	},
	{
		name:           "EmptyApproleRoleId",
		varName:        "approle_role_id",
		varValue:       "",
		expectedSubstr: "approle_role_id must not be blank",
	},
	{
		name:           "EmptyApproleSecretId",
		varName:        "approle_secret_id",
		varValue:       "",
		expectedSubstr: "approle_secret_id must not be blank",
	},
	{
		name:           "InvalidCertificate",
		varName:        "vault_ca_bundle_pem",
		varValue:       "not-a-certificate",
		expectedSubstr: "vault_ca_bundle_pem must be a valid PEM-encoded certificate",
	},
	{
		name:           "InvalidModeValue",
		varName:        "mode",
		varValue:       "invalid",
		expectedSubstr: "mode must be one of: render, apply",
	},
	{
		name:           "InvalidNamespace",
		varName:        "namespace",
		varValue:       "Invalid_Namespace",
		expectedSubstr: "namespace must be a valid Kubernetes namespace name",
	},
	{
		name:           "ZeroWebhookReplicaCount",
		varName:        "webhook_replica_count",
		varValue:       0,
		expectedSubstr: "webhook_replica_count must be greater than zero",
	},
}

func TestVaultESOModuleValidationErrors(t *testing.T) {
	t.Parallel()

	for _, tc := range validationErrorCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			vars := renderVars(t)
			vars[tc.varName] = tc.varValue

			_, opts := setupRender(t, vars)
			stdout, err := terraform.InitAndPlanE(t, opts)
			require.Error(t, err)
			combined := strings.Join([]string{stdout, err.Error()}, "\n")
			require.Contains(t, combined, tc.expectedSubstr)
		})
	}
}

func TestVaultESOModuleSyncPolicyContractOutput(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["pki_enabled"] = true
	vars["pki_mount_path"] = "pki"
	vars["kv_mount_path"] = "secret"
	vars["cluster_secret_store_kv_name"] = "vault-kv"
	vars["cluster_secret_store_pki_name"] = "vault-pki"

	_, opts := setupRender(t, vars)
	terraform.InitAndApply(t, opts)

	contract := terraform.OutputMap(t, opts, "sync_policy_contract")
	require.NotEmpty(t, contract, "expected sync_policy_contract output")
}
