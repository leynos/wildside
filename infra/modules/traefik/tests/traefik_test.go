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

const exampleKubeconfigError = "Set kubeconfig_path to a readable kubeconfig file before running the example"

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
		"namespace":                          "traefik",
		"acme_email":                         "admin@example.test",
		"cloudflare_api_token_secret_name":   "cloudflare-api-token",
		"cloudflare_api_token_secret_key":    "token",
		"cluster_issuer_name":                "letsencrypt-prod",
		"acme_server":                        "https://acme-v02.api.letsencrypt.org/directory",
		"dashboard_enabled":                  false,
		"http_to_https_redirect":             true,
		"kubeconfig_path":                    kubeconfigPath,
		"helm_values":                        []string{},
		"helm_values_files":                  []string{},
	}
}

func setup(t *testing.T, vars map[string]interface{}) (string, *terraform.Options) {
	t.Helper()
	return testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "..",
		TfSubDir:      "examples/basic",
		Vars:          vars,
		EnvVars:       testutil.TerraformEnvVars(nil),
	})
}

func testKubeconfigPathValidation(t *testing.T, kubeconfigPath string) {
	t.Helper()
	vars := testVars(t)
	vars["kubeconfig_path"] = kubeconfigPath
	_, opts := setup(t, vars)
	stdout, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	combined := strings.Join([]string{stdout, err.Error()}, "\n")
	require.Contains(t, combined, exampleKubeconfigError)
}

func requireBinary(t *testing.T, binary, skipMessage string) {
	t.Helper()
	if _, err := exec.LookPath(binary); err != nil {
		t.Skip(skipMessage)
	}
}

func requireEnvVar(t *testing.T, key, skipMessage string) string {
	t.Helper()
	value := os.Getenv(key)
	if value == "" {
		t.Skip(skipMessage)
	}
	return value
}

func renderTraefikPlan(t *testing.T, vars map[string]interface{}) (string, string) {
	t.Helper()
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

	return tfDir, jsonPath
}

type conftestRun struct {
	PlanPath   string
	PolicyPath string
	Kubeconfig string
	ExtraArgs  []string
	Timeout    time.Duration
}

func runConftestAgainstPlan(t *testing.T, cfg conftestRun) ([]byte, error) {
	t.Helper()
	timeout := cfg.Timeout
	if timeout == 0 {
		timeout = 60 * time.Second
	}
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()

	args := append([]string{"test", cfg.PlanPath, "--policy", cfg.PolicyPath}, cfg.ExtraArgs...)
	cmd := exec.CommandContext(ctx, "conftest", args...)
	cmd.Env = testutil.TerraformEnv(t, map[string]string{
		"KUBECONFIG": cfg.Kubeconfig,
	})
	out, err := cmd.CombinedOutput()
	require.NotEqual(t, context.DeadlineExceeded, ctx.Err(), "conftest timed out")
	return out, err
}

func writePlanFixture(t *testing.T, payload string) string {
	t.Helper()
	plan, err := os.CreateTemp("", "traefik-plan-*.json")
	require.NoError(t, err)
	t.Cleanup(func() { _ = os.Remove(plan.Name()) })

	_, err = plan.WriteString(payload)
	require.NoError(t, err)
	require.NoError(t, plan.Close())

	return plan.Name()
}

func TestTraefikModuleValidate(t *testing.T) {
	t.Parallel()
	_, opts := setup(t, testVars(t))
	terraform.InitAndValidate(t, opts)
}

func TestTraefikModuleInvalidEmail(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["acme_email"] = "not-an-email"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`acme_email`), err.Error())
}

func TestTraefikModuleInvalidChartVersion(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["chart_version"] = "invalid"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`chart_version`), err.Error())
}

func TestTraefikModuleInvalidNamespace(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["namespace"] = "Invalid_Namespace"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`namespace`), err.Error())
}

func TestTraefikModuleInvalidACMEServer(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["acme_server"] = "http://invalid.example.com"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Regexp(t, regexp.MustCompile(`acme_server`), err.Error())
}

func TestTraefikExampleRequiresKubeconfigPath(t *testing.T) {
	t.Parallel()
	testKubeconfigPathValidation(t, "")
}

func TestTraefikExampleRejectsWhitespaceKubeconfig(t *testing.T) {
	t.Parallel()
	testKubeconfigPathValidation(t, " \n\t ")
}

func TestTraefikModulePlanFailsWithoutKubeconfig(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	vars["kubeconfig_path"] = "/nonexistent/kubeconfig"
	_, opts := setup(t, vars)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	lowered := strings.ToLower(err.Error())
	require.Truef(t, strings.Contains(lowered, "no such file") || strings.Contains(lowered, "stat"),
		"expected missing kubeconfig error, got %q", err.Error())
}

func TestTraefikModulePlanDetailedExitCode(t *testing.T) {
	t.Parallel()
	requireBinary(t, "tofu", "tofu not found; skipping detailed exit code plan")
	kubeconfig := os.Getenv("KUBECONFIG")
	if kubeconfig == "" {
		t.Skip("KUBECONFIG not set; skipping detailed exit code plan")
	}
	vars := testVars(t)
	vars["namespace"] = fmt.Sprintf("traefik-terratest-%s", strings.ToLower(random.UniqueId()))
	vars["cluster_issuer_name"] = fmt.Sprintf("issuer-%s", strings.ToLower(random.UniqueId()))
	vars["kubeconfig_path"] = kubeconfig
	tfDir, opts := setup(t, vars)
	terraform.Init(t, opts)
	cmd := exec.Command("tofu", "plan", "-input=false", "-no-color", "-detailed-exitcode")
	cmd.Dir = tfDir
	cmd.Env = testutil.TerraformEnv(t, nil)
	err := cmd.Run()
	if err == nil {
		t.Fatalf("expected exit code 2 indicating changes, got 0")
	}
	exitErr, ok := err.(*exec.ExitError)
	require.True(t, ok, "expected ExitError")
	require.Equal(t, 2, exitErr.ExitCode())
}

func TestTraefikModuleSupportsHelmValues(t *testing.T) {
	t.Parallel()
	vars := testVars(t)
	valuesDir := t.TempDir()
	valuesFile := filepath.Join(valuesDir, "values.yaml")
	fileContent := "additionalArguments:\n  - --log.level=DEBUG\n"
	require.NoError(t, os.WriteFile(valuesFile, []byte(fileContent), 0600))
	vars["helm_values"] = []string{"resources:\n  requests:\n    cpu: 100m"}
	vars["helm_values_files"] = []string{valuesFile}
	_, opts := setup(t, vars)
	stdout, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Contains(t, stdout, "resources")
	require.Contains(t, stdout, "additionalArguments")
}

func TestTraefikModulePolicy(t *testing.T) {
	t.Parallel()
	requireBinary(t, "conftest", "conftest not found; skipping policy test")
	kubeconfig := requireEnvVar(t, "KUBECONFIG", "KUBECONFIG not set; skipping policy test")

	vars := testVars(t)
	vars["namespace"] = fmt.Sprintf("traefik-terratest-%s", strings.ToLower(random.UniqueId()))
	vars["cluster_issuer_name"] = fmt.Sprintf("issuer-%s", strings.ToLower(random.UniqueId()))
	vars["kubeconfig_path"] = kubeconfig

	tfDir, planJSON := renderTraefikPlan(t, vars)
	policyPath := filepath.Join(tfDir, "..", "policy")

	out, err := runConftestAgainstPlan(t, conftestRun{
		PlanPath:   planJSON,
		PolicyPath: policyPath,
		Kubeconfig: kubeconfig,
		Timeout:    60 * time.Second,
	})
	require.NoErrorf(t, err, "conftest failed: %s", string(out))

	t.Run("PolicyViolation_InvalidACMEServer", func(t *testing.T) {
		t.Parallel()
		payload := `{"resource_changes":[{"type":"kubernetes_manifest","change":{"after":{"manifest":{"kind":"ClusterIssuer","metadata":{"name":"invalid"},"spec":{"acme":{"server":"http://invalid","email":"","solvers":[]}}}}}}]}`
		planPath := writePlanFixture(t, payload)
		violationOut, violationErr := runConftestAgainstPlan(t, conftestRun{
			PlanPath:   planPath,
			PolicyPath: policyPath,
			Kubeconfig: kubeconfig,
			Timeout:    10 * time.Second,
		})
		require.Error(t, violationErr, "expected conftest to report a violation")
		exitErr, ok := violationErr.(*exec.ExitError)
		require.True(t, ok, "expected ExitError from conftest for policy violation")
		require.NotZero(t, exitErr.ExitCode(), "expected non-zero exit code for policy violation")
		require.Contains(t, strings.ToLower(string(violationOut)), "clusterissuer",
			"expected policy violation output")
	})

	t.Run("PolicyViolation_MissingEmail", func(t *testing.T) {
		t.Parallel()
		payload := `{"resource_changes":[{"type":"kubernetes_manifest","change":{"after":{"manifest":{"kind":"ClusterIssuer","metadata":{"name":"no-email"},"spec":{"acme":{"server":"https://acme.example.com","email":"","solvers":[{"dns01":{}}]}}}}}}]}`
		planPath := writePlanFixture(t, payload)
		violationOut, violationErr := runConftestAgainstPlan(t, conftestRun{
			PlanPath:   planPath,
			PolicyPath: policyPath,
			Kubeconfig: kubeconfig,
			Timeout:    10 * time.Second,
		})
		require.Error(t, violationErr, "expected conftest to report a violation for missing email")
		require.Contains(t, strings.ToLower(string(violationOut)), "email",
			"expected email violation output")
	})

	t.Run("PolicyViolation_NoSolvers", func(t *testing.T) {
		t.Parallel()
		payload := `{"resource_changes":[{"type":"kubernetes_manifest","change":{"after":{"manifest":{"kind":"ClusterIssuer","metadata":{"name":"no-solvers"},"spec":{"acme":{"server":"https://acme.example.com","email":"test@example.com","solvers":[]}}}}}}]}`
		planPath := writePlanFixture(t, payload)
		violationOut, violationErr := runConftestAgainstPlan(t, conftestRun{
			PlanPath:   planPath,
			PolicyPath: policyPath,
			Kubeconfig: kubeconfig,
			Timeout:    10 * time.Second,
		})
		require.Error(t, violationErr, "expected conftest to report a violation for no solvers")
		require.Contains(t, strings.ToLower(string(violationOut)), "solver",
			"expected solver violation output")
	})

	t.Run("PolicyWarn_StagingACME", func(t *testing.T) {
		t.Parallel()
		payload := `{"resource_changes":[{"type":"kubernetes_manifest","change":{"after":{"manifest":{"kind":"ClusterIssuer","metadata":{"name":"staging"},"spec":{"acme":{"server":"https://acme-staging-v02.api.letsencrypt.org/directory","email":"test@example.com","privateKeySecretRef":{"name":"staging"},"solvers":[{"dns01":{"cloudflare":{}}}]}}}}}}]}`
		planPath := writePlanFixture(t, payload)
		// Warnings don't cause conftest to fail, so we just check the output contains the warning
		warnOut, _ := runConftestAgainstPlan(t, conftestRun{
			PlanPath:   planPath,
			PolicyPath: policyPath,
			Kubeconfig: kubeconfig,
			Timeout:    10 * time.Second,
		})
		require.Contains(t, strings.ToLower(string(warnOut)), "staging",
			"expected staging warning in output")
	})
}

func TestTraefikModuleApplyIfKubeconfigPresent(t *testing.T) {
	kubeconfig := os.Getenv("KUBECONFIG")
	if kubeconfig == "" {
		t.Skip("KUBECONFIG not set; skipping apply test")
	}
	if os.Getenv("TRAEFIK_ACCEPT_APPLY") == "" {
		t.Skip("TRAEFIK_ACCEPT_APPLY not set; skipping apply test")
	}
	vars := testVars(t)
	vars["namespace"] = fmt.Sprintf("traefik-terratest-%s", strings.ToLower(random.UniqueId()))
	vars["cluster_issuer_name"] = fmt.Sprintf("issuer-%s", strings.ToLower(random.UniqueId()))
	vars["kubeconfig_path"] = kubeconfig
	_, opts := setup(t, vars)
	t.Cleanup(func() {
		terraform.Destroy(t, opts)
	})
	terraform.InitAndApply(t, opts)

	namespace := terraform.Output(t, opts, "namespace")
	helmRelease := terraform.Output(t, opts, "helm_release_name")
	clusterIssuer := terraform.Output(t, opts, "cluster_issuer_name")

	require.NotEmpty(t, namespace)
	require.NotEmpty(t, helmRelease)
	require.NotEmpty(t, clusterIssuer)

	stdout, err := terraform.RunTerraformCommandAndGetStdoutE(t, opts, "state", "show", "module.traefik.helm_release.traefik")
	require.NoError(t, err)
	require.Contains(t, stdout, "traefik")
}
