package tests

import (
    "os"
    "path/filepath"
    "testing"

    "github.com/gruntwork-io/terratest/modules/terraform"
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
        TerraformDir: filepath.Join("..", "examples", "basic"),
        TerraformBinary: "tofu",
        Vars:           testVars(),
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
        TerraformDir: filepath.Join("..", "examples", "basic"),
        TerraformBinary: "tofu",
        Vars:           testVars(),
        NoColor:       true,
    }

    _, err := terraform.InitAndPlanE(t, terraformOptions)
    if os.Getenv("DIGITALOCEAN_TOKEN") == "" && err == nil {
        t.Skip("plan succeeded without token")
    }
}

func TestDoksModuleApplyIfTokenPresent(t *testing.T) {
    if os.Getenv("DIGITALOCEAN_TOKEN") == "" {
        t.Skip("DIGITALOCEAN_TOKEN not set; skipping apply test")
    }

    terraformOptions := &terraform.Options{
        TerraformDir: filepath.Join("..", "examples", "basic"),
        TerraformBinary: "tofu",
        Vars:           testVars(),
        NoColor:       true,
    }

    defer terraform.Destroy(t, terraformOptions)
    terraform.InitAndApply(t, terraformOptions)
}
