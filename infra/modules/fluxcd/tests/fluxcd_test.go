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
	stubConfig := []byte(`apiVersion: v1
clusters:
- cluster:
    insecure-skip-tls-verify: true
    server: https://127.0.0.1
  name: stub
contexts:
- context:
    cluster: stub
    user: stub
  name: stub
current-context: stub
kind: Config
users:
- name: stub
  user:
    token: fake-token
`)
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
		"helm_values":                []string{},
		"helm_values_files":          []string{},
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

func TestFluxExampleRequiresKubeconfigPath(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["kubeconfig_path"] = ""
	_, opts := setup(t, vars)
	stdout, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	combined := strings.Join([]string{stdout, err.Error()}, "\n")
	require.Contains(t, combined, "Set kubeconfig_path to a readable kubeconfig file before running the example")
	require.Contains(t, combined, "Check block assertion failed")
}

func TestFluxModuleInvalidPath(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["git_repository_path"] = "/absolute/path"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`git_repository_path must be a non-empty relative path without\s+parent-directory traversal`), err.Error())
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

func TestFluxModuleSupportsHelmValues(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	valuesDir := t.TempDir()
	valuesFile := filepath.Join(valuesDir, "values.yaml")
	fileContent := "featureFlags:\n  tracing: true\n"
	require.NoError(t, os.WriteFile(valuesFile, []byte(fileContent), 0600))
	vars["helm_values"] = []string{"installCRDs: true"}
	vars["helm_values_files"] = []string{valuesFile}
	_, opts := setup(t, vars)
	stdout, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Contains(t, stdout, "installCRDs: true")
	require.Contains(t, stdout, "featureFlags")
	require.Contains(t, stdout, "tracing: true")
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

	t.Run("AllowsFileSchemeWhenOptedIn", func(t *testing.T) {
		t.Parallel()
		plan, err := os.CreateTemp("", "fluxcd-file-plan-*.json")
		require.NoError(t, err)
		t.Cleanup(func() { _ = os.Remove(plan.Name()) })

		payload := `{"resource_changes":[{"type":"kubernetes_manifest","change":{"after":{"manifest":{"kind":"GitRepository","metadata":{"name":"file"},"spec":{"url":"file:///tmp/repo","interval":"1m","ref":{"branch":"main"}}}}}}]}`
		_, err = plan.WriteString(payload)
		require.NoError(t, err)
		require.NoError(t, plan.Close())

		vCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		violationCmd := exec.CommandContext(vCtx, "conftest", "test", plan.Name(), "--policy", policyPath)
		violationCmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1", "KUBECONFIG="+kubeconfig)
		violationOut, violationErr := violationCmd.CombinedOutput()
		require.NotEqual(t, context.DeadlineExceeded, vCtx.Err(), "file-scheme policy run timed out")
		require.Error(t, violationErr, "expected conftest to report a violation when file:// is disallowed")
		exitErr, ok := violationErr.(*exec.ExitError)
		require.True(t, ok, "expected ExitError from conftest for file-scheme violation")
		require.NotZero(t, exitErr.ExitCode(), "expected non-zero exit code for file-scheme violation")
		require.Contains(t, strings.ToLower(string(violationOut)), "gitrepository", "expected git repository policy violation output")

		dataFile, err := os.CreateTemp("", "fluxcd-policy-data-*.json")
		require.NoError(t, err)
		t.Cleanup(func() { _ = os.Remove(dataFile.Name()) })
		_, err = dataFile.WriteString(`{"allow_file_scheme":true}`)
		require.NoError(t, err)
		require.NoError(t, dataFile.Close())

		allowCtx, allowCancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer allowCancel()
		allowCmd := exec.CommandContext(allowCtx, "conftest", "test", plan.Name(), "--policy", policyPath, "--data", dataFile.Name())
		allowCmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1", "KUBECONFIG="+kubeconfig)
		allowOut, allowErr := allowCmd.CombinedOutput()
		require.NotEqual(t, context.DeadlineExceeded, allowCtx.Err(), "allow-file policy run timed out")
		require.NoErrorf(t, allowErr, "conftest unexpectedly failed when allow_file_scheme=true: %s", string(allowOut))
	})

	t.Run("RejectsParentDirectoryTraversal", func(t *testing.T) {
		t.Parallel()
		plan, err := os.CreateTemp("", "fluxcd-path-plan-*.json")
		require.NoError(t, err)
		t.Cleanup(func() { _ = os.Remove(plan.Name()) })

		payload := `{"resource_changes":[{"type":"kubernetes_manifest","change":{"after":{"manifest":{"kind":"Kustomization","metadata":{"name":"invalid-path"},"spec":{"path":"../hack","prune":true,"suspend":false,"sourceRef":{"kind":"GitRepository"}}}}}}]}`
		_, err = plan.WriteString(payload)
		require.NoError(t, err)
		require.NoError(t, plan.Close())

		vCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		violationCmd := exec.CommandContext(vCtx, "conftest", "test", plan.Name(), "--policy", policyPath)
		violationCmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1", "KUBECONFIG="+kubeconfig)
		violationOut, violationErr := violationCmd.CombinedOutput()
		require.NotEqual(t, context.DeadlineExceeded, vCtx.Err(), "path policy run timed out")
		require.Error(t, violationErr, "expected conftest to report a violation for parent traversal")
		exitErr, ok := violationErr.(*exec.ExitError)
		require.True(t, ok, "expected ExitError from conftest for parent traversal")
		require.NotZero(t, exitErr.ExitCode(), "expected non-zero exit code for parent traversal violation")
		require.Contains(t, strings.ToLower(string(violationOut)), "kustomization", "expected kustomization path violation output")
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
