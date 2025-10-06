package testutil

import (
	"os"
	"os/exec"
	"strings"
	"testing"
)

// envEntriesToMap converts VAR=value slices into a map for easier assertions.
func envEntriesToMap(entries []string) map[string]string {
	env := make(map[string]string, len(entries))
	for _, entry := range entries {
		parts := strings.SplitN(entry, "=", 2)
		if len(parts) != 2 {
			continue
		}
		env[parts[0]] = parts[1]
	}
	return env
}

// parseEnvOutput converts the output from the `env` command to a map so tests
// can assert on individual keys.
func parseEnvOutput(output string) map[string]string {
	lines := strings.Split(strings.TrimSpace(output), "\n")
	env := make(map[string]string, len(lines))
	for _, line := range lines {
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			continue
		}
		env[parts[0]] = parts[1]
	}
	return env
}

func TestTerraformEnvVarsIncludesAutomation(t *testing.T) {
	t.Parallel()
	env := TerraformEnvVars(nil)
	if got := env["TF_IN_AUTOMATION"]; got != "1" {
		t.Fatalf("TF_IN_AUTOMATION mismatch: want 1, got %q", got)
	}
}

func TestTerraformEnvVarsMergesExtras(t *testing.T) {
	t.Parallel()
	extras := map[string]string{"FOO": "bar"}
	env := TerraformEnvVars(extras)
	if got := env["FOO"]; got != "bar" {
		t.Fatalf("expected FOO=bar, got %q", got)
	}
	if _, ok := extras["TF_IN_AUTOMATION"]; ok {
		t.Fatalf("extras map was mutated: %v", extras)
	}
}

func TestTerraformEnvVarsAllowsOverrides(t *testing.T) {
	t.Parallel()
	env := TerraformEnvVars(map[string]string{"TF_IN_AUTOMATION": "0"})
	if got := env["TF_IN_AUTOMATION"]; got != "0" {
		t.Fatalf("expected override to win, got %q", got)
	}
}

func TestTerraformEnvSetsProcessEnvironment(t *testing.T) {
	t.Setenv("SHOULD_NOT_LEAK", "1")
	envSlice := TerraformEnv(t, map[string]string{"FOO": "bar"})
	env := envEntriesToMap(envSlice)

	if got := os.Getenv("FOO"); got != "bar" {
		t.Fatalf("process env missing FOO=bar, got %q", got)
	}
	if got := os.Getenv("TF_IN_AUTOMATION"); got != "1" {
		t.Fatalf("process env missing TF_IN_AUTOMATION=1, got %q", got)
	}
	if _, ok := env["SHOULD_NOT_LEAK"]; ok {
		t.Fatalf("unexpected leaked variable in env slice: %v", env)
	}
	if _, ok := env["FOO"]; !ok {
		t.Fatalf("env slice missing FOO entry: %v", env)
	}
	if _, ok := env["TF_IN_AUTOMATION"]; !ok {
		t.Fatalf("env slice missing TF_IN_AUTOMATION entry: %v", env)
	}
	if _, ok := os.LookupEnv("PATH"); ok {
		if _, present := env["PATH"]; !present {
			t.Fatalf("PATH not propagated to child environment: %v", env)
		}
	}
}

func TestTerraformEnvIsolatesPerTest(t *testing.T) {
	const key = "ISOLATED_VAR"

	t.Run("first", func(t *testing.T) {
		TerraformEnv(t, map[string]string{key: "one"})
	})

	t.Run("second", func(t *testing.T) {
		if value, ok := os.LookupEnv(key); ok {
			t.Fatalf("expected %s to be unset after previous subtest, found %q", key, value)
		}
		TerraformEnv(t, map[string]string{key: "two"})
	})
}

func TestTerraformEnvAppliesToCommands(t *testing.T) {
	t.Setenv("SHOULD_NOT_LEAK", "1")
	envSlice := TerraformEnv(t, map[string]string{"FOO": "bar"})

	cmd := exec.Command("env")
	cmd.Env = envSlice
	output, err := cmd.Output()
	if err != nil {
		t.Fatalf("env command failed: %v", err)
	}

	env := parseEnvOutput(string(output))
	if got := env["FOO"]; got != "bar" {
		t.Fatalf("expected child env to contain FOO=bar, got %q", got)
	}
	if got := env["TF_IN_AUTOMATION"]; got != "1" {
		t.Fatalf("expected child env to contain TF_IN_AUTOMATION=1, got %q", got)
	}
	if _, ok := env["SHOULD_NOT_LEAK"]; ok {
		t.Fatalf("child env leaked parent variable: %v", env)
	}
}

func TestTerraformEnvHandlesNilExtras(t *testing.T) {
	envSlice := TerraformEnv(t, nil)
	env := envEntriesToMap(envSlice)
	if got := env["TF_IN_AUTOMATION"]; got != "1" {
		t.Fatalf("expected automation flag for nil extras, got %q", got)
	}
}
