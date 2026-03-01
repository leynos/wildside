//! Default test-double builders for the adapter-guardrails harness.

use backend::domain::ports::{
    CreateWalkSessionResponse, DeleteNoteResponse, DeleteOfflineBundleResponse,
    EnrichmentProvenanceRecord, GetOfflineBundleResponse, ListEnrichmentProvenanceResponse,
    ListOfflineBundlesResponse, OfflineBundlePayload, UpdatePreferencesResponse,
    UpdateProgressResponse, UpsertNoteResponse, UpsertOfflineBundleResponse,
    WalkCompletionSummaryPayload, empty_catalogue_and_descriptor_snapshots,
};
use backend::domain::{
    BoundingBox, DisplayName, InterestThemeId, OfflineBundleKind, OfflineBundleStatus,
    RouteAnnotations, RouteNote, RouteProgress, UnitSystem, User, UserId, UserInterests,
    UserPreferences, WalkPrimaryStatDraft, WalkPrimaryStatKind, WalkSecondaryStatDraft,
    WalkSecondaryStatKind, ZoomRange,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::doubles::{
    CatalogueQueryResponse, DeleteNoteCommandResponse, DeleteOfflineBundleCommandResponse,
    DescriptorQueryResponse, EnrichmentProvenanceListResponse, LoginResponse,
    OfflineBundleGetQueryResponse, OfflineBundleListQueryResponse, RecordingCatalogueRepository,
    RecordingDescriptorRepository, RecordingEnrichmentProvenanceRepository, RecordingLoginService,
    RecordingOfflineBundleCommand, RecordingOfflineBundleQuery, RecordingRouteAnnotationsCommand,
    RecordingRouteAnnotationsQuery, RecordingUserInterestsCommand, RecordingUserPreferencesCommand,
    RecordingUserPreferencesQuery, RecordingUserProfileQuery, RecordingUsersQuery,
    RecordingWalkSessionCommand, RouteAnnotationsQueryResponse, UpdateProgressCommandResponse,
    UpsertNoteCommandResponse, UpsertOfflineBundleCommandResponse, UserInterestsResponse,
    UserPreferencesCommandResponse, UserPreferencesQueryResponse, UserProfileResponse,
    UsersResponse, WalkSessionCommandResponse,
};

/// Returns the canonical authenticated user identifier for harness defaults.
///
/// # Examples
///
/// ```rust,ignore
/// let user_id = create_fixture_user_id();
/// assert_eq!(user_id.to_string(), "11111111-1111-1111-1111-111111111111");
/// ```
pub(super) fn create_fixture_user_id() -> UserId {
    UserId::new("11111111-1111-1111-1111-111111111111").expect("fixture user id")
}

/// Parses a UUID fixture value and panics when the fixture is invalid.
///
/// # Examples
///
/// ```rust,ignore
/// let id = fixture_uuid("00000000-0000-0000-0000-000000000101");
/// assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000101");
/// ```
pub(super) fn fixture_uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("fixture uuid")
}

/// Parses an RFC 3339 fixture timestamp into UTC.
///
/// # Examples
///
/// ```rust,ignore
/// let timestamp = fixture_timestamp("2026-02-01T10:00:00Z");
/// assert_eq!(timestamp.to_rfc3339(), "2026-02-01T10:00:00+00:00");
/// ```
fn fixture_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixture timestamp")
        .with_timezone(&Utc)
}

/// Creates the default login, user-list, and profile doubles for a user.
///
/// # Examples
///
/// ```rust,ignore
/// let user_id = create_fixture_user_id();
/// let (_login, _users, _profile) = create_user_doubles(&user_id);
/// ```
pub(super) fn create_user_doubles(
    user_id: &UserId,
) -> (
    RecordingLoginService,
    RecordingUsersQuery,
    RecordingUserProfileQuery,
) {
    let login = RecordingLoginService::new(LoginResponse::Ok(user_id.clone()));
    let users = RecordingUsersQuery::new(UsersResponse::Ok(vec![User::new(
        UserId::new("22222222-2222-2222-2222-222222222222").expect("fixture user id"),
        DisplayName::new("Ada Lovelace").expect("fixture display name"),
    )]));
    let profile = RecordingUserProfileQuery::new(UserProfileResponse::Ok(User::new(
        user_id.clone(),
        DisplayName::new("Ada Lovelace").expect("fixture display name"),
    )));

    (login, users, profile)
}

/// Creates a default interests-command double for the supplied user.
///
/// # Examples
///
/// ```rust,ignore
/// let user_id = create_fixture_user_id();
/// let _interests = create_interests_double(&user_id);
/// ```
pub(super) fn create_interests_double(user_id: &UserId) -> RecordingUserInterestsCommand {
    RecordingUserInterestsCommand::new(UserInterestsResponse::Ok(UserInterests::new(
        user_id.clone(),
        vec![
            InterestThemeId::new("3fa85f64-5717-4562-b3fc-2c963f66afa6")
                .expect("fixture interest theme id"),
        ],
    )))
}

/// Creates default command/query doubles for user preferences.
///
/// # Examples
///
/// ```rust,ignore
/// let user_id = create_fixture_user_id();
/// let (_command, _query) = create_preferences_doubles(&user_id);
/// ```
pub(super) fn create_preferences_doubles(
    user_id: &UserId,
) -> (
    RecordingUserPreferencesCommand,
    RecordingUserPreferencesQuery,
) {
    let preferences = RecordingUserPreferencesCommand::new(UserPreferencesCommandResponse::Ok(
        UpdatePreferencesResponse {
            preferences: UserPreferences::builder(user_id.clone())
                .interest_theme_ids(vec![fixture_uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")])
                .safety_toggle_ids(vec![fixture_uuid("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")])
                .unit_system(UnitSystem::Metric)
                .revision(2)
                .build(),
            replayed: false,
        },
    ));
    let preferences_query = RecordingUserPreferencesQuery::new(UserPreferencesQueryResponse::Ok(
        UserPreferences::builder(user_id.clone())
            .interest_theme_ids(vec![fixture_uuid("cccccccc-cccc-cccc-cccc-cccccccccccc")])
            .safety_toggle_ids(vec![fixture_uuid("dddddddd-dddd-dddd-dddd-dddddddddddd")])
            .unit_system(UnitSystem::Metric)
            .revision(1)
            .build(),
    ));

    (preferences, preferences_query)
}

/// Creates default command/query doubles for route annotations.
///
/// # Examples
///
/// ```rust,ignore
/// let user_id = create_fixture_user_id();
/// let (_command, _query) = create_route_annotations_doubles(&user_id);
/// ```
pub(super) fn create_route_annotations_doubles(
    user_id: &UserId,
) -> (
    RecordingRouteAnnotationsCommand,
    RecordingRouteAnnotationsQuery,
) {
    let route_id = fixture_uuid("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee");
    let note_id = fixture_uuid("ffffffff-ffff-ffff-ffff-ffffffffffff");
    let note = RouteNote::builder(note_id, route_id, user_id.clone())
        .body("First note")
        .revision(1)
        .build();
    let progress = RouteProgress::builder(route_id, user_id.clone())
        .visited_stop_ids(vec![fixture_uuid("99999999-9999-9999-9999-999999999999")])
        .revision(1)
        .build();
    let route_annotations_query =
        RecordingRouteAnnotationsQuery::new(RouteAnnotationsQueryResponse::Ok(RouteAnnotations {
            route_id,
            notes: vec![note.clone()],
            progress: Some(progress.clone()),
        }));
    let route_annotations = RecordingRouteAnnotationsCommand::new(
        UpsertNoteCommandResponse::Ok(UpsertNoteResponse {
            note: note.clone(),
            replayed: false,
        }),
        UpdateProgressCommandResponse::Ok(UpdateProgressResponse {
            progress: progress.clone(),
            replayed: false,
        }),
        DeleteNoteCommandResponse::Ok(DeleteNoteResponse {
            deleted: false,
            replayed: false,
        }),
    );

    (route_annotations, route_annotations_query)
}

/// Creates default catalogue and descriptor repository doubles.
///
/// # Examples
///
/// ```rust,ignore
/// let (_catalogue, _descriptors) = create_catalogue_doubles();
/// ```
pub(super) fn create_catalogue_doubles()
-> (RecordingCatalogueRepository, RecordingDescriptorRepository) {
    let (catalogue_snapshot, descriptor_snapshot) = empty_catalogue_and_descriptor_snapshots();
    let catalogue =
        RecordingCatalogueRepository::new(CatalogueQueryResponse::Ok(catalogue_snapshot));
    let descriptors =
        RecordingDescriptorRepository::new(DescriptorQueryResponse::Ok(descriptor_snapshot));

    (catalogue, descriptors)
}

/// Creates a default enrichment provenance reporting double.
///
/// # Examples
///
/// ```rust,ignore
/// let _provenance = create_enrichment_provenance_double();
/// ```
pub(super) fn create_enrichment_provenance_double() -> RecordingEnrichmentProvenanceRepository {
    RecordingEnrichmentProvenanceRepository::new(EnrichmentProvenanceListResponse::Ok(
        ListEnrichmentProvenanceResponse {
            records: vec![EnrichmentProvenanceRecord {
                source_url: "https://overpass.example/api/interpreter".to_owned(),
                imported_at: fixture_timestamp("2026-02-28T12:00:00Z"),
                bounding_box: [-3.2, 55.9, -3.0, 56.0],
            }],
            next_before: None,
        },
    ))
}

/// Creates default offline bundle and walk-session doubles.
///
/// # Examples
///
/// ```rust,ignore
/// let user_id = create_fixture_user_id();
/// let (_offline_cmd, _offline_query, _walk_cmd) = create_offline_and_walk_doubles(&user_id);
/// ```
pub(super) fn create_offline_and_walk_doubles(
    user_id: &UserId,
) -> (
    RecordingOfflineBundleCommand,
    RecordingOfflineBundleQuery,
    RecordingWalkSessionCommand,
) {
    let bundle_id = fixture_uuid("00000000-0000-0000-0000-000000000101");
    let route_id = fixture_uuid("00000000-0000-0000-0000-000000000202");
    let session_id = fixture_uuid("00000000-0000-0000-0000-000000000501");

    let bundle = OfflineBundlePayload {
        id: bundle_id,
        owner_user_id: Some(user_id.clone()),
        device_id: "ios-iphone-15".to_owned(),
        kind: OfflineBundleKind::Route,
        route_id: Some(route_id),
        region_id: None,
        bounds: BoundingBox::new(-3.2, 55.9, -3.0, 56.0).expect("fixture bounds"),
        zoom_range: ZoomRange::new(11, 15).expect("fixture zoom range"),
        estimated_size_bytes: 4_096,
        created_at: fixture_timestamp("2026-02-01T10:00:00Z"),
        updated_at: fixture_timestamp("2026-02-01T10:00:00Z"),
        status: OfflineBundleStatus::Queued,
        progress: 0.0,
    };

    let offline_command = RecordingOfflineBundleCommand::new(
        UpsertOfflineBundleCommandResponse::Ok(UpsertOfflineBundleResponse {
            bundle: bundle.clone(),
            is_replayed: false,
        }),
        DeleteOfflineBundleCommandResponse::Ok(DeleteOfflineBundleResponse {
            bundle_id,
            is_replayed: false,
        }),
    );
    let offline_query = RecordingOfflineBundleQuery::new(
        OfflineBundleListQueryResponse::Ok(ListOfflineBundlesResponse {
            bundles: vec![bundle.clone()],
        }),
        OfflineBundleGetQueryResponse::Ok(GetOfflineBundleResponse { bundle }),
    );
    let walk_command = RecordingWalkSessionCommand::new(WalkSessionCommandResponse::Ok(
        CreateWalkSessionResponse {
            session_id,
            completion_summary: Some(WalkCompletionSummaryPayload {
                session_id,
                user_id: user_id.clone(),
                route_id,
                started_at: fixture_timestamp("2026-02-01T11:00:00Z"),
                ended_at: fixture_timestamp("2026-02-01T11:40:00Z"),
                primary_stats: vec![WalkPrimaryStatDraft {
                    kind: WalkPrimaryStatKind::Distance,
                    value: 1_234.0,
                }],
                secondary_stats: vec![WalkSecondaryStatDraft {
                    kind: WalkSecondaryStatKind::Energy,
                    value: 120.0,
                    unit: Some("kcal".to_owned()),
                }],
                highlighted_poi_ids: vec![fixture_uuid("00000000-0000-0000-0000-000000000503")],
            }),
        },
    ));

    (offline_command, offline_query, walk_command)
}
