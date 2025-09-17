package testutil

import (
	"os"
	"strings"
)

// KubernetesVersion returns the desired Kubernetes version override for tests.
// It reads and trims DOKS_KUBERNETES_VERSION; if unset, it falls back to
// DOKS_VERSION so legacy tooling continues to work. When this returns "", omit
// the 'kubernetes_version' input:
// - in module tests, the module default applies;
// - in dev cluster tests, the root default applies.
func KubernetesVersion() string {
	if v := strings.TrimSpace(os.Getenv("DOKS_KUBERNETES_VERSION")); v != "" {
		return v
	}
	if v := strings.TrimSpace(os.Getenv("DOKS_VERSION")); v != "" {
		return v
	}
	return ""
}
