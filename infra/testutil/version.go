package testutil

import (
	"os"
	"strings"

	_ "embed"
)

//go:embed DOKS_KUBERNETES_VERSION
var embeddedVersion string

// KubernetesVersion returns the default Kubernetes version for tests.
// An environment variable overrides the embedded default.
func KubernetesVersion() string {
	if v := os.Getenv("DOKS_KUBERNETES_VERSION"); v != "" {
		return v
	}
	return strings.TrimSpace(embeddedVersion)
}
