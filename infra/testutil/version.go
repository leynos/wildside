package testutil

import "os"

// KubernetesVersion returns the Kubernetes version for tests.
// It reads DOKS_KUBERNETES_VERSION and falls back to a known-good default.
func KubernetesVersion() string {
        if v := os.Getenv("DOKS_KUBERNETES_VERSION"); v != "" {
                return v
        }
        return "1.31.1-do.3"
}
