package testutil

import (
        "os"
        "strings"
)

// KubernetesVersion returns the Kubernetes version for tests.
// It reads DOKS_KUBERNETES_VERSION and relies on the module's default when unset.
func KubernetesVersion() string {
        return strings.TrimSpace(os.Getenv("DOKS_KUBERNETES_VERSION"))
}
