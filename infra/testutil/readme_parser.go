// README parsing utilities for module documentation accuracy tests.
//
// # README Heading Conventions
//
// This file provides functions for parsing README.md files to extract
// documented inputs and outputs. The parsing relies on specific markdown
// heading conventions:
//
//   - Output tables must appear under a "## Outputs" heading (exact match)
//   - Input tables must appear under headings containing "configuration" or
//     "inputs" (case-insensitive), at ## or ### level
//   - A new ## heading that doesn't contain "configuration" ends the inputs
//     section
//
// These conventions should be followed in all module README files to ensure
// accurate documentation testing.
package testutil

import (
	"bufio"
	"fmt"
	"os"
	"regexp"
	"strings"
	"testing"

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

// SectionStateUpdater is a function type that updates section state based on
// the current line and previous state.
type SectionStateUpdater func(line MarkdownLine, inSection SectionState) SectionState

// RowChecker is a function type that determines if a line should be parsed.
type RowChecker func(line MarkdownLine, inSection SectionState) bool

// ExtractHCLBlockNames parses an HCL file and returns all block names of the
// specified type (e.g., "output" or "variable"). The regex pattern tolerates
// leading whitespace to handle indented block definitions.
func ExtractHCLBlockNames(t *testing.T, path FilePath, blockType string) []string {
	t.Helper()

	content, err := os.ReadFile(string(path))
	require.NoError(t, err)

	pattern := fmt.Sprintf(`(?m)^\s*%s\s+"([^"]+)"\s*\{`, blockType)
	re := regexp.MustCompile(pattern)
	matches := re.FindAllStringSubmatch(string(content), -1)

	var names []string
	for _, match := range matches {
		if len(match) > 1 {
			names = append(names, match[1])
		}
	}
	return names
}

// ExtractHCLOutputNames parses an HCL file and returns all output block names.
func ExtractHCLOutputNames(t *testing.T, path FilePath) []string {
	return ExtractHCLBlockNames(t, path, "output")
}

// ExtractHCLVariableNames parses an HCL file and returns all variable block
// names.
func ExtractHCLVariableNames(t *testing.T, path FilePath) []string {
	return ExtractHCLBlockNames(t, path, "variable")
}

// ExtractREADMESectionTableNames parses a README.md file and extracts table
// names from a specific section using the provided state updater and row
// checker functions.
func ExtractREADMESectionTableNames(
	t *testing.T,
	path FilePath,
	stateUpdater SectionStateUpdater,
	shouldParseRow RowChecker,
) []string {
	t.Helper()

	file, err := os.Open(string(path))
	require.NoError(t, err)
	defer file.Close()

	var names []string
	scanner := bufio.NewScanner(file)
	inSection := SectionState(false)

	for scanner.Scan() {
		line := MarkdownLine(scanner.Text())
		inSection = stateUpdater(line, inSection)

		if shouldParseRow(line, inSection) {
			if name := ParseValidTableName(line); name != "" {
				names = append(names, string(name))
			}
		}
	}

	require.NoError(t, scanner.Err())
	return names
}

// ExtractREADMEOutputNames parses a README.md file and extracts output names
// from the Outputs table.
//
// Expected heading format: "## Outputs" (exact match, case-sensitive).
func ExtractREADMEOutputNames(t *testing.T, path FilePath) []string {
	return ExtractREADMESectionTableNames(
		t,
		path,
		UpdateOutputsSectionState,
		ShouldParseOutputRow,
	)
}

// UpdateOutputsSectionState determines whether we're entering or exiting the
// Outputs section based on the current line and previous state.
//
// Enters section on: "## Outputs" heading (exact prefix match)
// Exits section on: any other "## " heading.
func UpdateOutputsSectionState(line MarkdownLine, inSection SectionState) SectionState {
	if strings.HasPrefix(string(line), "## Outputs") {
		return true
	}
	if bool(inSection) && strings.HasPrefix(string(line), "## ") {
		return false
	}
	return inSection
}

// ShouldParseOutputRow returns true if the line should be parsed as a table
// row (starts with "|" and we're in the Outputs section).
func ShouldParseOutputRow(line MarkdownLine, inSection SectionState) bool {
	return bool(inSection) && strings.HasPrefix(string(line), "|")
}

// ExtractREADMEInputNames parses a README.md file and extracts input names
// from Inputs tables.
//
// Expected heading format: ## or ### level headings containing "configuration"
// or "inputs" (case-insensitive). For example:
//   - "## Inputs"
//   - "### Core configuration"
//   - "### Cluster configuration"
func ExtractREADMEInputNames(t *testing.T, path FilePath) []string {
	return ExtractREADMESectionTableNames(
		t,
		path,
		UpdateInputsSectionState,
		ShouldParseInputRow,
	)
}

// UpdateInputsSectionState determines whether we're entering or exiting an
// Inputs section based on the current line and previous state.
//
// Enters section on: ## or ### heading containing "configuration" or "inputs"
// Exits section on: ## heading not containing "configuration".
func UpdateInputsSectionState(line MarkdownLine, inSection SectionState) SectionState {
	if IsInputsSectionHeader(line) {
		return true
	}
	if bool(inSection) && IsNonInputsSectionHeader(line) {
		return false
	}
	return inSection
}

// IsInputsSectionHeader returns true if the line is an Inputs or Configuration
// section header (## or ### level containing "configuration" or "inputs").
func IsInputsSectionHeader(line MarkdownLine) bool {
	if !IsHeadingLevel2Or3(line) {
		return false
	}
	lower := strings.ToLower(string(line))
	return strings.Contains(lower, "configuration") ||
		strings.Contains(lower, "inputs")
}

// IsNonInputsSectionHeader returns true if the line is a ## level section
// header that doesn't contain "configuration".
func IsNonInputsSectionHeader(line MarkdownLine) bool {
	s := string(line)
	if !strings.HasPrefix(s, "## ") {
		return false
	}
	return !strings.Contains(strings.ToLower(s), "configuration")
}

// IsHeadingLevel2Or3 returns true if the line starts with "## " or "### ".
func IsHeadingLevel2Or3(line MarkdownLine) bool {
	s := string(line)
	return strings.HasPrefix(s, "## ") || strings.HasPrefix(s, "### ")
}

// ShouldParseInputRow returns true if the line should be parsed as a table row
// (starts with "|" and we're in an inputs section).
func ShouldParseInputRow(line MarkdownLine, inSection SectionState) bool {
	return bool(inSection) && strings.HasPrefix(string(line), "|")
}

// ParseValidTableName extracts the first column from a table row and validates
// it. Returns empty TableCellName for invalid entries (empty, "Name", or
// separator rows starting with "-").
func ParseValidTableName(line MarkdownLine) TableCellName {
	name := ExtractTableFirstColumn(line)
	if IsInvalidTableName(name) {
		return ""
	}
	return name
}

// IsInvalidTableName returns true if the name should be excluded from results.
// This includes empty names, table headers, and separator rows.
func IsInvalidTableName(name TableCellName) bool {
	return IsEmptyName(name) || IsTableHeaderName(name) || IsSeparatorRow(name)
}

// IsEmptyName returns true if the name is an empty string.
func IsEmptyName(name TableCellName) bool {
	return name == ""
}

// IsTableHeaderName returns true if the name equals "Name", indicating it is
// the table header row rather than actual content.
func IsTableHeaderName(name TableCellName) bool {
	return name == "Name"
}

// IsSeparatorRow returns true if the name starts with "-", indicating it is
// a markdown table separator row.
func IsSeparatorRow(name TableCellName) bool {
	return strings.HasPrefix(string(name), "-")
}

// ExtractTableFirstColumn extracts the first column value from a markdown
// table row.
func ExtractTableFirstColumn(line MarkdownLine) TableCellName {
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
