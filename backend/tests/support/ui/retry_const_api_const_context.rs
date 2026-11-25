//! Compile-pass fixture: the retry API's `const fn` stays usable in a
//! constant context.
//!
//! `MIDPOINT` is evaluated by the compiler rather than at run time. If
//! `JitterSample::midpoint` loses its `const` qualifier, or its signature
//! stops resolving here, trybuild reports the fixture as a failed compilation.
//! The constant is read in `main` so the fixture itself compiles without
//! unused-item warnings.

use backend::domain::jobs::retry::JitterSample;

const MIDPOINT: JitterSample = JitterSample::midpoint();

fn main() {
    let _ = MIDPOINT;
}
