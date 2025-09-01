# Using Cloudflare DNS with OpenTofu

## 1. Set Up the Cloudflare Provider

Your `provider "cloudflare"` block configures authentication and connects OpenTofu to Cloudflare. Use environment variables for credentials to avoid leaking secrets:

```hcl
provider "cloudflare" {
  api_token = var.cloudflare_api_token
}
```

Set credentials securely:

```bash
export CLOUDFLARE_API_TOKEN="your-token"
export TF_VAR_cloudflare_api_token="$CLOUDFLARE_API_TOKEN"
```

This ensures that sensitive data never lands in your repository.

## 2. Define and Manage DNS Zones

Create or reference a Cloudflare DNS zone with a `cloudflare_zone` resource:

```hcl
resource "cloudflare_zone" "example" {
  name = "example.com"
  type = "full"
}
```

This gives you access to the `zone_id` required for record management.

## 3. Configure DNS Records

Use `cloudflare_record` resources to define DNS entries:

```hcl
resource "cloudflare_record" "www" {
  zone_id = cloudflare_zone.example.id
  name    = "www"
  type    = "A"
  content = "203.0.113.10"
  ttl     = 3600
  proxied = true
}
```

This creates a proxied A record pointing to `203.0.113.10`.

## 4. Automate Bulk Records with Variables

For repeated or multiple record definitions, leverage `for_each` or `count` with a structured variable:

```hcl
variable "dns_records" {
  type = list(object({
    name    = string
    type    = string
    content = string
    ttl     = number
    proxied = optional(bool, false)
  }))
}

resource "cloudflare_record" "bulk" {
  for_each = { for r in var.dns_records : r.name => r }

  zone_id = var.cloudflare_zone_id
  name    = each.value.name
  type    = each.value.type
  content = each.value.content
  ttl     = each.value.ttl
  proxied = each.value.proxied
}
```

Define `dns_records` in your `terraform.tfvars`:

```hcl
dns_records = [
  { name = "app.example.com", type = "A", content = "192.168.1.1", ttl = 3600, proxied = true },
  { name = "api.example.com", type = "CNAME", content = "example.com", ttl = 300 }
]
```

This keeps your configuration DRY and maintainable.

## 5. Import Existing DNS Records

When onboarding existing DNS infrastructure into OpenTofu, you need Cloudflare’s record ID (not just name) to import:

1. Retrieve via API:

   ```bash
   export CLOUDFLARE_API_TOKEN="…"
   ZONE_ID=$(curl -s -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
     https://api.cloudflare.com/client/v4/zones?name=example.com | jq -r '.result[0].id')

   curl -s -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
     "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records?name=www.example.com&type=A" | jq -r '.result[0].id'
   ```

2. Then import:

   ```bash
   tofu import cloudflare_record.example DNS_ID
   ```

This aligns your existing records with your IaC workflow.

## 6. Example Project Structure

```plaintext
infra/
├── main.tf
├── variables.tf
├── terraform.tfvars
├── outputs.tf
├── provider.tf
```

- **`provider.tf`** – Sets Cloudflare provider and auth via variables.
- **`variables.tf`** – Defines `dns_records`, `cloudflare_zone_id`, etc.
- **`main.tf`** – Contains `cloudflare_zone` and `cloudflare_record` blocks (static or dynamic).
- **`outputs.tf`** – Outputs useful values like `name_servers`.
- **`terraform.tfvars`** – Specifies your actual values: zone name, token, record definitions.

## 7. Workflow Quick Hit List

1. **Init**: `tofu init`
2. **Preview**: `tofu plan`
3. **Apply**: `tofu apply -auto-approve`
4. **Observe**: Check state changes and Dashboard results
5. **Import** (if migrating): Use the API to find record IDs, then `tofu import`
6. **Version Control**: Store in Git, exclude secrets

## Additional Levers & Advanced Practices

- Refer to the Filador blog for integrating DNS, WAF, mTLS, Pages—all with OpenTofu and Cloudflare. It offers rich sample code for elevated use cases.

- Cloudflare’s Terraform provider supports advanced modularisation. Use the module registry and example repos for better modular design.

### Summary Table

| Step      | Description                                 |
| --------- | ------------------------------------------- |
| Provider  | Set up securely via environment variables   |
| Zone      | Define or reference Cloudflare DNS zone     |
| Record    | Create DNS entries, dynamic via `for_each`  |
| Import    | Migrate existing records using API + import |
| Structure | Organise by tf files, use version control   |
| Advanced  | Extend with WAF, mTLS, modules as needed    |

Let me know if you'd like actual module scaffolding or integration examples with CI/CD systems.
