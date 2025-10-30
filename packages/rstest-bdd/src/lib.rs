//! Minimal stand-in for the `rstest-bdd` crate used in tests.
//!
//! The real crate currently requires nightly compiler features. This shim keeps
//! the ergonomic attribute syntax (`#[given]`, `#[when]`, `#[then]`) but simply
//! passes the annotated functions through unchanged so they can be composed
//! manually inside tests. Behavioural semantics are provided by the tests
//! themselves, which call the functions explicitly to express scenarios.

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn given(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn when(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn then(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
