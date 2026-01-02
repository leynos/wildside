//! Unit tests for route annotation domain types.

use super::*;
use rstest::rstest;

#[rstest]
fn route_note_new_sets_revision_to_one() {
    let note = RouteNote::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        UserId::random(),
        RouteNoteContent::new("Test note"),
    );

    assert_eq!(note.revision, 1);
    assert!(note.poi_id.is_none());
}

#[rstest]
fn route_note_new_with_poi() {
    let poi_id = Uuid::new_v4();
    let note = RouteNote::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        UserId::random(),
        RouteNoteContent::new("Note on POI").with_poi(poi_id),
    );

    assert_eq!(note.poi_id, Some(poi_id));
}

#[rstest]
fn route_note_builder() {
    let id = Uuid::new_v4();
    let route_id = Uuid::new_v4();
    let poi_id = Uuid::new_v4();
    let user_id = UserId::random();

    let note = RouteNote::builder(id, route_id, user_id.clone())
        .poi_id(poi_id)
        .body("Builder note")
        .revision(5)
        .build();

    assert_eq!(note.id, id);
    assert_eq!(note.route_id, route_id);
    assert_eq!(note.poi_id, Some(poi_id));
    assert_eq!(note.user_id, user_id);
    assert_eq!(note.body, "Builder note");
    assert_eq!(note.revision, 5);
}

#[rstest]
fn route_progress_new_initialises_empty() {
    let route_id = Uuid::new_v4();
    let user_id = UserId::random();
    let progress = RouteProgress::new(route_id, user_id.clone());

    assert_eq!(progress.route_id, route_id);
    assert_eq!(progress.user_id, user_id);
    assert!(progress.visited_stop_ids().is_empty());
    assert_eq!(progress.revision, 1);
}

#[rstest]
fn route_progress_has_visited() {
    let stop1 = Uuid::new_v4();
    let stop2 = Uuid::new_v4();
    let stop3 = Uuid::new_v4();

    let progress = RouteProgress::builder(Uuid::new_v4(), UserId::random())
        .visited_stop_ids(vec![stop1, stop2])
        .build();

    assert!(progress.has_visited(&stop1));
    assert!(progress.has_visited(&stop2));
    assert!(!progress.has_visited(&stop3));
}

#[rstest]
#[case::empty(0, 10, 0.0)]
#[case::partial(3, 10, 30.0)]
#[case::complete(10, 10, 100.0)]
#[case::zero_total(5, 0, 0.0)]
fn route_progress_completion_percent(
    #[case] visited: usize,
    #[case] total: usize,
    #[case] expected: f64,
) {
    let visited_ids: Vec<Uuid> = (0..visited).map(|_| Uuid::new_v4()).collect();
    let progress = RouteProgress::builder(Uuid::new_v4(), UserId::random())
        .visited_stop_ids(visited_ids)
        .build();

    let percent = progress.completion_percent(total);
    assert!((percent - expected).abs() < f64::EPSILON);
}

#[rstest]
fn route_progress_builder() {
    let route_id = Uuid::new_v4();
    let user_id = UserId::random();
    let stops = vec![Uuid::new_v4(), Uuid::new_v4()];

    let progress = RouteProgress::builder(route_id, user_id.clone())
        .visited_stop_ids(stops.clone())
        .revision(3)
        .build();

    assert_eq!(progress.route_id, route_id);
    assert_eq!(progress.user_id, user_id);
    assert_eq!(progress.visited_stop_ids(), stops.as_slice());
    assert_eq!(progress.revision, 3);
}

#[rstest]
fn route_progress_hashset_synced_with_vec() {
    let stop1 = Uuid::new_v4();
    let stop2 = Uuid::new_v4();
    let stop3 = Uuid::new_v4();

    let progress = RouteProgress::builder(Uuid::new_v4(), UserId::random())
        .visited_stop_ids(vec![stop1, stop2, stop3])
        .build();

    // Verify that has_visited (using HashSet) matches visited_stop_ids (Vec)
    for stop_id in progress.visited_stop_ids() {
        assert!(progress.has_visited(stop_id));
    }

    // Verify count matches
    assert_eq!(progress.visited_stop_ids().len(), 3);

    // Verify a non-existent stop is not found
    let other_stop = Uuid::new_v4();
    assert!(!progress.has_visited(&other_stop));
}
