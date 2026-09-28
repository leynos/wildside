//! Properties ensuring that material single-leaf edits remain observable.

use proptest::prelude::*;

use super::strategies::payload_pair_with_single_leaf_edit;
use crate::domain::ports::RouteCacheKey;

proptest! {
    #[test]
    fn edited_leaves_produce_distinct_keys((first, second) in payload_pair_with_single_leaf_edit()) {
        let first_key = RouteCacheKey::for_route_request(&first);
        let second_key = RouteCacheKey::for_route_request(&second);

        prop_assert!(first_key.is_ok(), "valid generated payload: {:?}", first_key.as_ref().err());
        prop_assert!(second_key.is_ok(), "valid edited payload: {:?}", second_key.as_ref().err());
        prop_assert_ne!(first_key.ok(), second_key.ok());
    }
}
