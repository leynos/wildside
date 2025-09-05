package tests

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/gruntwork-io/terratest/modules/random"
	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/gruntwork-io/terratest/modules/test-structure"
	"github.com/stretchr/testify/require"
)

func testVars() map[string]interface{} {
	return map[string]interface{}{
		"cluster_name":       "terratest-cluster",
		"region":             "nyc1",
		"kubernetes_version": "1.28.0-do.0",
		"node_pools": []map[string]interface{}{
			{
				"name":       "default",
				"size":       "s-2vcpu-2gb",
				"node_count": 1,
				"auto_scale": false,
				"min_nodes":  1,
				"max_nodes":  1,
			},
		},
	}
}

func setupTerraform(t *testing.T, vars map[string]interface{}, env map[string]string) (string, *terraform.Options) {
	tempRoot := test_structure.CopyTerraformFolderToTemp(t, "..", ".")
	tfDir := filepath.Join(tempRoot, "examples", "basic")
	opts := terraform.WithDefaultRetryableErrors(t, &terraform.Options{
		TerraformDir:    tfDir,
		TerraformBinary: "tofu",
		Vars:            vars,
		EnvVars:         env,
		NoColor:         true,
	})
	return tfDir, opts
}

func TestDoksModuleValidate(t *testing.T) {
	t.Parallel()

	vars := testVars()
	vars["cluster_name"] = fmt.Sprintf("terratest-%s", strings.ToLower(random.UniqueId()))
	_, opts := setupTerraform(t, vars, map[string]string{"DIGITALOCEAN_TOKEN": "dummy"})
	terraform.InitAndValidate(t, opts)
}

func TestDoksModulePlanUnauthenticated(t *testing.T) {
	t.Parallel()

	vars := testVars()
	vars["cluster_name"] = fmt.Sprintf("terratest-%s", strings.ToLower(random.UniqueId()))
	_, opts := setupTerraform(t, vars, map[string]string{})

	_, err := terraform.InitAndPlanE(t, opts)
	if os.Getenv("DIGITALOCEAN_TOKEN") == "" {
		if err == nil {
			t.Skip("plan succeeded without DIGITALOCEAN_TOKEN")
		}
		require.Error(t, err, "expected error when DIGITALOCEAN_TOKEN is missing")
		require.Contains(t, err.Error(), "DIGITALOCEAN_TOKEN", "error message should mention missing DIGITALOCEAN_TOKEN")
	} else {
		require.NoError(t, err)
	}
}

func TestDoksModuleApplyIfTokenPresent(t *testing.T) {
	token := os.Getenv("DIGITALOCEAN_TOKEN")
	if token == "" {
		t.Skip("DIGITALOCEAN_TOKEN not set; skipping apply test")
	}

	vars := testVars()
	vars["cluster_name"] = fmt.Sprintf("terratest-%s", strings.ToLower(random.UniqueId()))
	_, opts := setupTerraform(t, vars, map[string]string{"DIGITALOCEAN_TOKEN": token})

	defer terraform.Destroy(t, opts)
	terraform.InitAndApply(t, opts)

	clusterID := terraform.Output(t, opts, "cluster_id")
	endpoint := terraform.Output(t, opts, "endpoint")
	kubeconfig := terraform.Output(t, opts, "kubeconfig")

	require.NotEmpty(t, clusterID, "expected cluster_id output to be set")
	require.NotEmpty(t, endpoint, "expected endpoint output to be set")
	require.NotEmpty(t, kubeconfig, "expected kubeconfig output to be set")

}

func TestDoksModulePolicy(t *testing.T) {
	t.Parallel()

	vars := testVars()
	vars["cluster_name"] = fmt.Sprintf("terratest-%s", strings.ToLower(random.UniqueId()))
	tfDir, opts := setupTerraform(t, vars, map[string]string{"DIGITALOCEAN_TOKEN": "dummy"})

	planFile := filepath.Join(tfDir, "tfplan.binary")
	opts.PlanFilePath = planFile
	terraform.InitAndPlan(t, opts)

	show, err := terraform.RunTerraformCommandE(t, opts, "show", "-json", planFile)
	require.NoError(t, err)
	jsonPath := filepath.Join(tfDir, "plan.json")
	require.NoError(t, os.WriteFile(jsonPath, []byte(show), 0600))
	cmd := exec.Command("conftest", "test", jsonPath, "--policy", filepath.Join("..", "..", "policy"))
	cmd.Dir = tfDir
	output, err := cmd.CombinedOutput()
	require.NoErrorf(t, err, "conftest failed: %s", string(output))
}

func TestDoksModuleInvalidInputs(t *testing.T) {
	cases := map[string]struct {
		Vars        map[string]interface{}
		ErrContains string
	}{
		"EmptyClusterName": {
			Vars: map[string]interface{}{
				"cluster_name":       "",
				"region":             "nyc1",
				"kubernetes_version": "1.28.0-do.0",
				"node_pools":         testVars()["node_pools"],
			},
			ErrContains: "cluster_name must not be empty",
		},
		"InvalidRegion": {
			Vars: map[string]interface{}{
				"cluster_name":       "terratest-cluster",
				"region":             "invalid",
				"kubernetes_version": "1.28.0-do.0",
				"node_pools":         testVars()["node_pools"],
			},
			ErrContains: "region must be a valid DigitalOcean slug",
		},
		"InvalidKubernetesVersion": {
			Vars: map[string]interface{}{
				"cluster_name":       "terratest-cluster",
				"region":             "nyc1",
				"kubernetes_version": "1.28",
				"node_pools":         testVars()["node_pools"],
			},
			ErrContains: "kubernetes_version must match",
		},
		"EmptyNodePools": {
			Vars: map[string]interface{}{
				"cluster_name":       "terratest-cluster",
				"region":             "nyc1",
				"kubernetes_version": "1.28.0-do.0",
				"node_pools":         []map[string]interface{}{},
			},
			ErrContains: "each node pool requires at least one node",
		},
		"ZeroNodes": {
			Vars: map[string]interface{}{
				"cluster_name":       "terratest-cluster",
				"region":             "nyc1",
				"kubernetes_version": "1.28.0-do.0",
				"node_pools": []map[string]interface{}{
					{
						"name":       "default",
						"size":       "s-2vcpu-2gb",
						"node_count": 0,
						"auto_scale": false,
						"min_nodes":  0,
						"max_nodes":  0,
					},
				},
			},
			ErrContains: "each node pool requires at least one node",
		},
	}

	for name, tc := range cases {
		t.Run(name, func(t *testing.T) {
			_, opts := setupTerraform(t, tc.Vars, map[string]string{"DIGITALOCEAN_TOKEN": "dummy"})
			_, err := terraform.InitAndPlanE(t, opts)
			require.Error(t, err)
			require.Contains(t, err.Error(), tc.ErrContains)
		})
	}
}
