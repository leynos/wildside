config {
  call_module_type = "all"
  force            = true
}

plugin "terraform" {
  enabled = true
  version = "0.10.0"
  source  = "github.com/terraform-linters/tflint-ruleset-terraform"
  preset  = "recommended"
}

rule "terraform_documented_variables" { enabled = true }
rule "terraform_documented_outputs"   { enabled = true }
rule "terraform_typed_variables"      { enabled = true }

rule "terraform_naming_convention" {
  enabled = true
  format  = "snake_case"
}

rule "terraform_unused_declarations" { enabled = true }
