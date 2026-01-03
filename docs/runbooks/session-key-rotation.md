# Session signing key rotation

This runbook documents the procedure for rotating the session signing key used
by the Wildside backend for cookie-based authentication sessions.

## Overview

The backend uses `actix-session` with `CookieSessionStore` for stateless,
cookie-based sessions. Sessions are signed with a secret key loaded from a
Kubernetes Secret. Key rotation relies on **rolling deployment overlap** rather
than in-app dual-key validation: during rotation, old pods continue serving
with the old key while new pods use the new key.

Sessions have a 2-hour TTL, so the overlap window naturally allows existing
sessions to remain valid until they expire.

## Prerequisites

Before starting rotation:

- [ ] Verify at least 2 replicas are running (required for zero-downtime)
- [ ] Record the current key fingerprint from pod logs
- [ ] Ensure the Kubernetes Secret exists and is mounted correctly
- [ ] Have `kubectl` configured with cluster access

### Check replica count

```bash
kubectl get deployment wildside-backend -n wildside -o jsonpath='{.spec.replicas}'
```

Rotation requires **at least 2 replicas** to maintain availability during the
rolling restart.

### Get current fingerprint

```bash
kubectl logs -n wildside -l app.kubernetes.io/name=wildside --tail=100 \
  | grep "session signing key loaded"
```

Record the fingerprint value for comparison after rotation.

## Rotation procedure

### Option A: Using the rotation script (recommended)

The automated script handles key generation, secret update, and rollout:

```bash
./scripts/rotate_session_key.py \
  --namespace wildside \
  --secret-name wildside-session-key \
  --deployment wildside-backend
```

The script will:

1. Generate a new 64-byte cryptographically secure key
2. Update the Kubernetes Secret
3. Trigger a rolling restart
4. Wait for the rollout to complete
5. Output old and new fingerprints for logging

### Option B: Manual rotation

If the script is unavailable, follow these manual steps:

#### 1. Generate a new key

```bash
NEW_KEY=$(openssl rand 64 | base64 -w0)
echo "New key generated (base64): ${NEW_KEY:0:20}..."
```

#### 2. Update the Kubernetes Secret

```bash
kubectl patch secret wildside-session-key -n wildside \
  --type=merge \
  -p "{\"data\": {\"session_key\": \"$NEW_KEY\"}}"
```

#### 3. Trigger rolling restart

```bash
kubectl rollout restart deployment/wildside-backend -n wildside
```

#### 4. Monitor the rollout

```bash
kubectl rollout status deployment/wildside-backend -n wildside --timeout=300s
```

## Post-rotation verification

### Verify new fingerprint in logs

```bash
kubectl logs -n wildside -l app.kubernetes.io/name=wildside --tail=100 \
  | grep "session signing key loaded"
```

All pods should report the **new** fingerprint.

### Verify session functionality

1. Log in to the application in a new browser session
2. Confirm the session persists across page refreshes
3. Check for any 401 errors in the pod logs

### Record the rotation

Log the rotation in your change management system:

- Date and time of rotation
- Old fingerprint
- New fingerprint
- Operator name
- Reason for rotation (scheduled/incident/compromise)

## Rollback procedure

If issues arise after rotation, rollback by restoring the previous key:

### If you saved the old key

```bash
kubectl patch secret wildside-session-key -n wildside \
  --type=merge \
  -p '{"data": {"session_key": "<base64-encoded-old-key>"}}'

kubectl rollout restart deployment/wildside-backend -n wildside
```

### If you did not save the old key

Without the old key, rollback is not possible. Users with existing sessions
will be logged out when routed to new pods. Options:

1. Accept the session disruption (users re-authenticate)
2. Continue with the new key

## Troubleshooting

### Pods crash after rotation

**Symptom:** New pods fail readiness checks or crash loop.

**Cause:** Key file may not be mounted correctly or key is too short.

**Resolution:**

```bash
# Check if the secret is mounted
kubectl exec -n wildside <pod-name> -- ls -la /var/run/secrets/

# Check pod logs for key loading errors
kubectl logs -n wildside <pod-name> | grep -i "session"
```

### Mixed fingerprints during rollout

**Symptom:** Logs show both old and new fingerprints.

**Expected:** During rolling deployment, this is normal. Old pods retain the
old fingerprint until they terminate.

**Resolution:** Wait for rollout to complete:

```bash
kubectl rollout status deployment/wildside-backend -n wildside
```

### Users reporting logout issues

**Symptom:** Some users are unexpectedly logged out during rotation.

**Cause:** Users routed to new pods have their old sessions invalidated.

**Mitigation:** This is expected behaviour with the rolling overlap approach.
Sessions will recover after users re-authenticate. The 2-hour TTL means all old
sessions expire naturally.

### Secret not updating

**Symptom:** Fingerprint unchanged after secret patch.

**Cause:** Rolling restart may not have triggered, or pods cached the old
secret.

**Resolution:**

```bash
# Force a rolling restart
kubectl rollout restart deployment/wildside-backend -n wildside

# Or delete pods to force recreation
kubectl delete pods -n wildside -l app.kubernetes.io/name=wildside
```

## Rotation schedule

Recommended rotation schedule:

| Scenario                            | Frequency       |
| ----------------------------------- | --------------- |
| Routine rotation                    | Quarterly       |
| After suspected compromise          | Immediately     |
| After personnel change (key access) | Within 24 hours |
| After security audit finding        | As recommended  |

## Related documentation

- [Backend architecture: Session configuration](../wildside-backend-architecture.md#session-configuration-and-rotation)
- [Execution plan: Signing key rotation](../execplans/backend-phase-1-signing-key-rotation.md)
- [Helm chart values](../../deploy/charts/wildside/values.yaml)
