package tests

import (
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
)

func TestCNPGModuleValidationInvalidMode(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["mode"] = "invalid"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid mode")
}

func TestCNPGModuleValidationZeroInstances(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["instances"] = 0

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for zero instances")
}

func TestCNPGModuleValidationInvalidNamespace(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["cluster_namespace"] = "Invalid_Namespace"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid namespace")
}

func TestCNPGModuleValidationInvalidStorageSize(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["storage_size"] = "not-a-size"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid storage size")
}

func TestCNPGModuleValidationInvalidChartVersion(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["chart_version"] = "not-a-version"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid chart version")
}

func TestCNPGModuleValidationInvalidDatabaseName(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["database_name"] = "123-invalid"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid database name")
}

func TestCNPGModuleValidationInvalidUpdateStrategy(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["primary_update_strategy"] = "invalid"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid update strategy")
}

func TestCNPGModuleValidationInvalidBackupDestination(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["backup_enabled"] = true
	vars["backup_destination_path"] = "not-an-s3-uri"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid backup destination")
}

func TestCNPGModuleValidationInvalidBackupEndpoint(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["backup_enabled"] = true
	vars["backup_destination_path"] = "s3://valid-bucket/path/"
	vars["backup_endpoint_url"] = "http://insecure.endpoint.com"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for non-HTTPS backup endpoint")
}

func TestCNPGModuleValidationInvalidWalCompression(t *testing.T) {
	t.Parallel()

	vars := renderVars(t)
	vars["wal_compression"] = "invalid"

	_, opts := setupRender(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err, "expected validation error for invalid WAL compression")
}
