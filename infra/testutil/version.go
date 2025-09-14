package testutil

import "os"

// KubernetesVersion returns the Kubernetes version for tests.
// It reads the value from the DOKS_KUBERNETES_VERSION environment variable.
func KubernetesVersion() string {
	return os.Getenv("DOKS_KUBERNETES_VERSION")
}
