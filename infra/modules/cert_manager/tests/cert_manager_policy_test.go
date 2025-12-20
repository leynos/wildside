package tests

import (
	"testing"
	"time"

	"github.com/stretchr/testify/require"
)

func TestCertManagerModulePlanPolicy(t *testing.T) {
	t.Parallel()
	requireBinary(t, "conftest", "conftest not found; skipping policy test")

	kubeconfig := requireEnvVar(t, "KUBECONFIG", "KUBECONFIG not set; skipping plan policy test")
	vars := testVars(t)
	vars["kubeconfig_path"] = kubeconfig
	tfDir, planPath := renderCertManagerPlan(t, vars)
	policyPath := certManagerPlanPolicyPath(tfDir)

	out, err := runConftest(t, conftestRun{
		InputPath:  planPath,
		PolicyPath: policyPath,
		Kubeconfig: kubeconfig,
		ExtraArgs: []string{
			"--fail-on-warn",
			"--namespace",
			certManagerPolicyPlanNamespace,
		},
		Timeout: 60 * time.Second,
	})
	require.NoErrorf(t, err, "conftest failed: %s", string(out))
}

var planPolicyRejectionTestCases = []struct {
	name            string
	planPayload     string
	expectedMessage string
}{
	{
		name: "MissingAcmeEmail",
		planPayload: `{
  "resource_changes": [{
    "type": "kubernetes_manifest",
    "change": {
      "after": {
        "manifest": {
          "apiVersion": "cert-manager.io/v1",
          "kind": "ClusterIssuer",
          "metadata": {"name": "letsencrypt-staging"},
          "spec": {
            "acme": {
              "server": "https://acme-staging-v02.api.letsencrypt.org/directory",
              "email": "",
              "privateKeySecretRef": {"name": "letsencrypt-staging-account-key"},
              "solvers": [{
                "dns01": {
                  "webhook": {
                    "groupName": "acme.example.com",
                    "solverName": "namecheap",
                    "config": {
                      "apiKeySecretRef": {"name": "namecheap-api", "key": "api-key"},
                      "apiUserSecretRef": {"name": "namecheap-api", "key": "api-user"}
                    }
                  }
                }
              }]
            }
          }
        }
      }
    }
  }]
}`,
		expectedMessage: "valid ACME email",
	},
	{
		name: "MissingWebhookSecretRef",
		planPayload: `{
  "resource_changes": [{
    "type": "kubernetes_manifest",
    "change": {
      "after": {
        "manifest": {
          "apiVersion": "cert-manager.io/v1",
          "kind": "ClusterIssuer",
          "metadata": {"name": "letsencrypt-staging"},
          "spec": {
            "acme": {
              "server": "https://acme-staging-v02.api.letsencrypt.org/directory",
              "email": "admin@example.test",
              "privateKeySecretRef": {"name": "letsencrypt-staging-account-key"},
              "solvers": [{
                "dns01": {
                  "webhook": {
                    "groupName": "acme.example.com",
                    "solverName": "namecheap",
                    "config": {
                      "apiKeySecretRef": {"name": "", "key": "api-key"},
                      "apiUserSecretRef": {"name": "namecheap-api", "key": "api-user"}
                    }
                  }
                }
              }]
            }
          }
        }
      }
    }
  }]
}`,
		expectedMessage: "apiKeySecretRef.name",
	},
	{
		name: "MissingVaultCaBundle",
		planPayload: `{
  "resource_changes": [{
    "type": "kubernetes_manifest",
    "change": {
      "after": {
        "manifest": {
          "apiVersion": "cert-manager.io/v1",
          "kind": "ClusterIssuer",
          "metadata": {"name": "vault-issuer"},
          "spec": {
            "vault": {
              "server": "https://vault.example.test:8200",
              "path": "pki/sign/example",
              "auth": {
                "tokenSecretRef": {"name": "vault-token", "key": "token"}
              },
              "caBundle": ""
            }
          }
        }
      }
    }
  }]
}`,
		expectedMessage: "vault.caBundle",
	},
	{
		name: "MissingWebhookPdb",
		planPayload: `{
  "resource_changes": [{
    "type": "helm_release",
    "change": {
      "after": {
        "name": "cert-manager",
        "chart": "cert-manager",
        "repository": "oci://quay.io/jetstack/charts",
        "version": "v1.19.2",
        "values": ["replicaCount: 3\nwebhook:\n  replicaCount: 3\ncainjector:\n  replicaCount: 3\n"]
      }
    }
  }]
}`,
		expectedMessage: "PodDisruptionBudget",
	},
}

func TestCertManagerModulePlanPolicyRejections(t *testing.T) {
	t.Parallel()
	requireBinary(t, "conftest", "conftest not found; skipping policy test")

	tfDir, _ := setup(t, testVars(t))
	policyPath := certManagerPlanPolicyPath(tfDir)

	for _, tc := range planPolicyRejectionTestCases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			planPath := writePlanFixture(t, tc.planPayload)

			out, err := runConftest(t, conftestRun{
				InputPath:  planPath,
				PolicyPath: policyPath,
				Kubeconfig: "",
				ExtraArgs: []string{
					"--fail-on-warn",
					"--namespace",
					certManagerPolicyPlanNamespace,
				},
				Timeout: 60 * time.Second,
			})
			require.Error(t, err, "expected conftest to report a violation")
			require.Contains(t, string(out), tc.expectedMessage)
		})
	}
}

func TestCertManagerModulePlanPolicyIgnoresNonCertManagerWebhook(t *testing.T) {
	t.Parallel()
	requireBinary(t, "conftest", "conftest not found; skipping policy test")

	tfDir, _ := setup(t, testVars(t))
	policyPath := certManagerPlanPolicyPath(tfDir)
	planPayload := `{
  "resource_changes": [{
    "type": "helm_release",
    "change": {
      "after": {
        "name": "webhook-operator",
        "chart": "webhook-operator",
        "repository": "https://example.com/charts",
        "version": "1.2.3",
        "values": ["replicaCount: 1\n"]
      }
    }
  }]
}`
	planPath := writePlanFixture(t, planPayload)

	out, err := runConftest(t, conftestRun{
		InputPath:  planPath,
		PolicyPath: policyPath,
		Kubeconfig: "",
		ExtraArgs: []string{
			"--fail-on-warn",
			"--namespace",
			certManagerPolicyPlanNamespace,
		},
		Timeout: 60 * time.Second,
	})
	require.NoErrorf(t, err, "expected non-cert-manager webhook to pass: %s", string(out))
}
