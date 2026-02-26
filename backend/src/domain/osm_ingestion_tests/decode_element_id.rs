//! Unit coverage for encoded OSM element identifier decoding.

use rstest::rstest;

use super::*;

#[rstest]
#[case::node(10, "node", 10)]
#[case::way(WAY_ID_PREFIX | 11, "way", 11)]
#[case::relation(RELATION_ID_PREFIX | 12, "relation", 12)]
fn decode_element_id_decodes_type_prefixes(
    #[case] encoded_id: u64,
    #[case] expected_type: &str,
    #[case] expected_id: i64,
) {
    let (element_type, element_id) = decode_element_id(encoded_id).expect("decode should work");
    assert_eq!(element_type, expected_type);
    assert_eq!(element_id, expected_id);
}

#[rstest]
fn decode_element_id_interprets_high_bit_as_relation_prefix() {
    let encoded = i64::MAX as u64 + 1;
    let (element_type, element_id) =
        decode_element_id(encoded).expect("high-bit element id should decode");

    assert_eq!(element_type, "relation");
    assert_eq!(element_id, 0);
}
