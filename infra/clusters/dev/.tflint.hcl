config {
  # The `module` setting enables deep checking of modules.
  # This is important for catching issues in your reusable modules.
  call_module_type = "all"
  # TFLint v0.54+ renamed the old `module` flag to `call_module_type`, so
  # enabling `all` matches the "deep module" behaviour described above.
  # The `force` setting causes TFLint to exit with a non-zero status
  # if any issues are found. This is essential for failing CI builds.
  force = true
  # The `disabled_by_default` setting enables all rules that are
  # disabled by default. This is a good way to discover new rules
  # that might be useful for your project.
  disabled_by_default = true
}

# The `plugin` block enables and configures TFLint plugins.
# In this case, we are enabling the `digitalocean` plugin.
plugin "digitalocean" {
  enabled = true
  version = "0.1.1"
  source  = "github.com/terraform-linters/tflint-ruleset-digitalocean"
}

# The `rule` block enables and configures individual rules.
# The following rules are from the `terraform` ruleset and are
# disabled by default.

# This rule enforces that all variables have a `description`.
# This is good practice for making your modules easier to understand.
rule "terraform_documented_variables" {
  enabled = true
}

# This rule enforces that all outputs have a `description`.
# This is good practice for making your modules easier to use.
rule "terraform_documented_outputs" {
  enabled = true
}

# This rule enforces that all variables have a `type`.
# This helps to prevent errors caused by incorrect variable types.
rule "terraform_typed_variables" {
  enabled = true
}

# This rule enforces a consistent naming convention for all
# resources, variables, and outputs. This makes your code
# more readable and easier to maintain.
rule "terraform_naming_convention" {
  enabled = true
  format  = "snake_case"
}

# This rule warns about unused declarations in your code.
# This helps to keep your code clean and free of clutter.
rule "terraform_unused_declarations" {
  enabled = true
}

plugin "kubernetes" {
  enabled = true
  version = "0.3.1"
  source  = "github.com/terraform-linters/tflint-ruleset-kubernetes"
}

plugin "helm" {
  enabled = true
  version = "0.1.2"
  source  = "github.com/terraform-linters/tflint-ruleset-helm"
}

plugin "cloudflare" {
  enabled = true
  version = "0.1.2"
  source  = "github.com/terraform-linters/tflint-ruleset-cloudflare"
}
