package tests

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"

	"github.com/gruntwork-io/terratest/modules/random"
	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
	testutil "wildside/infra/testutil"
)

func testVars(t *testing.T) map[string]interface{} {
	t.Helper()
	kubeconfigDir := t.TempDir()
	kubeconfigPath := filepath.Join(kubeconfigDir, "kubeconfig")
	stubConfig := []byte("apiVersion: v1\nkind: Config\nclusters: []\ncontexts: []\ncurrent-context: \"\"\nusers: []\n")
	require.NoError(t, os.WriteFile(kubeconfigPath, stubConfig, 0600))

	return map[string]interface{}{
		"namespace":                  "flux-system",
		"git_repository_name":        "flux-system",
		"kustomization_name":         "flux-system",
		"git_repository_url":         "https://github.com/fluxcd/flux2-kustomize-helm-example.git",
		"git_repository_branch":      "main",
		"git_repository_path":        "./clusters/my-cluster",
		"reconcile_interval":         "1m",
		"kustomization_prune":        true,
		"kustomization_suspend":      false,
		"git_repository_secret_name": nil,
		"kubeconfig_path":            kubeconfigPath,
	}
}

func setup(t *testing.T, vars map[string]interface{}) (string, *terraform.Options) {
	t.Helper()
	return testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "..",
		TfSubDir:      "examples/basic",
		Vars:          vars,
		EnvVars:       map[string]string{},
	})
}

func TestFluxModuleValidate(t *testing.T) {
	t.Parallel()
	_, opts := setup(t, testVars(t))
	terraform.InitAndValidate(t, opts)
}

func TestFluxModuleInvalidURL(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["git_repository_url"] = "ftp://invalid"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`git_repository_url`), err.Error())
}

func TestFluxModuleInvalidPath(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["git_repository_path"] = "/absolute/path"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
    require.Regexp(t, regexp.MustCompile(`git_repository_path must be a non-empty relative path without traversal`), err.Error())
}

func TestFluxModuleInvalidBranch(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["git_repository_branch"] = ""
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`git_repository_branch must not be blank`), err.Error())
}

func TestFluxModulePlanFailsWithoutKubeconfig(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["kubeconfig_path"] = "/nonexistent/kubeconfig"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	lowered := strings.ToLower(err.Error())
	require.Truef(t, strings.Contains(lowered, "no such file") || strings.Contains(lowered, "stat"), "expected missing kubeconfig error, got %q", err.Error())
}

func TestFluxModulePlanDetailedExitCode(t *testing.T) {
	t.Parallel()
	kubeconfig := os.Getenv("KUBECONFIG")
	if kubeconfig == "" {
		t.Skip("KUBECONFIG not set; skipping detailed exit code plan")
	}
	vars := testVars(t)
	vars["namespace"] = fmt.Sprintf("flux-terratest-%s", strings.ToLower(random.UniqueId()))
	vars["git_repository_name"] = fmt.Sprintf("flux-repo-%s", strings.ToLower(random.UniqueId()))
	vars["kustomization_name"] = fmt.Sprintf("flux-kustomization-%s", strings.ToLower(random.UniqueId()))
	vars["kubeconfig_path"] = kubeconfig
	tfDir, opts := setup(t, vars)
	terraform.Init(t, opts)
	cmd := exec.Command("tofu", "plan", "-input=false", "-no-color", "-detailed-exitcode")
	cmd.Dir = tfDir
	cmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1")
	err := cmd.Run()
	if err == nil {
		t.Fatalf("expected exit code 2 indicating changes, got 0")
	}
	exitErr, ok := err.(*exec.ExitError)
	require.True(t, ok, "expected ExitError")
	require.Equal(t, 2, exitErr.ExitCode())
}

func TestFluxModulePolicy(t *testing.T) {
	t.Parallel()
	if _, err := exec.LookPath("conftest"); err != nil {
		t.Skip("conftest not found; skipping policy test")
	}
	kubeconfig := os.Getenv("KUBECONFIG")
	if kubeconfig == "" {
		t.Skip("KUBECONFIG not set; skipping policy test")
	}
	vars := testVars(t)
	vars["namespace"] = fmt.Sprintf("flux-terratest-%s", strings.ToLower(random.UniqueId()))
	vars["git_repository_name"] = fmt.Sprintf("flux-repo-%s", strings.ToLower(random.UniqueId()))
	vars["kustomization_name"] = fmt.Sprintf("flux-kustomization-%s", strings.ToLower(random.UniqueId()))
	vars["kubeconfig_path"] = kubeconfig
	tfDir, opts := setup(t, vars)
	planFile := filepath.Join(tfDir, "tfplan.binary")
	opts.PlanFilePath = planFile
	terraform.InitAndPlan(t, opts)
	t.Cleanup(func() { _ = os.Remove(planFile) })

	show, err := terraform.RunTerraformCommandE(t, opts, "show", "-json", planFile)
	require.NoError(t, err)
	jsonPath := filepath.Join(tfDir, "plan.json")
	require.NoError(t, os.WriteFile(jsonPath, []byte(show), 0600))
	t.Cleanup(func() { _ = os.Remove(jsonPath) })

	policyPath := filepath.Join(tfDir, "..", "policy")
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, "conftest", "test", jsonPath, "--policy", policyPath)
	cmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1", "KUBECONFIG="+kubeconfig)
	out, err := cmd.CombinedOutput()
	require.NotEqual(t, context.DeadlineExceeded, ctx.Err(), "conftest timed out")
	require.NoErrorf(t, err, "conftest failed: %s", string(out))

	t.Run("PolicyViolation", func(t *testing.T) {
		t.Parallel()
		badPlan, err := os.CreateTemp("", "fluxcd-bad-plan-*.json")
		require.NoError(t, err)
		t.Cleanup(func() { _ = os.Remove(badPlan.Name()) })

		payload := `{"resource_changes":[{"type":"kubernetes_manifest","change":{"after":{"manifest":{"kind":"GitRepository","metadata":{"name":"invalid"},"spec":{"url":"ftp://invalid","interval":"15m"}}}}}]}`
		_, err = badPlan.WriteString(payload)
		require.NoError(t, err)
		require.NoError(t, badPlan.Close())

		vCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		violationCmd := exec.CommandContext(vCtx, "conftest", "test", badPlan.Name(), "--policy", policyPath)
		violationCmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1", "KUBECONFIG="+kubeconfig)
		violationOut, violationErr := violationCmd.CombinedOutput()
		require.NotEqual(t, context.DeadlineExceeded, vCtx.Err(), "negative policy run timed out")
		require.Error(t, violationErr, "expected conftest to report a violation")
		exitErr, ok := violationErr.(*exec.ExitError)
		require.True(t, ok, "expected ExitError from conftest for policy violation")
		require.NotZero(t, exitErr.ExitCode(), "expected non-zero exit code for policy violation")
		require.Contains(t, strings.ToLower(string(violationOut)), "gitrepository", "expected policy violation output")
	})
}

func TestFluxModuleApplyIfKubeconfigPresent(t *testing.T) {
	kubeconfig := os.Getenv("KUBECONFIG")
	if kubeconfig == "" {
		t.Skip("KUBECONFIG not set; skipping apply test")
	}
	if os.Getenv("FLUXCD_ACCEPT_APPLY") == "" {
		t.Skip("FLUXCD_ACCEPT_APPLY not set; skipping apply test")
	}
	vars := testVars(t)
	vars["namespace"] = fmt.Sprintf("flux-terratest-%s", strings.ToLower(random.UniqueId()))
	vars["git_repository_name"] = fmt.Sprintf("flux-repo-%s", strings.ToLower(random.UniqueId()))
	vars["kustomization_name"] = fmt.Sprintf("flux-kustomization-%s", strings.ToLower(random.UniqueId()))
	vars["kubeconfig_path"] = kubeconfig
	_, opts := setup(t, vars)
	t.Cleanup(func() {
		terraform.Destroy(t, opts)
	})
	terraform.InitAndApply(t, opts)

	namespace := terraform.Output(t, opts, "namespace")
	gitRepo := terraform.Output(t, opts, "git_repository_name")
	kustomization := terraform.Output(t, opts, "kustomization_name")

	require.NotEmpty(t, namespace)
	require.NotEmpty(t, gitRepo)
	require.NotEmpty(t, kustomization)

	stdout, err := terraform.RunTerraformCommandAndGetStdoutE(t, opts, "state", "show", fmt.Sprintf("kubernetes_manifest.git_repository"))
	require.NoError(t, err)
	require.Contains(t, stdout, vars["git_repository_url"].(string))
}
