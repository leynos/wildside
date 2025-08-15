//! Domain data models.
//!
//! Purpose: Define strongly typed domain entities used by the API and
//! persistence layers. Keep types immutable and document invariants and
//! serialisation contracts (serde) in each type's Rustdoc.
//!
//! Public surface:
//! - user::User — domain user identity and display name.

pub mod user;
pub use self::user::User;
