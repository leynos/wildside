plugin "terraform" {
  enabled = true
  version = "0.10.0"
  source  = "github.com/terraform-linters/tflint-ruleset-terraform"
  preset  = "recommended"
}

# After running `tflint --init`, commit `.tflint.hcl.lock` to version control.
