package testutil

import (
	"os/exec"
	"path/filepath"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/gruntwork-io/terratest/modules/test-structure"
)

// TerraformConfig bundles configuration for SetupTerraform.
//
// Example:
//
//	tfDir, opts := testutil.SetupTerraform(t, testutil.TerraformConfig{
//	        SourceRootRel: "..",
//	        TfSubDir:      "examples/basic",
//	        Vars:          map[string]interface{}{"foo": "bar"},
//	        EnvVars:       map[string]string{"DIGITALOCEAN_TOKEN": "dummy"},
//	})
//
// The function copies the Terraform configuration to a temporary directory,
// returning the directory path and execution options.
type TerraformConfig struct {
	SourceRootRel string
	TfSubDir      string
	Vars          map[string]interface{}
	EnvVars       map[string]string
}

// SetupTerraform copies a Terraform configuration to a temporary directory and
// returns its path and options for execution.
func SetupTerraform(t *testing.T, config TerraformConfig) (string, *terraform.Options) {
	t.Helper()
	if _, err := exec.LookPath("tofu"); err != nil {
		t.Skip("tofu not found; skipping Terraform-based tests")
	}
	tempRoot := test_structure.CopyTerraformFolderToTemp(t, config.SourceRootRel, ".")
	tfDir := filepath.Join(tempRoot, config.TfSubDir)
	opts := terraform.WithDefaultRetryableErrors(t, &terraform.Options{
		TerraformDir:    tfDir,
		TerraformBinary: "tofu",
		Vars:            config.Vars,
		EnvVars:         config.EnvVars,
		NoColor:         true,
	})
	return tfDir, opts
}
