package tests

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/gruntwork-io/terratest/modules/terraform"
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

func TestDoksModuleValidate(t *testing.T) {
	t.Parallel()

	terraformOptions := &terraform.Options{
		TerraformDir:    filepath.Join("..", "examples", "basic"),
		TerraformBinary: "tofu",
		Vars:            testVars(),
		EnvVars: map[string]string{
			"DIGITALOCEAN_TOKEN": "dummy",
		},
		NoColor: true,
	}

	terraform.InitAndValidate(t, terraformOptions)
}

func TestDoksModulePlanUnauthenticated(t *testing.T) {
	t.Parallel()

	terraformOptions := &terraform.Options{
		TerraformDir:    filepath.Join("..", "examples", "basic"),
		TerraformBinary: "tofu",
		Vars:            testVars(),
		NoColor:         true,
	}

	_, err := terraform.InitAndPlanE(t, terraformOptions)
	if os.Getenv("DIGITALOCEAN_TOKEN") == "" {
		if err == nil {
			t.Skip("plan succeeded without token")
		}
		require.Contains(t, err.Error(), "DIGITALOCEAN_TOKEN", "error message should mention missing DIGITALOCEAN_TOKEN")
	}
}

func TestDoksModuleApplyIfTokenPresent(t *testing.T) {
	if os.Getenv("DIGITALOCEAN_TOKEN") == "" {
		t.Skip("DIGITALOCEAN_TOKEN not set; skipping apply test")
	}

	terraformOptions := &terraform.Options{
		TerraformDir:    filepath.Join("..", "examples", "basic"),
		TerraformBinary: "tofu",
		Vars:            testVars(),
		NoColor:         true,
	}

	defer terraform.Destroy(t, terraformOptions)
	terraform.InitAndApply(t, terraformOptions)

	clusterID := terraform.Output(t, terraformOptions, "cluster_id")
	endpoint := terraform.Output(t, terraformOptions, "endpoint")
	kubeconfig := terraform.Output(t, terraformOptions, "kubeconfig")

	if clusterID == "" {
		t.Errorf("expected cluster_id output to be set, got empty string")
	}
	if endpoint == "" {
		t.Errorf("expected endpoint output to be set, got empty string")
	}
	if kubeconfig == "" {
		t.Errorf("expected kubeconfig output to be set, got empty string")
	}
}

func TestDoksModuleInvalidInputs(t *testing.T) {
	t.Parallel()

	cases := map[string]map[string]interface{}{
		"EmptyClusterName": {
			"cluster_name":       "",
			"region":             "nyc1",
			"kubernetes_version": "1.28.0-do.0",
			"node_pools":         testVars()["node_pools"],
		},
		"InvalidRegion": {
			"cluster_name":       "terratest-cluster",
			"region":             "invalid",
			"kubernetes_version": "1.28.0-do.0",
			"node_pools":         testVars()["node_pools"],
		},
		"InvalidKubernetesVersion": {
			"cluster_name":       "terratest-cluster",
			"region":             "nyc1",
			"kubernetes_version": "1.28",
			"node_pools":         testVars()["node_pools"],
		},
		"EmptyNodePools": {
			"cluster_name":       "terratest-cluster",
			"region":             "nyc1",
			"kubernetes_version": "1.28.0-do.0",
			"node_pools":         []map[string]interface{}{},
		},
		"ZeroNodes": {
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
	}

	for name, vars := range cases {
		t.Run(name, func(t *testing.T) {
			opts := &terraform.Options{
				TerraformDir:    filepath.Join("..", "examples", "basic"),
				TerraformBinary: "tofu",
				Vars:            vars,
				EnvVars: map[string]string{
					"DIGITALOCEAN_TOKEN": "dummy",
				},
				NoColor: true,
			}

			_, err := terraform.InitAndPlanE(t, opts)
			require.Error(t, err)
		})
	}
}
