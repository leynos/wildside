terraform {
  required_version = ">= 1.9.0, < 2.0.0"

  # This module is a pure orchestration layer that invokes child modules.
  # Provider requirements are inherited from the child modules; no additional
  # providers are required at this level.
}
