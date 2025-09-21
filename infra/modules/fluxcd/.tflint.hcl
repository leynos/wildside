config {
  call_module_type    = "all"
  force               = true
  disabled_by_default = true
}

plugin "terraform" {
  enabled = true
  version = "0.10.0"
  source  = "github.com/terraform-linters/tflint-ruleset-terraform"
  preset  = "recommended"
}

# Temporary community OpenTofu ruleset until the upstream terraform-linters
# ruleset publishes public releases. Tracks v0.1.7.
plugin "tofu" {
  enabled = true
  version = "0.1.7"
  source  = "github.com/calxus/tflint-ruleset-tofu"
}

rule "terraform_documented_variables" { enabled = true }
rule "terraform_documented_outputs"   { enabled = true }
rule "terraform_typed_variables"      { enabled = true }

rule "terraform_naming_convention" {
  enabled = true
  format  = "snake_case"
}

rule "terraform_unused_declarations" { enabled = true }
