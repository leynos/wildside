//! Internal validation predicates for OSM ingestion value-object construction.

pub(super) fn is_valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[rustfmt::skip]
pub(super) fn valid_longitude(value: f64) -> bool { value.is_finite() && (-180.0..=180.0).contains(&value) }

#[rustfmt::skip]
pub(super) fn valid_latitude(value: f64) -> bool { value.is_finite() && (-90.0..=90.0).contains(&value) }
