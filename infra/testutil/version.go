package testutil

import (
	"os"
	"strings"
)

// KubernetesVersion returns the Kubernetes version for tests.
// It reads and trims DOKS_KUBERNETES_VERSION.
// Omit the 'kubernetes_version' input when this returns "" so the module's default applies.
func KubernetesVersion() string {
        return strings.TrimSpace(os.Getenv("DOKS_KUBERNETES_VERSION"))
}
