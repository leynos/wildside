package tests

import (
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

func TestCNPGModuleValidation(t *testing.T) {
	t.Parallel()

	testCases := []struct {
		name        string
		vars        map[string]interface{}
		expectError string
	}{
		{
			name:        "InvalidMode",
			vars:        map[string]interface{}{"mode": "invalid"},
			expectError: "expected validation error for invalid mode",
		},
		{
			name:        "ZeroInstances",
			vars:        map[string]interface{}{"instances": 0},
			expectError: "expected validation error for zero instances",
		},
		{
			name:        "InvalidNamespace",
			vars:        map[string]interface{}{"cluster_namespace": "Invalid_Namespace"},
			expectError: "expected validation error for invalid namespace",
		},
		{
			name:        "InvalidStorageSize",
			vars:        map[string]interface{}{"storage_size": "not-a-size"},
			expectError: "expected validation error for invalid storage size",
		},
		{
			name:        "InvalidChartVersion",
			vars:        map[string]interface{}{"chart_version": "not-a-version"},
			expectError: "expected validation error for invalid chart version",
		},
		{
			name:        "InvalidDatabaseName",
			vars:        map[string]interface{}{"database_name": "123-invalid"},
			expectError: "expected validation error for invalid database name",
		},
		{
			name:        "InvalidUpdateStrategy",
			vars:        map[string]interface{}{"primary_update_strategy": "invalid"},
			expectError: "expected validation error for invalid update strategy",
		},
		{
			name: "InvalidBackupDestination",
			vars: map[string]interface{}{
				"backup_enabled":          true,
				"backup_destination_path": "not-an-s3-uri",
			},
			expectError: "expected validation error for invalid backup destination",
		},
		{
			name: "InvalidBackupEndpoint",
			vars: map[string]interface{}{
				"backup_enabled":          true,
				"backup_destination_path": "s3://valid-bucket/path/",
				"backup_endpoint_url":     "http://insecure.endpoint.com",
			},
			expectError: "expected validation error for non-HTTPS backup endpoint",
		},
		{
			name:        "InvalidWalCompression",
			vars:        map[string]interface{}{"wal_compression": "invalid"},
			expectError: "expected validation error for invalid WAL compression",
		},
	}

	for _, tc := range testCases {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			vars := renderVars(t)
			for k, v := range tc.vars {
				vars[k] = v
			}

			_, opts := setupRender(t, vars)
			_, err := terraform.InitAndPlanE(t, opts)
			require.Error(t, err, tc.expectError)
		})
	}
}
