plugin "terraform" {
  enabled = true
  # Pin official Terraform ruleset plugin and enforce recommended checks
  version = "0.10.0"
  source  = "github.com/terraform-linters/tflint-ruleset-terraform"
  preset  = "recommended"
}

