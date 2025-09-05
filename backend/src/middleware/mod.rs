//! Request middleware.
//!
//! Purpose: Define middleware components for request lifecycle concerns such as
//! tracing and authentication.

pub mod trace;

pub use trace::Trace;
