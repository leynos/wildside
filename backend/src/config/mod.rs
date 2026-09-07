//! Process-configuration adapters.
//!
//! Every module here owns one configuration boundary: the variable names it
//! reads, the parsing of their string values, and a single process-backed
//! reader that application composition injects. Domain and service code
//! receives the resulting values, never the environment.
//!
//! See "Environment seams" in `docs/developers-guide.md` for the rule that
//! selects a seam shape.

pub mod idempotency;
