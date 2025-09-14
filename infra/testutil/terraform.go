package testutil

import (
	"path/filepath"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/gruntwork-io/terratest/modules/test-structure"
)

// SetupTerraform copies a Terraform configuration to a temporary directory and
// returns its path and options for execution.
func SetupTerraform(t *testing.T, sourceRootRel, tfSubDir string, vars map[string]interface{}, env map[string]string) (string, *terraform.Options) {
	t.Helper()
	tempRoot := test_structure.CopyTerraformFolderToTemp(t, sourceRootRel, ".")
	tfDir := filepath.Join(tempRoot, tfSubDir)
	opts := terraform.WithDefaultRetryableErrors(t, &terraform.Options{
		TerraformDir:    tfDir,
		TerraformBinary: "tofu",
		Vars:            vars,
		EnvVars:         env,
		NoColor:         true,
	})
	return tfDir, opts
}
