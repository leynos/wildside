package tests

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
	testutil "wildside/infra/testutil"
)

// testVars returns a baseline variable set matching the defaults in variables.tf.
func testVars(t *testing.T) map[string]interface{} {
	return map[string]interface{}{
		"cluster_name":       "wildside-dev",
		"region":             "nyc1",
		"kubernetes_version": testutil.KubernetesVersion(),
		"node_pools": []map[string]interface{}{
			{
				"name":       "default",
				"size":       "s-2vcpu-2gb",
				"node_count": 2,
				"auto_scale": false,
				"min_nodes":  2,
				"max_nodes":  2,
				"tags":       []string{"env:dev"},
			},
		},
		"tags":              []string{"env:dev"},
		"expose_kubeconfig": false,
	}
}

func TestDevClusterValidate(t *testing.T) {
	t.Parallel()
	_, opts := testutil.SetupTerraform(t, "../../..", "clusters/dev", testVars(t), map[string]string{})
	terraform.InitAndValidate(t, opts)
}

func TestDevClusterPlanUnauthenticated(t *testing.T) {
	t.Parallel()
	if os.Getenv("DIGITALOCEAN_TOKEN") != "" {
		t.Skip("DIGITALOCEAN_TOKEN set; skipping unauthenticated plan")
	}
	// The DigitalOcean provider does not require authentication at plan time,
	// so an unauthenticated plan should succeed.
	_, opts := testutil.SetupTerraform(t, "../../..", "clusters/dev", testVars(t), map[string]string{})
	_, err := terraform.InitAndPlanE(t, opts)
	require.NoError(t, err)
}

func TestDevClusterPlanDetailedExitCode(t *testing.T) {
	t.Parallel()
	token := os.Getenv("DIGITALOCEAN_TOKEN")
	if token == "" {
		t.Skip("DIGITALOCEAN_TOKEN not set; skipping detailed exit code plan")
	}
	tfDir, opts := testutil.SetupTerraform(t, "../../..", "clusters/dev", testVars(t), map[string]string{"DIGITALOCEAN_TOKEN": token})
	terraform.Init(t, opts)
	cmd := exec.Command("tofu", "plan", "-detailed-exitcode")
	cmd.Dir = tfDir
	cmd.Env = append(os.Environ(), "DIGITALOCEAN_TOKEN="+token)
	err := cmd.Run()
	if err == nil {
		t.Fatalf("expected exit code 2 (changes present), got 0")
	}
	exitErr, ok := err.(*exec.ExitError)
	require.True(t, ok, "expected ExitError")
	require.Equal(t, 2, exitErr.ExitCode())
}

func TestDevClusterPolicy(t *testing.T) {
	t.Parallel()
	token := os.Getenv("DIGITALOCEAN_TOKEN")
	if token == "" {
		t.Skip("DIGITALOCEAN_TOKEN not set; skipping policy test")
	}
	tfDir, opts := testutil.SetupTerraform(t, "../../..", "clusters/dev", testVars(t), map[string]string{"DIGITALOCEAN_TOKEN": token})
	planFile := filepath.Join(tfDir, "tfplan.binary")
	opts.PlanFilePath = planFile
	terraform.InitAndPlan(t, opts)

	show, err := terraform.RunTerraformCommandE(t, opts, "show", "-json", planFile)
	require.NoError(t, err)
	planJSON := filepath.Join(tfDir, "plan.json")
	require.NoError(t, os.WriteFile(planJSON, []byte(show), 0600))
	policyPath, err := filepath.Abs(filepath.Join(tfDir, "..", "..", "modules", "doks", "policy"))
	require.NoError(t, err)
	if _, lookErr := exec.LookPath("conftest"); lookErr != nil {
		t.Skip("conftest not found; skipping policy test")
	}
	cmd := exec.Command("conftest", "test", planJSON, "--policy", policyPath)
	cmd.Env = append(os.Environ(), "DIGITALOCEAN_TOKEN="+token)
	out, err := cmd.CombinedOutput()
        require.NoErrorf(t, err, "conftest failed: %s", string(out))
}

// testInvalidNodePoolConfig plans the dev cluster with the provided node pool
// definitions and asserts that validation fails.
//
// Example:
//
// testInvalidNodePoolConfig(t, []map[string]interface{}{
//     {
//             "name":       "default",
//             "size":       "s-2vcpu-2gb",
//             "node_count": 1,
//             "auto_scale": false,
//             "min_nodes":  1,
//             "max_nodes":  1,
//     },
// })
func testInvalidNodePoolConfig(t *testing.T, invalidNodePools []map[string]interface{}) {
        t.Parallel()
        vars := testVars(t)
        vars["node_pools"] = invalidNodePools
        _, opts := testutil.SetupTerraform(t, "../../..", "clusters/dev", vars, map[string]string{})
        _, err := terraform.InitAndPlanE(t, opts)
        require.Error(t, err)
}

func TestDevClusterInvalidNodePool(t *testing.T) {
        testInvalidNodePoolConfig(t, []map[string]interface{}{
                {
                        "name":       "default",
                        "size":       "s-2vcpu-2gb",
                        "node_count": 1,
                        "auto_scale": false,
                        "min_nodes":  1,
                        "max_nodes":  1,
                },
        })
}

func TestDevClusterAutoScaleMinExceedsCount(t *testing.T) {
        testInvalidNodePoolConfig(t, []map[string]interface{}{
                {
                        "name":       "default",
                        "size":       "s-2vcpu-2gb",
                        "node_count": 2,
                        "auto_scale": true,
                        "min_nodes":  3,
                        "max_nodes":  5,
                },
        })
}
