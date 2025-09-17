package testutil

import (
	"os"
	"strings"
)

// KubernetesVersion returns the desired Kubernetes version override for tests.
// It reads and trims DOKS_KUBERNETES_VERSION.
// When this returns "", omit the 'kubernetes_version' input:
// - in module tests, the module default applies;
// - in dev cluster tests, the root default applies.
func KubernetesVersion() string {
	if version := strings.TrimSpace(os.Getenv("DOKS_KUBERNETES_VERSION")); version != "" {
		return version
	}
	return strings.TrimSpace(os.Getenv("DOKS_VERSION"))
}
