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

    for name, v := range cases {
        t.Run(name, func(t *testing.T) {
            _, opts := setupTerraform(t, v, map[string]string{"DIGITALOCEAN_TOKEN": "dummy"})
            _, err := terraform.InitAndPlanE(t, opts)
            require.Error(t, err)
        })
    }
}

