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

// FilePath represents a file system path.
type FilePath string

// MarkdownLine represents a line from a markdown document.
type MarkdownLine string

// TableCellName represents extracted table cell content.
type TableCellName string

// SectionState represents whether we're inside a specific markdown section.
type SectionState bool

// TestREADMEDocumentsAllOutputs verifies that all outputs defined in outputs.tf
// are documented in README.md.
func TestREADMEDocumentsAllOutputs(t *testing.T) {
	t.Parallel()

	moduleDir := ".."
	readmePath := FilePath(filepath.Join(moduleDir, "README.md"))
	outputsPath := FilePath(filepath.Join(moduleDir, "outputs.tf"))

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
	readmePath := FilePath(filepath.Join(moduleDir, "README.md"))

	// Find all variables files
	variablesFiles := []FilePath{
		FilePath(filepath.Join(moduleDir, "variables-core.tf")),
		FilePath(filepath.Join(moduleDir, "variables-cluster.tf")),
		FilePath(filepath.Join(moduleDir, "variables-credentials.tf")),
		FilePath(filepath.Join(moduleDir, "variables-tls.tf")),
	}

	// Extract all variable names from variables files
	var allVariables []string
	for _, vf := range variablesFiles {
		if _, err := os.Stat(string(vf)); err == nil {
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
	readmePath := FilePath(filepath.Join(moduleDir, "README.md"))

	content, err := os.ReadFile(string(readmePath))
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
func extractHCLOutputNames(t *testing.T, path FilePath) []string {
	t.Helper()

	content, err := os.ReadFile(string(path))
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
func extractHCLVariableNames(t *testing.T, path FilePath) []string {
	t.Helper()

	content, err := os.ReadFile(string(path))
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
func extractREADMEOutputNames(t *testing.T, path FilePath) []string {
	t.Helper()

	file, err := os.Open(string(path))
	require.NoError(t, err)
	defer file.Close()

	var names []string
	scanner := bufio.NewScanner(file)
	inOutputsSection := SectionState(false)

	for scanner.Scan() {
		line := MarkdownLine(scanner.Text())

		// Detect start of Outputs section
		if strings.HasPrefix(string(line), "## Outputs") {
			inOutputsSection = true
			continue
		}

		// Detect end of Outputs section (next heading)
		if bool(inOutputsSection) && strings.HasPrefix(string(line), "## ") {
			break
		}

		// Parse table rows in Outputs section
		if bool(inOutputsSection) && strings.HasPrefix(string(line), "|") {
			name := extractTableFirstColumn(line)
			if name != "" && name != "Name" && !strings.HasPrefix(string(name), "-") {
				names = append(names, string(name))
			}
		}
	}

	require.NoError(t, scanner.Err())
	return names
}

// extractREADMEInputNames parses a README.md file and extracts input names
// from Inputs tables.
func extractREADMEInputNames(t *testing.T, path FilePath) []string {
	t.Helper()

	file, err := os.Open(string(path))
	require.NoError(t, err)
	defer file.Close()

	var names []string
	scanner := bufio.NewScanner(file)
	inInputsSection := SectionState(false)

	for scanner.Scan() {
		line := MarkdownLine(scanner.Text())
		inInputsSection = updateInputsSectionState(line, inInputsSection)

		if shouldParseInputRow(line, inInputsSection) {
			if name := parseValidTableName(line); name != "" {
				names = append(names, string(name))
			}
		}
	}

	require.NoError(t, scanner.Err())
	return names
}

// updateInputsSectionState determines whether we're entering or exiting an
// Inputs section based on the current line and previous state.
func updateInputsSectionState(line MarkdownLine, inSection SectionState) SectionState {
	if isInputsSectionHeader(line) {
		return true
	}
	if bool(inSection) && isNonInputsSectionHeader(line) {
		return false
	}
	return inSection
}

// isInputsSectionHeader returns true if the line is an Inputs or Configuration
// section header (## or ### level containing "configuration" or "inputs").
func isInputsSectionHeader(line MarkdownLine) bool {
	s := string(line)
	if !strings.HasPrefix(s, "## ") && !strings.HasPrefix(s, "### ") {
		return false
	}
	lower := strings.ToLower(s)
	return strings.Contains(lower, "configuration") ||
		strings.Contains(lower, "inputs")
}

// isNonInputsSectionHeader returns true if the line is a ## level section
// header that doesn't contain "configuration".
func isNonInputsSectionHeader(line MarkdownLine) bool {
	s := string(line)
	if !strings.HasPrefix(s, "## ") {
		return false
	}
	return !strings.Contains(strings.ToLower(s), "configuration")
}

// shouldParseInputRow returns true if the line should be parsed as a table row
// (starts with "|" and we're in an inputs section).
func shouldParseInputRow(line MarkdownLine, inSection SectionState) bool {
	return bool(inSection) && strings.HasPrefix(string(line), "|")
}

// parseValidTableName extracts the first column from a table row and validates
// it. Returns empty TableCellName for invalid entries (empty, "Name", or
// separator rows starting with "-").
func parseValidTableName(line MarkdownLine) TableCellName {
	name := extractTableFirstColumn(line)
	if isInvalidTableName(name) {
		return ""
	}
	return name
}

// isInvalidTableName returns true if the name should be excluded from results.
// This includes empty names, table headers, and separator rows.
func isInvalidTableName(name TableCellName) bool {
	return isEmptyName(name) || isTableHeaderName(name) || isSeparatorRow(name)
}

// isEmptyName returns true if the name is an empty string.
func isEmptyName(name TableCellName) bool {
	return name == ""
}

// isTableHeaderName returns true if the name equals "Name", indicating it is
// the table header row rather than actual content.
func isTableHeaderName(name TableCellName) bool {
	return name == "Name"
}

// isSeparatorRow returns true if the name starts with "-", indicating it is
// a markdown table separator row.
func isSeparatorRow(name TableCellName) bool {
	return strings.HasPrefix(string(name), "-")
}

// extractTableFirstColumn extracts the first column value from a markdown
// table row.
func extractTableFirstColumn(line MarkdownLine) TableCellName {
	// Split by | and get the first non-empty cell
	parts := strings.Split(string(line), "|")
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

	return TableCellName(cell)
}
