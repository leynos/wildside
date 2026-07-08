// Compile-fail fixture: an unregistered module name must be rejected by
// `declare_test_support!`'s `@module` guard at macro-expansion time rather than
// silently expanding to nothing. This protects the module-wiring macro so a
// typo'd or unsupported `@module` name fails at compile time instead of only
// surfacing as a downstream BDD binary failure.

include!("../entrypoint.rs");

fn main() {}

declare_test_support!(@module not_a_real_module);
