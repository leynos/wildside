//! Unit tests for split-boundary page capping behaviour.

use chrono::TimeZone;
use rstest::rstest;
use uuid::Uuid;

use super::*;

fn row(id: Uuid, imported_at: DateTime<Utc>) -> EnrichmentProvenanceRow {
    EnrichmentProvenanceRow {
        id,
        source_url: "https://overpass.example/api/interpreter".to_owned(),
        imported_at,
        bounds_min_lng: -3.2,
        bounds_min_lat: 55.9,
        bounds_max_lng: -3.0,
        bounds_max_lat: 56.0,
        created_at: imported_at,
    }
}

#[rstest]
fn split_boundary_caps_page_and_advances_cursor_inside_bucket() {
    let boundary = Utc
        .with_ymd_and_hms(2026, 3, 1, 12, 0, 0)
        .single()
        .expect("valid timestamp");
    let newer = Utc
        .with_ymd_and_hms(2026, 3, 1, 12, 1, 0)
        .single()
        .expect("valid timestamp");

    let id_a = Uuid::from_u128(10);
    let id_b = Uuid::from_u128(9);
    let id_c = Uuid::from_u128(8);
    let id_d = Uuid::from_u128(7);

    // First query page (limit + 1) includes one newer row and two boundary rows.
    let rows = vec![
        row(Uuid::from_u128(11), newer),
        row(id_a, boundary),
        row(id_b, boundary),
    ];
    let boundary_rows = vec![
        row(id_a, boundary),
        row(id_b, boundary),
        row(id_c, boundary),
        row(id_d, boundary),
    ];

    let (page_rows, cursor) = DieselEnrichmentProvenanceRepository::split_boundary_page_rows(
        rows,
        boundary_rows,
        3,
        boundary,
    );

    assert_eq!(
        page_rows.len(),
        3,
        "split boundary path must enforce page limit"
    );
    assert_eq!(page_rows[1].id, id_a);
    assert_eq!(page_rows[2].id, id_b);

    let cursor = cursor.expect("cursor should be present");
    assert_eq!(cursor.imported_at, boundary);
    assert_eq!(
        cursor.id, id_b,
        "cursor id must track last returned row inside boundary bucket"
    );
}
