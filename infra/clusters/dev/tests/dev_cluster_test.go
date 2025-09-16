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
	vars := map[string]interface{}{
		"should_create_cluster": true,
		"cluster_name":          "wildside-dev",
		"region":                "nyc1",
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

	if version := testutil.KubernetesVersion(); version != "" {
		vars["kubernetes_version"] = version
	}

	return vars
}

func TestDevClusterValidate(t *testing.T) {
	t.Parallel()
	_, opts := testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "../../..",
		TfSubDir:      "clusters/dev",
		Vars:          testVars(t),
		EnvVars:       map[string]string{},
	})
	terraform.InitAndValidate(t, opts)
}

func TestDevClusterPlanUnauthenticated(t *testing.T) {
	t.Parallel()
	if os.Getenv("DIGITALOCEAN_TOKEN") != "" {
		t.Skip("DIGITALOCEAN_TOKEN set; skipping unauthenticated plan")
	}
	// The DigitalOcean provider does not require authentication at plan time,
	// so an unauthenticated plan should succeed.
	_, opts := testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "../../..",
		TfSubDir:      "clusters/dev",
		Vars:          testVars(t),
		EnvVars:       map[string]string{},
	})
	_, err := terraform.InitAndPlanE(t, opts)
	require.NoError(t, err)
}

func TestDevClusterPlanDetailedExitCode(t *testing.T) {
	t.Parallel()
	token := os.Getenv("DIGITALOCEAN_TOKEN")
	if token == "" {
		t.Skip("DIGITALOCEAN_TOKEN not set; skipping detailed exit code plan")
	}
	tfDir, opts := testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "../../..",
		TfSubDir:      "clusters/dev",
		Vars:          testVars(t),
		EnvVars:       map[string]string{"DIGITALOCEAN_TOKEN": token},
	})
	terraform.Init(t, opts)
	cmd := exec.Command("tofu", "plan", "-input=false", "-no-color", "-detailed-exitcode")
	cmd.Dir = tfDir
	cmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1", "DIGITALOCEAN_TOKEN="+token)
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
	tfDir, opts := testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "../../..",
		TfSubDir:      "clusters/dev",
		Vars:          testVars(t),
		EnvVars:       map[string]string{"DIGITALOCEAN_TOKEN": token},
	})
	if _, err := exec.LookPath("conftest"); err != nil {
		t.Skip("conftest not found; skipping policy test")
	}
	planFile := filepath.Join(tfDir, "tfplan.binary")
	opts.PlanFilePath = planFile
	terraform.InitAndPlan(t, opts)
	t.Cleanup(func() { _ = os.Remove(planFile) })

	show, err := terraform.RunTerraformCommandE(t, opts, "show", "-json", planFile)
	require.NoError(t, err)
	planJSON := filepath.Join(tfDir, "plan.json")
	require.NoError(t, os.WriteFile(planJSON, []byte(show), 0600))
	t.Cleanup(func() { _ = os.Remove(planJSON) })
	policyPath, err := filepath.Abs(filepath.Join(tfDir, "..", "..", "modules", "doks", "policy"))
	require.NoError(t, err)
	cmd := exec.Command("conftest", "test", planJSON, "--policy", policyPath)
	cmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1", "DIGITALOCEAN_TOKEN="+token)
	out, err := cmd.CombinedOutput()
	require.NoErrorf(t, err, "conftest failed: %s", string(out))
}

// testInvalidNodePoolConfig plans the dev cluster with the provided node pool
// definitions and asserts that validation fails.
//
// Example:
//
//	testInvalidNodePoolConfig(t, []map[string]interface{}{
//	    {
//	            "name":       "default",
//	            "size":       "s-2vcpu-2gb",
//	            "node_count": 1,
//	            "auto_scale": false,
//	            "min_nodes":  1,
//	            "max_nodes":  1,
//	    },
//	})
func testInvalidNodePoolConfig(t *testing.T, invalidNodePools []map[string]interface{}, want ...string) {
        t.Helper()
        t.Parallel()
        vars := testVars(t)
        vars["node_pools"] = invalidNodePools
        _, opts := testutil.SetupTerraform(t, testutil.TerraformConfig{
                SourceRootRel: "../../..",
                TfSubDir:      "clusters/dev",
                Vars:          vars,
                EnvVars:       map[string]string{},
        })
        _, err := terraform.InitAndPlanE(t, opts)
        require.Error(t, err)
        for _, s := range append([]string{"node_pools"}, want...) {
                require.ErrorContains(t, err, s)
        }
}

func TestDevClusterInvalidNodePools(t *testing.T) {
        cases := map[string]struct {
                Pools []map[string]interface{}
                Want  []string
        }{
                "InvalidNodePool": {
                        Pools: []map[string]interface{}{
                                {
                                        "name":       "default",
                                        "size":       "s-2vcpu-2gb",
                                        "node_count": 1,
                                        "auto_scale": false,
                                        "min_nodes":  1,
                                        "max_nodes":  1,
                                },
                        },
                        Want: []string{"node_count", "at least 2 nodes"},
                },
                "AutoScaleMinExceedsCount": {
                        Pools: []map[string]interface{}{
                                {
                                        "name":       "default",
                                        "size":       "s-2vcpu-2gb",
                                        "node_count": 2,
                                        "auto_scale": true,
                                        "min_nodes":  3,
                                        "max_nodes":  5,
                                },
                        },
                        Want: []string{"auto_scale", "min_nodes"},
                },
        }
        for name, tc := range cases {
                t.Run(name, func(t *testing.T) {
                        testInvalidNodePoolConfig(t, tc.Pools, tc.Want...)
                })
        }
}
