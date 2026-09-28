//! Generated canonicalization properties, grouped by invariant.
//!
//! Key-order permutation is vacuous with this crate's BTreeMap-backed
//! `serde_json::Map` configuration. Geographic validity is outside this
//! cache-key contract. Negative-control evidence is recorded in
//! `docs/execplans/backend-5-1-4a-cache-key-canonicalization-property-tests.md`.

mod canonical_arrays;
mod coordinates;
mod normalization;
mod single_leaf_edits;
mod strategies;
mod strategy_coverage;
