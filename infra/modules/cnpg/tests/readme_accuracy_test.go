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

// MarkdownTableRow represents a markdown table row.
type MarkdownTableRow string

// MarkdownLine represents a line from a markdown document.
type MarkdownLine string

// TableCellName represents extracted table cell content.
type TableCellName string

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

	// Verify README does not document outputs that are not defined in outputs.tf
	for _, output := range documentedOutputs {
		assert.Contains(t, actualOutputs, output,
			"output %q is documented in README.md but not defined in outputs.tf", output)
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
		FilePath(filepath.Join(moduleDir, "variables-backup.tf")),
		FilePath(filepath.Join(moduleDir, "variables-credentials.tf")),
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
}

// extractHCLOutputNames parses an HCL file and returns all output block names.
func extractHCLOutputNames(t *testing.T, path FilePath) []string {
	t.Helper()

	content, err := os.ReadFile(string(path))
	require.NoError(t, err)

	// Match output "name" { patterns, allowing for leading indentation
	re := regexp.MustCompile(`(?m)^\s*output\s+"([^"]+)"\s*\{`)
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

	// Match variable "name" { patterns, allowing for leading indentation
	re := regexp.MustCompile(`(?m)^\s*variable\s+"([^"]+)"\s*\{`)
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
	inOutputsSection := false

	for scanner.Scan() {
		line := MarkdownLine(scanner.Text())
		inOutputsSection = updateOutputsSectionState(line, inOutputsSection)

		if shouldParseOutputRow(line, inOutputsSection) {
			if name := parseValidTableName(line); name != "" {
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
	inInputsSection := false

	for scanner.Scan() {
		line := MarkdownLine(scanner.Text())
		inInputsSection = updateInputsSectionState(line, inInputsSection)

		if shouldParseInputRow(line, inInputsSection) {
			if name := parseValidInputTableName(line); name != "" {
				names = append(names, string(name))
			}
		}
	}

	require.NoError(t, scanner.Err())
	return names
}

// updateOutputsSectionState determines whether we're entering or exiting the
// Outputs section based on the current line and previous state.
func updateOutputsSectionState(line MarkdownLine, inSection bool) bool {
	if strings.HasPrefix(string(line), "## Outputs") {
		return true
	}
	if inSection && strings.HasPrefix(string(line), "## ") {
		return false
	}
	return inSection
}

// shouldParseOutputRow returns true if the line should be parsed as a table
// row (starts with "|" and we're in the Outputs section).
func shouldParseOutputRow(line MarkdownLine, inSection bool) bool {
	return inSection && strings.HasPrefix(string(line), "|")
}

// updateInputsSectionState determines whether we're entering or exiting an
// Inputs section based on the current line and previous state.
func updateInputsSectionState(line MarkdownLine, inSection bool) bool {
	if isInputsSectionHeader(line) {
		return true
	}
	if inSection && isNonInputsSectionHeader(line) {
		return false
	}
	return inSection
}

// isInputsSectionHeader returns true if the line is a ## or ### level header
// containing "configuration" or "inputs" (case-insensitive).
func isInputsSectionHeader(line MarkdownLine) bool {
	if !isHeadingLevel2Or3(line) {
		return false
	}
	lower := strings.ToLower(string(line))
	return strings.Contains(lower, "configuration") ||
		strings.Contains(lower, "inputs")
}

// isNonInputsSectionHeader returns true if the line is a ## level header that
// doesn't contain "configuration" (case-insensitive).
func isNonInputsSectionHeader(line MarkdownLine) bool {
	if !strings.HasPrefix(string(line), "## ") {
		return false
	}
	return !strings.Contains(strings.ToLower(string(line)), "configuration")
}

// isHeadingLevel2Or3 returns true if the line starts with "## " or "### ".
func isHeadingLevel2Or3(line MarkdownLine) bool {
	s := string(line)
	return strings.HasPrefix(s, "## ") || strings.HasPrefix(s, "### ")
}

// shouldParseInputRow returns true if the line should be parsed as a table row
// (starts with "|" and we're in an inputs section).
func shouldParseInputRow(line MarkdownLine, inSection bool) bool {
	return inSection && strings.HasPrefix(string(line), "|")
}

// parseValidInputTableName extracts the first column from a table row and
// validates it using isInvalidTableName. Returns empty TableCellName for
// invalid entries.
func parseValidInputTableName(line MarkdownLine) TableCellName {
	name := extractTableFirstColumn(MarkdownTableRow(line))
	if isInvalidTableName(name) {
		return ""
	}
	return name
}

// parseValidTableName extracts the first column from a table row and validates
// it. Returns empty TableCellName for invalid entries (empty, "Name", or
// separator rows starting with "-").
func parseValidTableName(line MarkdownLine) TableCellName {
	name := extractTableFirstColumn(MarkdownTableRow(line))
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
func extractTableFirstColumn(line MarkdownTableRow) TableCellName {
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
