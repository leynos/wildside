package tests

import (
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

type validationTestCase struct {
	name              string
	vars              map[string]interface{}
	expectError       string
	expectedSubstring string
}

var validationTestCases = []validationTestCase{
	{"InvalidMode", map[string]interface{}{"mode": "invalid"}, "expected validation error for invalid mode", "mode"},
	{"ZeroInstances", map[string]interface{}{"instances": 0}, "expected validation error for zero instances", "instances"},
	{"InvalidNamespace", map[string]interface{}{"cluster_namespace": "Invalid_Namespace"}, "expected validation error for invalid namespace", "cluster_namespace"},
	{"InvalidStorageSize", map[string]interface{}{"storage_size": "not-a-size"}, "expected validation error for invalid storage size", "storage_size"},
	{"InvalidChartVersion", map[string]interface{}{"chart_version": "not-a-version"}, "expected validation error for invalid chart version", "chart_version"},
	{"InvalidDatabaseName", map[string]interface{}{"database_name": "123-invalid"}, "expected validation error for invalid database name", "database_name"},
	{"InvalidUpdateStrategy", map[string]interface{}{"primary_update_strategy": "invalid"}, "expected validation error for invalid update strategy", "primary_update_strategy"},
	{"InvalidBackupDestination", map[string]interface{}{"backup_enabled": true, "backup_destination_path": "not-an-s3-uri"}, "expected validation error for invalid backup destination", "backup_destination_path"},
	{"InvalidBackupEndpoint", map[string]interface{}{"backup_enabled": true, "backup_destination_path": "s3://valid-bucket/path/", "backup_endpoint_url": "http://insecure.endpoint.com"}, "expected validation error for non-HTTPS backup endpoint", "backup_endpoint_url"},
	{"InvalidWalCompression", map[string]interface{}{"wal_compression": "invalid"}, "expected validation error for invalid WAL compression", "wal_compression"},
}

func TestCNPGModuleValidation(t *testing.T) {
	t.Parallel()
	for _, tc := range validationTestCases {
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
			require.ErrorContains(t, err, tc.expectedSubstring, "error should mention the invalid variable")
		})
	}
}
