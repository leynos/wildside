//! Property tests for the `example-data` crate.
//!
//! Properties are grouped in child modules by invariant, following the
//! repository's Rust property-testing conventions.

// Integration-test crate roots resolve `mod` children beside this file, so the
// grouped layout needs an explicit path.
#[path = "properties/seed_names.rs"]
mod seed_names;
