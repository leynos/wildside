# Vault appliance bootstrap process

This guide explains how to initialise and maintain the DigitalOcean-hosted Vault
appliance with `scripts/bootstrap_vault_appliance.py`. The helper discovers the
appliance droplet, verifies Vault is running, initialises and unseals the
cluster when needed, enables the KV v2 engine, and provisions the AppRole used
by the DOKS deployment workflow. Re-running the script is safe: existing
configuration is reused whenever possible.

## Prerequisites

Before running the helper ensure the following tools are installed and
authenticated on the workstation or CI runner executing the script:

- `doctl` logged in with access to the target DigitalOcean account.
- `vault` CLI able to reach the appliance network location.
- SSH access to the droplet using the user supplied via `--ssh-user` (defaults to
  `root`).
- The Vault CA bundle and server credentials exported from the
  `vault_appliance` OpenTofu module outputs. Save the CA certificate to disk so
  it can be supplied through `--ca-cert-path`.

The DigitalOcean Secrets Manager namespace identified by `--secret-prefix` must
exist. The script will create or update individual secrets inside that
namespace.

## Running the script

The helper may be launched directly thanks to the embedded [`uv`](https://github.com/astral-sh/uv)
metadata:

```sh
./scripts/bootstrap_vault_appliance.py \
  --environment dev \
  --droplet-tag vault-dev \
  --secret-prefix dev-vault \
  --vault-address https://vault.dev.example:8200 \
  --ca-cert-path /secure/path/vault-ca.pem
```

Key options:

- `--environment` and `--secret-prefix` scope the DigitalOcean Secrets Manager
  entries used to persist unseal keys, the root token, and AppRole credentials.
- `--droplet-tag` locates the appliance droplet. The helper aborts if multiple
  droplets share the tag to avoid operating on the wrong instance.
- `--vault-address` overrides the Vault API endpoint used by the CLI. When not
  provided, the helper derives `https://<droplet-ip>:8200` automatically.
- `--ca-cert-path` writes the certificate bundle to `VAULT_CACERT` so the Vault
  CLI trusts the appliance's TLS certificate. Omit the flag only when the CA is
  already trusted by the host system.
- `--mount-path`, `--approle-name`, and `--policy-name` customise the KV engine
  path and DOKS AppRole naming.

All options accept the values documented in `scripts/bootstrap_vault_appliance.py
--help`.

## Secrets management

On the first run the helper:

1. Initialises Vault with the requested number of key shares and threshold.
2. Stores each unseal share and the generated root token inside the secrets
   manager under `<secret-prefix>-unseal-N` and `<secret-prefix>-root-token`.
3. Enables the KV v2 engine and writes the AppRole policy before generating a
   role ID and secret ID. These are saved as `<secret-prefix>-role-id` and
   `<secret-prefix>-secret-id`.

Subsequent executions read the stored secrets, unseal Vault if necessary, and
update the AppRole credentials in-place. Failed operations (for example missing
unseal shares or an unhealthy systemd unit) raise descriptive errors without
modifying the appliance.

## Troubleshooting

- **`Vault remains sealed`** – verify that all unseal secrets exist in the
  DigitalOcean Secrets Manager namespace. Missing shares cause the helper to
  abort before any unseal attempts.
- **`certificate verify failed`** – ensure the CA bundle path supplied via
  `--ca-cert-path` matches the file exported from the module outputs and that
  the executing user can read it.
- **`No droplets tagged`** – confirm the OpenTofu module outputs include the tag
  referenced by `--droplet-tag` and that the droplet is active in the target
  account and region.

For advanced usage, including integrating the helper in GitHub Actions, see the
architecture notes in `docs/cloud-native-ephemeral-previews.md`.
