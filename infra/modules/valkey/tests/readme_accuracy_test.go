package tests

import (
	"bufio"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestREADMEDocumentsAllOutputs verifies that all outputs defined in outputs.tf
// are documented in README.md.
func TestREADMEDocumentsAllOutputs(t *testing.T) {
	t.Parallel()

	moduleDir := ".."
	readmePath := filepath.Join(moduleDir, "README.md")
	outputsPath := filepath.Join(moduleDir, "outputs.tf")

	// Extract output names from outputs.tf
	actualOutputs := extractHCLOutputNames(t, outputsPath)
	require.NotEmpty(t, actualOutputs, "outputs.tf should define at least one output")

	// Extract documented outputs from README.md
	documentedOutputs := extractREADMEOutputNames(t, readmePath)
	require.NotEmpty(t, documentedOutputs, "README.md should document at least one output")

	// Verify all actual outputs are documented
	for _, output := range actualOutputs {
		assert.Contains(t, documentedOutputs, output,
			"output %q is defined in outputs.tf but not documented in README.md", output)
	}
}

// TestREADMEDocumentsAllRequiredInputs verifies that all required inputs are
// documented in README.md.
func TestREADMEDocumentsAllRequiredInputs(t *testing.T) {
	t.Parallel()

	moduleDir := ".."
	readmePath := filepath.Join(moduleDir, "README.md")

	// Find all variables files
	variablesFiles := []string{
		filepath.Join(moduleDir, "variables-core.tf"),
		filepath.Join(moduleDir, "variables-cluster.tf"),
		filepath.Join(moduleDir, "variables-credentials.tf"),
		filepath.Join(moduleDir, "variables-tls.tf"),
	}

	// Extract all variable names from variables files
	var allVariables []string
	for _, vf := range variablesFiles {
		if _, err := os.Stat(vf); err == nil {
			vars := extractHCLVariableNames(t, vf)
			allVariables = append(allVariables, vars...)
		}
	}
	require.NotEmpty(t, allVariables, "at least one variable should be defined")

	// Extract documented inputs from README.md
	documentedInputs := extractREADMEInputNames(t, readmePath)
	require.NotEmpty(t, documentedInputs, "README.md should document at least one input")

	// Verify all variables are documented
	for _, variable := range allVariables {
		assert.Contains(t, documentedInputs, variable,
			"variable %q is defined but not documented in README.md", variable)
	}
}

// TestREADMEDocumentsSyncPolicyContract verifies that the sync_policy_contract
// output is documented with its structure.
func TestREADMEDocumentsSyncPolicyContract(t *testing.T) {
	t.Parallel()

	moduleDir := ".."
	readmePath := filepath.Join(moduleDir, "README.md")

	content, err := os.ReadFile(readmePath)
	require.NoError(t, err)

	readme := string(content)

	// Verify sync_policy_contract section exists
	assert.Contains(t, readme, "sync_policy_contract",
		"README.md should document sync_policy_contract output")

	// Verify key contract fields are documented
	assert.Contains(t, readme, "cluster",
		"sync_policy_contract documentation should include 'cluster' field")
	assert.Contains(t, readme, "endpoints",
		"sync_policy_contract documentation should include 'endpoints' field")
	assert.Contains(t, readme, "credentials",
		"sync_policy_contract documentation should include 'credentials' field")
	assert.Contains(t, readme, "tls",
		"sync_policy_contract documentation should include 'tls' field")
	assert.Contains(t, readme, "persistence",
		"sync_policy_contract documentation should include 'persistence' field")
	assert.Contains(t, readme, "replication",
		"sync_policy_contract documentation should include 'replication' field")
}

// extractHCLOutputNames parses an HCL file and returns all output block names.
func extractHCLOutputNames(t *testing.T, path string) []string {
	t.Helper()

	content, err := os.ReadFile(path)
	require.NoError(t, err)

	// Match output "name" { patterns
	re := regexp.MustCompile(`(?m)^output\s+"([^"]+)"\s*\{`)
	matches := re.FindAllStringSubmatch(string(content), -1)

	var names []string
	for _, match := range matches {
		if len(match) > 1 {
			names = append(names, match[1])
		}
	}
	return names
}

// extractHCLVariableNames parses an HCL file and returns all variable block
// names.
func extractHCLVariableNames(t *testing.T, path string) []string {
	t.Helper()

	content, err := os.ReadFile(path)
	require.NoError(t, err)

	// Match variable "name" { patterns
	re := regexp.MustCompile(`(?m)^variable\s+"([^"]+)"\s*\{`)
	matches := re.FindAllStringSubmatch(string(content), -1)

	var names []string
	for _, match := range matches {
		if len(match) > 1 {
			names = append(names, match[1])
		}
	}
	return names
}

// extractREADMEOutputNames parses a README.md file and extracts output names
// from the Outputs table.
func extractREADMEOutputNames(t *testing.T, path string) []string {
	t.Helper()

	file, err := os.Open(path)
	require.NoError(t, err)
	defer file.Close()

	var names []string
	scanner := bufio.NewScanner(file)
	inOutputsSection := false

	for scanner.Scan() {
		line := scanner.Text()

		// Detect start of Outputs section
		if strings.HasPrefix(line, "## Outputs") {
			inOutputsSection = true
			continue
		}

		// Detect end of Outputs section (next heading)
		if inOutputsSection && strings.HasPrefix(line, "## ") {
			break
		}

		// Parse table rows in Outputs section
		if inOutputsSection && strings.HasPrefix(line, "|") {
			name := extractTableFirstColumn(line)
			if name != "" && name != "Name" && !strings.HasPrefix(name, "-") {
				names = append(names, name)
			}
		}
	}

	require.NoError(t, scanner.Err())
	return names
}

// extractREADMEInputNames parses a README.md file and extracts input names
// from Inputs tables.
func extractREADMEInputNames(t *testing.T, path string) []string {
	t.Helper()

	file, err := os.Open(path)
	require.NoError(t, err)
	defer file.Close()

	var names []string
	scanner := bufio.NewScanner(file)
	inInputsSection := false

	for scanner.Scan() {
		line := scanner.Text()

		// Detect start of Inputs section (any heading containing "configuration"
		// or exactly "## Inputs")
		if strings.HasPrefix(line, "## ") || strings.HasPrefix(line, "### ") {
			lower := strings.ToLower(line)
			if strings.Contains(lower, "configuration") ||
				strings.Contains(lower, "inputs") {
				inInputsSection = true
				continue
			}
			// Other section headings end the inputs parsing unless they're
			// subsections
			if inInputsSection && strings.HasPrefix(line, "## ") &&
				!strings.Contains(lower, "configuration") {
				inInputsSection = false
			}
		}

		// Parse table rows in Inputs sections
		if inInputsSection && strings.HasPrefix(line, "|") {
			name := extractTableFirstColumn(line)
			if name != "" && name != "Name" && !strings.HasPrefix(name, "-") {
				names = append(names, name)
			}
		}
	}

	require.NoError(t, scanner.Err())
	return names
}

// extractTableFirstColumn extracts the first column value from a markdown
// table row.
func extractTableFirstColumn(line string) string {
	// Split by | and get the first non-empty cell
	parts := strings.Split(line, "|")
	if len(parts) < 2 {
		return ""
	}

	// First part is empty (before first |), second is first column
	cell := strings.TrimSpace(parts[1])

	// Remove backticks around variable names
	cell = strings.Trim(cell, "`")

	// Skip separator rows (containing only dashes and colons)
	if strings.Trim(cell, "-:") == "" {
		return ""
	}

	return cell
}
