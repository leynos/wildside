package tests

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/gruntwork-io/terratest/modules/logger"
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
	_, err := terraform.InitAndValidateE(t, opts)
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

func TestDevClusterFluxRequiresRepositoryURL(t *testing.T) {
	t.Parallel()
	testInvalidFluxConfig(t, map[string]interface{}{
		"should_create_cluster":   false,
		"should_install_flux":     true,
		"flux_kubeconfig_path":    "/tmp/kubeconfig",
		"flux_git_repository_url": "",
	}, "flux_git_repository_url must be set to an HTTPS, SSH, git@, or file URL when installing Flux")
}

func TestDevClusterFluxRequiresCluster(t *testing.T) {
	t.Parallel()
	testInvalidFluxConfig(t, map[string]interface{}{
		"should_create_cluster": false,
		"should_install_flux":   true,
	}, "should_install_flux requires should_create_cluster to be true or flux_kubeconfig_path to be set")
}

func testInvalidConfig(t *testing.T, varModifications map[string]interface{}, wantErrSubstrings ...string) {
	t.Helper()
	vars := testVars(t)
	for key, value := range varModifications {
		vars[key] = value
	}
	_, opts := testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "../../..",
		TfSubDir:      "clusters/dev",
		Vars:          vars,
		EnvVars:       map[string]string{},
	})
	opts.Logger = logger.Discard
	terraform.Init(t, opts)
	planArgs := terraform.FormatArgs(opts, "plan", "-input=false")
	out, err := terraform.RunTerraformCommandE(t, opts, planArgs...)
	require.Error(t, err)
	combined := strings.Join([]string{out, err.Error()}, "\n")
	normalised := strings.Join(strings.Fields(combined), " ")
	for _, substring := range wantErrSubstrings {
		require.Contains(t, normalised, substring)
	}
}

func testInvalidFluxConfig(t *testing.T, varModifications map[string]interface{}, wantErrSubstrings ...string) {
	t.Helper()
	testInvalidConfig(t, varModifications, wantErrSubstrings...)
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
func testInvalidNodePoolConfig(t *testing.T, invalidNodePools []map[string]interface{}, wantErrSubstrings ...string) {
	t.Helper()
	allErrSubstrings := append([]string{"node_pools"}, wantErrSubstrings...)
	testInvalidConfig(t, map[string]interface{}{
		"node_pools": invalidNodePools,
	}, allErrSubstrings...)
}

func TestDevClusterInvalidNodePoolConfigs(t *testing.T) {
	cases := []struct {
		name              string
		nodePools         []map[string]interface{}
		wantErrSubstrings []string
	}{
		{
			name: "NodeCountBelowMinimum",
			nodePools: []map[string]interface{}{
				{
					"name":       "default",
					"size":       "s-2vcpu-2gb",
					"node_count": 1,
					"auto_scale": false,
					"min_nodes":  1,
					"max_nodes":  1,
				},
			},
			wantErrSubstrings: []string{"node_count"},
		},
		{
			name: "AutoScaleMinExceedsCount",
			nodePools: []map[string]interface{}{
				{
					"name":       "default",
					"size":       "s-2vcpu-2gb",
					"node_count": 2,
					"auto_scale": true,
					"min_nodes":  3,
					"max_nodes":  5,
				},
			},
			wantErrSubstrings: []string{"auto_scale", "min_nodes"},
		},
		{
			name: "AutoScaleMinBelowTwo",
			nodePools: []map[string]interface{}{
				{
					"name":       "default",
					"size":       "s-2vcpu-2gb",
					"node_count": 2,
					"auto_scale": true,
					"min_nodes":  1,
					"max_nodes":  5,
				},
			},
			wantErrSubstrings: []string{"min_nodes"},
		},
		{
			name: "MaxNodesBelowMinNodes",
			nodePools: []map[string]interface{}{
				{
					"name":       "default",
					"size":       "s-2vcpu-2gb",
					"node_count": 2,
					"auto_scale": true,
					"min_nodes":  5,
					"max_nodes":  4,
				},
			},
			wantErrSubstrings: []string{"max_nodes"},
		},
	}
	for _, tc := range cases {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			testInvalidNodePoolConfig(t, tc.nodePools, tc.wantErrSubstrings...)
		})
	}
}
