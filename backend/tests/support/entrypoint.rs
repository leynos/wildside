/**
Declares the test-local `support` module used by BDD and integration test
binaries.

This file is pulled into each test binary with `include!`, so its contents are
pasted inline rather than compiled as a module of their own. That is why this
purpose documentation is an *outer* doc comment (`/** */`) attached to the
macro rather than an inner `//!` module doc: an inner doc comment at an
`include!` site is rejected by rustc with E0753.

`declare_test_support!` expands to a `support` submodule that re-exports the
shared helpers in `../support/mod.rs` and conditionally wires in additional
support submodules (for example `atexit_cleanup`, `cluster_skip`,
`embedded_postgres`) by name, so each test binary only compiles the support
code it actually uses.
*/
macro_rules! declare_test_support {
    ($($module:ident),+ $(,)?) => {
        mod support {
            //! Test-local view of shared support helpers.
            #[path = "../support/mod.rs"]
            mod shared;
            pub use shared::*;
            $(declare_test_support!(@module $module);)+
        }
    };
    (@module atexit_cleanup) => {
        #[path = "../support/atexit_cleanup.rs"]
        pub mod atexit_cleanup;
    };
    (@module cluster_skip) => {
        #[path = "../support/cluster_skip.rs"]
        pub mod cluster_skip;
    };
    (@module embedded_postgres) => {
        #[path = "../support/embedded_postgres.rs"]
        pub mod embedded_postgres;
    };
    (@module example_data_seeding_world) => {
        #[path = "../support/example_data_seeding_world.rs"]
        pub mod example_data_seeding_world;
    };
    (@module fixture_auth) => {
        #[path = "../support/fixture_auth.rs"]
        pub mod fixture_auth;
    };
    (@module flow_helpers) => {
        #[path = "../support/flow_helpers.rs"]
        pub mod flow_helpers;
    };
    (@module profile_interests) => {
        #[path = "../support/profile_interests.rs"]
        pub mod profile_interests;
    };
    (@module redis) => {
        #[path = "../support/redis.rs"]
        pub mod redis;
    };
    (@module redis_skip) => {
        #[path = "../support/redis_skip.rs"]
        pub mod redis_skip;
    };
    (@module seed_connection_helpers) => {
        #[path = "../support/seed_connection_helpers.rs"]
        pub mod seed_connection_helpers;
    };
    (@module seed_helpers) => {
        #[path = "../support/seed_helpers.rs"]
        pub mod seed_helpers;
    };
    (@module session_middleware) => {
        #[path = "../support/session_middleware.rs"]
        pub mod session_middleware;
    };
    (@module table_helpers) => {
        #[path = "../support/table_helpers.rs"]
        pub mod table_helpers;
    };
}
