package tests

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/gruntwork-io/terratest/modules/random"
	"github.com/gruntwork-io/terratest/modules/terraform"
	"github.com/stretchr/testify/require"
	testutil "wildside/infra/testutil"
)

func baseVars(t *testing.T) map[string]interface{} {
	t.Helper()
	suffix := strings.ToLower(random.UniqueId())
	name := fmt.Sprintf("vault-%s", suffix)
	commonName := fmt.Sprintf("%s.example.test", name)
	return map[string]interface{}{
		"name":                    name,
		"region":                  "nyc1",
		"tags":                    []string{"terratest"},
		"ha_enabled":              false,
		"allowed_ssh_cidrs":       []string{"203.0.113.5/32"},
		"certificate_common_name": commonName,
		"certificate_dns_names":   []string{commonName},
		"certificate_ip_sans":     []string{},
		"recovery_shares":         5,
		"recovery_threshold":      3,
		"recovery_key_length":     32,
	}
}

func setupTerraform(t *testing.T, vars map[string]interface{}, env map[string]string) (string, *terraform.Options) {
	t.Helper()
	return testutil.SetupTerraform(t, testutil.TerraformConfig{
		SourceRootRel: "..",
		TfSubDir:      "examples/basic",
		Vars:          vars,
		EnvVars:       env,
	})
}

func requireBinary(t *testing.T, name, skipMessage string) {
	t.Helper()
	if _, err := exec.LookPath(name); err != nil {
		t.Skip(skipMessage)
	}
}

func renderPlanJSON(t *testing.T, vars map[string]interface{}) (string, string) {
	t.Helper()
	env := map[string]string{"DIGITALOCEAN_TOKEN": "dummy"}
	tfDir, opts := setupTerraform(t, vars, env)
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

func policyPath(t *testing.T) string {
	t.Helper()
	_, thisFile, _, ok := runtime.Caller(0)
	require.True(t, ok, "unable to resolve caller path")
	policyDir := filepath.Join(filepath.Dir(thisFile), "..", "policy")
	entries, err := os.ReadDir(policyDir)
	require.NoError(t, err, "policy directory not readable")
	hasRego := false
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".rego") {
			hasRego = true
			break
		}
	}
	require.True(t, hasRego, "policy directory contains no .rego files")
	return policyDir
}

func runConftestPolicyTest(t *testing.T, vars map[string]interface{}) ([]byte, error) {
	t.Helper()

	requireBinary(t, "conftest", "conftest not installed; skipping policy test")
	_, planJSON := renderPlanJSON(t, vars)
	return runConftestWithPlan(t, planJSON)
}

func runConftestWithPlan(t *testing.T, planJSON string) ([]byte, error) {
	t.Helper()

	policyDir := policyPath(t)

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, "conftest", "test", planJSON, "--policy", policyDir, "--namespace", "policy")
	cmd.Env = append(os.Environ(), "TF_IN_AUTOMATION=1")
	output, err := cmd.CombinedOutput()
	require.NotEqual(t, context.DeadlineExceeded, ctx.Err(), "conftest timed out")

	return output, err
}

func mutatePlanJSON(t *testing.T, planJSON string, mutate func(map[string]interface{})) string {
	t.Helper()

	data, err := os.ReadFile(planJSON)
	require.NoError(t, err, "failed to read plan JSON")

	var document map[string]interface{}
	require.NoError(t, json.Unmarshal(data, &document), "failed to decode plan JSON")

	mutate(document)

	mutatedPath := filepath.Join(t.TempDir(), filepath.Base(planJSON))
	mutated, err := json.MarshalIndent(document, "", "  ")
	require.NoError(t, err, "failed to encode mutated plan JSON")
	require.NoError(t, os.WriteFile(mutatedPath, mutated, 0600))

	return mutatedPath
}

func mutateLoadBalancerForwardingRules(t *testing.T, doc map[string]interface{}, mutate func(map[string]interface{})) {
	t.Helper()

	changes, ok := doc["resource_changes"].([]interface{})
	require.True(t, ok, "plan JSON missing resource_changes")

	for _, changeRaw := range changes {
		change, ok := changeRaw.(map[string]interface{})
		if !ok {
			continue
		}
		if change["type"] != "digitalocean_loadbalancer" {
			continue
		}
		delta, _ := change["change"].(map[string]interface{})
		if delta == nil {
			continue
		}
		after, _ := delta["after"].(map[string]interface{})
		if after == nil {
			continue
		}
		rules, _ := after["forwarding_rule"].([]interface{})
		for _, ruleRaw := range rules {
			rule, ok := ruleRaw.(map[string]interface{})
			if !ok {
				continue
			}
			mutate(rule)
		}
	}
}

func mutateFirewallInboundRules(t *testing.T, doc map[string]interface{}, mutate func(map[string]interface{})) {
	t.Helper()

	changes, ok := doc["resource_changes"].([]interface{})
	require.True(t, ok, "plan JSON missing resource_changes")

	for _, changeRaw := range changes {
		change, ok := changeRaw.(map[string]interface{})
		if !ok {
			continue
		}
		if change["type"] != "digitalocean_firewall" {
			continue
		}
		delta, _ := change["change"].(map[string]interface{})
		if delta == nil {
			continue
		}
		after, _ := delta["after"].(map[string]interface{})
		if after != nil {
			rules, _ := after["inbound_rule"].([]interface{})
			for _, ruleRaw := range rules {
				rule, ok := ruleRaw.(map[string]interface{})
				if !ok {
					continue
				}
				mutate(rule)
			}
		}

		unknown, _ := delta["after_unknown"].(map[string]interface{})
		if unknown != nil {
			if entries, exists := unknown["inbound_rule"]; exists {
				list, _ := entries.([]interface{})
				for _, entryRaw := range list {
					entry, ok := entryRaw.(map[string]interface{})
					if !ok {
						continue
					}
					mutate(entry)
				}
			}
		}
	}
}

func TestVaultApplianceModuleValidate(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	env := map[string]string{"DIGITALOCEAN_TOKEN": "dummy"}
	_, opts := setupTerraform(t, vars, env)
	terraform.InitAndValidate(t, opts)
}

func TestVaultApplianceModuleValidateMissingRequiredVars(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	delete(vars, "region")
	env := map[string]string{"DIGITALOCEAN_TOKEN": "dummy"}
	_, opts := setupTerraform(t, vars, env)
	err := terraform.InitAndValidateE(t, opts)
	require.Error(t, err, "validation should fail when required variable 'region' is missing")
}

func TestVaultApplianceModuleValidateInvalidOptionalVars(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	vars["load_balancer_algorithm"] = "invalid-algorithm"
	env := map[string]string{"DIGITALOCEAN_TOKEN": "dummy"}
	_, opts := setupTerraform(t, vars, env)
	err := terraform.InitAndValidateE(t, opts)
	require.Error(t, err, "validation should fail when optional variable 'load_balancer_algorithm' is invalid")
}

func TestVaultAppliancePlanUnauthenticated(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	env := map[string]string{"DIGITALOCEAN_TOKEN": ""}
	_, opts := setupTerraform(t, vars, env)

	_, err := terraform.InitAndPlanE(t, opts)
	if err == nil {
		_, err = terraform.ApplyE(t, opts)
	}
	require.Error(t, err, "expected unauthenticated plan/apply to fail")

	combined := err.Error()
	re := regexp.MustCompile(`(?i)(authentication|authenticate|token|unauthori[sz]ed|credentials)`)
	require.Truef(t, re.MatchString(combined), "error %q should reference authentication", combined)
}

func TestVaultAppliancePolicyPasses(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	output, err := runConftestPolicyTest(t, vars)
	require.NoErrorf(t, err, "conftest reported failure: %s", string(output))
}

func TestVaultAppliancePolicyRejectsOpenSSH(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	vars["allowed_ssh_cidrs"] = []string{"0.0.0.0/0"}

	output, err := runConftestPolicyTest(t, vars)
	require.Error(t, err, "expected conftest to reject public SSH")
	require.Contains(t, string(output), "must not expose SSH")
}

func TestVaultAppliancePolicyEnforcesHTTPS(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	_, planJSON := renderPlanJSON(t, vars)
	mutated := mutatePlanJSON(t, planJSON, func(doc map[string]interface{}) {
		mutateLoadBalancerForwardingRules(t, doc, func(rule map[string]interface{}) {
			rule["entry_protocol"] = "http"
			rule["entry_port"] = float64(80)
		})
	})

	output, err := runConftestWithPlan(t, mutated)
	require.Error(t, err, "expected conftest to reject HTTP-only load balancer rules")
	require.Contains(t, string(output), "must terminate HTTPS on port 443")
}

func TestVaultAppliancePolicyRedirectsHTTPToHTTPS(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	vars["load_balancer_redirect_http_to_https"] = false

	output, err := runConftestPolicyTest(t, vars)
	require.Error(t, err, "expected conftest to require HTTP to HTTPS redirection")
	require.Contains(t, string(output), "must redirect HTTP to HTTPS")
}

func TestVaultAppliancePolicyLoadBalancerFirewallRules(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	_, planJSON := renderPlanJSON(t, vars)
	mutated := mutatePlanJSON(t, planJSON, func(doc map[string]interface{}) {
		mutateFirewallInboundRules(t, doc, func(rule map[string]interface{}) {
			delete(rule, "source_load_balancer_uids")
		})
	})

	output, err := runConftestWithPlan(t, mutated)
	require.Error(t, err, "expected conftest to require load balancer firewall rules")
	require.Contains(t, string(output), "must allow traffic from the managed load balancer")
}

func TestVaultApplianceInvalidRecoveryThreshold(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	vars["recovery_shares"] = 2
	vars["recovery_threshold"] = 3
	env := map[string]string{"DIGITALOCEAN_TOKEN": "dummy"}
	_, opts := setupTerraform(t, vars, env)
	_, err := terraform.InitAndPlanE(t, opts)
	require.Error(t, err)
	require.Contains(t, err.Error(), "recovery_threshold cannot exceed recovery_shares")
}

func TestVaultApplianceHAPlanRendersTwoDroplets(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	vars["ha_enabled"] = true
	env := map[string]string{"DIGITALOCEAN_TOKEN": "dummy"}
	tfDir, opts := setupTerraform(t, vars, env)
	plan, err := terraform.InitAndPlanE(t, opts)
	require.NoError(t, err)
	require.Contains(t, plan, "digitalocean_droplet.vault[1]", "expected plan to create a second droplet in HA mode")
	t.Cleanup(func() { _ = os.Remove(filepath.Join(tfDir, "terraform.tfstate")) })
}

func TestVaultApplianceDetailedExitCode(t *testing.T) {
	t.Parallel()
	vars := baseVars(t)
	env := map[string]string{"DIGITALOCEAN_TOKEN": "dummy"}
	tfDir, opts := setupTerraform(t, vars, env)
	terraform.Init(t, opts)

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	args := terraform.FormatArgs(opts, "plan", "-detailed-exitcode")
	command := exec.CommandContext(ctx, "tofu", args...)
	command.Dir = opts.TerraformDir
	command.Env = append(os.Environ(), formatEnvVars(opts.EnvVars)...)
	command.Env = append(command.Env, "TF_IN_AUTOMATION=1")
	output, err := command.CombinedOutput()
	require.NotEqual(t, context.DeadlineExceeded, ctx.Err(), "terraform plan timed out")
	exitErr := &exec.ExitError{}
	require.ErrorAs(t, err, &exitErr, "expected detailed exit code to produce non-zero exit status")
	require.Equal(t, 2, exitErr.ExitCode(), "expected plan to return detailed exit code 2 when creating resources\nOutput: %s", string(output))
	t.Cleanup(func() { _ = os.Remove(filepath.Join(tfDir, "terraform.tfstate")) })
}

func formatEnvVars(env map[string]string) []string {
	items := make([]string, 0, len(env))
	for k, v := range env {
		items = append(items, fmt.Sprintf("%s=%s", k, v))
	}
	return items
}

func TestVaultApplianceApplyWhenExplicitlyAuthorised(t *testing.T) {
	token := os.Getenv("DIGITALOCEAN_TOKEN")
	if token == "" {
		t.Skip("DIGITALOCEAN_TOKEN not set; skipping apply test")
	}
	if strings.ToLower(os.Getenv("VAULT_APPLIANCE_ACCEPT_APPLY")) != "true" {
		t.Skip("VAULT_APPLIANCE_ACCEPT_APPLY must be 'true' to permit live apply tests")
	}

	vars := baseVars(t)
	vars["ha_enabled"] = false
	env := map[string]string{"DIGITALOCEAN_TOKEN": token}
	_, opts := setupTerraform(t, vars, env)

	defer terraform.Destroy(t, opts)
	terraform.InitAndApply(t, opts)

	endpoint := terraform.OutputMap(t, opts, "public_endpoint")
	require.NotEmpty(t, endpoint["name"], "expected name in public_endpoint output")
	require.NotEmpty(t, endpoint["ip"], "expected IP in public_endpoint output")

	ca := terraform.Output(t, opts, "ca_certificate")
	require.NotEmpty(t, ca, "expected CA certificate to be populated")

	var recoveryKeys []string
	rawKeys := terraform.Output(t, opts, "recovery_keys")
	require.NoError(t, json.Unmarshal([]byte(rawKeys), &recoveryKeys))
	require.Len(t, recoveryKeys, 5)
}
