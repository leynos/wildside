package tests

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

type syncPolicyContract struct {
	KVSecretStore       *secretStoreRef `json:"kv_secret_store"`
	VaultAddress        string          `json:"vault_address"`
	AuthSecretName      string          `json:"auth_secret_name"`
	AuthSecretNamespace string          `json:"auth_secret_namespace"`
}

type secretStoreRef struct {
	Name      string `json:"name"`
	Kind      string `json:"kind"`
	MountPath string `json:"mount_path"`
}

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
	vars["kv_mount_path"] = "secret"
	vars["cluster_secret_store_kv_name"] = "vault-kv"

	_, opts := setupRender(t, vars)
	terraform.InitAndApply(t, opts)

	contractJSON := terraform.OutputJson(t, opts, "sync_policy_contract")
	require.NotEmpty(t, contractJSON, "expected sync_policy_contract output")

	var contract syncPolicyContract
	require.NoError(t, json.Unmarshal([]byte(contractJSON), &contract))

	// Verify KV secret store structure
	require.NotNil(t, contract.KVSecretStore, "kv_secret_store must be present")
	require.Equal(t, "vault-kv", contract.KVSecretStore.Name)
	require.Equal(t, "ClusterSecretStore", contract.KVSecretStore.Kind)
	require.Equal(t, "secret", contract.KVSecretStore.MountPath)

	// Verify auth and vault fields
	require.Equal(t, "https://vault.example.test:8200", contract.VaultAddress)
	require.Equal(t, "vault-approle-credentials", contract.AuthSecretName)
	require.Equal(t, "external-secrets", contract.AuthSecretNamespace)
}
