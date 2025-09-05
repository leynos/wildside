plugin "terraform" {
  enabled = true
  # Pin official Terraform ruleset plugin at v0.10.0 and enforce recommended preset
  version = "0.10.0"
  source  = "github.com/terraform-linters/tflint-ruleset-terraform"
  preset  = "recommended"
}

