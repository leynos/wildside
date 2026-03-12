//! Server harness and shared world for adapter guardrails.
//!
//! The harness owns a single-threaded Tokio runtime plus a `LocalSet` because
//! Actix uses `spawn_local` internally. The `WorldFixture` ensures the server
//! is stopped even if a test panics.

use std::cell::RefCell;
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::Arc;

use actix_session::SessionMiddleware;
use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Key, SameSite, time::Duration as CookieDuration};
use actix_web::dev::ServerHandle;
use actix_web::{App, HttpServer, web};
use actix_ws::CloseCode;
use rstest::fixture;
use serde_json::Value;
use tokio::runtime::Runtime;
use tokio::task::LocalSet;

mod harness_defaults;

use self::harness_defaults::{
    create_catalogue_doubles, create_enrichment_provenance_double, create_fixture_user_id,
    create_interests_double, create_offline_and_walk_doubles, create_preferences_doubles,
    create_route_annotations_doubles, create_user_doubles,
};
use crate::doubles::{
    QueueUserOnboarding, RecordingCatalogueRepository, RecordingDescriptorRepository,
    RecordingEnrichmentProvenanceRepository, RecordingLoginService, RecordingOfflineBundleCommand,
    RecordingOfflineBundleQuery, RecordingRouteAnnotationsCommand, RecordingRouteAnnotationsQuery,
    RecordingUserInterestsCommand, RecordingUserPreferencesCommand, RecordingUserPreferencesQuery,
    RecordingUserProfileQuery, RecordingUsersQuery, RecordingWalkSessionCommand,
};
use backend::Trace;
use backend::domain::ports::{FixtureRouteSubmissionService, FixtureWalkSessionQuery};
use backend::inbound::http::admin_enrichment::list_enrichment_provenance as list_enrichment_provenance_handler;
use backend::inbound::http::annotations::{
    get_annotations as get_annotations_handler, update_progress as update_progress_handler,
    upsert_note as upsert_note_handler,
};
use backend::inbound::http::catalogue::{
    get_descriptors as get_descriptors_handler,
    get_explore_catalogue as get_explore_catalogue_handler,
};
use backend::inbound::http::offline::{
    delete_offline_bundle as delete_offline_bundle_handler,
    list_offline_bundles as list_offline_bundles_handler,
    upsert_offline_bundle as upsert_offline_bundle_handler,
};
use backend::inbound::http::preferences::{
    get_preferences as get_preferences_handler, update_preferences as update_preferences_handler,
};
use backend::inbound::http::state::{HttpState, HttpStateExtraPorts, HttpStatePorts};
use backend::inbound::http::users::{
    current_user as current_user_handler, list_users as list_users_handler, login as login_handler,
    update_interests as update_interests_handler,
};
use backend::inbound::http::walk_sessions::create_walk_session as create_walk_session_handler;
use backend::inbound::ws;
use backend::inbound::ws::state::WsState;

pub(crate) struct AdapterWorld {
    pub(crate) runtime: Runtime,
    pub(crate) local: LocalSet,
    pub(crate) base_url: String,
    pub(crate) server: ServerHandle,
    pub(crate) login: RecordingLoginService,
    pub(crate) users: RecordingUsersQuery,
    pub(crate) profile: RecordingUserProfileQuery,
    pub(crate) interests: RecordingUserInterestsCommand,
    pub(crate) preferences: RecordingUserPreferencesCommand,
    pub(crate) preferences_query: RecordingUserPreferencesQuery,
    pub(crate) route_annotations: RecordingRouteAnnotationsCommand,
    pub(crate) route_annotations_query: RecordingRouteAnnotationsQuery,
    pub(crate) catalogue: RecordingCatalogueRepository,
    pub(crate) descriptors: RecordingDescriptorRepository,
    pub(crate) enrichment_provenance: RecordingEnrichmentProvenanceRepository,
    pub(crate) offline_bundles: RecordingOfflineBundleCommand,
    pub(crate) offline_bundles_query: RecordingOfflineBundleQuery,
    pub(crate) walk_sessions: RecordingWalkSessionCommand,
    pub(crate) onboarding: QueueUserOnboarding,
    pub(crate) last_status: Option<u16>,
    pub(crate) last_body: Option<Value>,
    pub(crate) last_cache_control: Option<String>,
    pub(crate) last_trace_id: Option<String>,
    pub(crate) session_cookie: Option<String>,
    pub(crate) last_ws_value: Option<Value>,
    pub(crate) last_ws_close: Option<CloseCode>,
    pub(crate) last_ws_call_count_baseline: Option<usize>,
}

pub(crate) type SharedWorld = Rc<RefCell<AdapterWorld>>;

pub(crate) struct WorldFixture {
    world: SharedWorld,
}

impl WorldFixture {
    pub(crate) fn world(&self) -> SharedWorld {
        self.world.clone()
    }
}

impl Drop for WorldFixture {
    fn drop(&mut self) {
        shutdown(self.world.clone());
    }
}

pub(crate) fn shutdown(world: SharedWorld) {
    // `LocalSet` must be driven on the thread that owns it, so we lock the world
    // while calling `block_on`. The future must not try to lock the world.
    let ctx = world.borrow();
    let server = ctx.server.clone();
    ctx.local.block_on(&ctx.runtime, async move {
        server.stop(true).await;
    });
}

pub(crate) fn with_world_async<R, F>(world: &SharedWorld, operation: impl FnOnce(String) -> F) -> R
where
    F: std::future::Future<Output = R>,
{
    let ctx = world.borrow();
    let base_url = ctx.base_url.clone();
    ctx.local.block_on(&ctx.runtime, operation(base_url))
}

fn test_session_middleware(key: Key) -> SessionMiddleware<CookieSessionStore> {
    SessionMiddleware::builder(CookieSessionStore::default(), key)
        .cookie_name("session".to_owned())
        .cookie_path("/".to_owned())
        .cookie_secure(false)
        .cookie_http_only(true)
        .cookie_content_security(CookieContentSecurity::Private)
        .cookie_same_site(SameSite::Lax)
        .session_lifecycle(PersistentSession::default().session_ttl(CookieDuration::hours(2)))
        .build()
}

async fn spawn_adapter_server(
    http_state: HttpState,
    ws_state: WsState,
) -> Result<(String, ServerHandle), String> {
    let key = Key::generate();
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    let addr = listener.local_addr().map_err(|err| err.to_string())?;

    let http_data = web::Data::new(http_state);
    let ws_data = web::Data::new(ws_state);

    let server = HttpServer::new(move || {
        let api = web::scope("/api/v1")
            .wrap(test_session_middleware(key.clone()))
            .service(login_handler)
            .service(list_users_handler)
            .service(current_user_handler)
            .service(update_interests_handler)
            .service(get_preferences_handler)
            .service(update_preferences_handler)
            .service(get_annotations_handler)
            .service(upsert_note_handler)
            .service(update_progress_handler)
            .service(get_explore_catalogue_handler)
            .service(get_descriptors_handler)
            .service(list_enrichment_provenance_handler)
            .service(list_offline_bundles_handler)
            .service(upsert_offline_bundle_handler)
            .service(delete_offline_bundle_handler)
            .service(create_walk_session_handler);

        App::new()
            .app_data(http_data.clone())
            .app_data(ws_data.clone())
            .wrap(Trace)
            .service(api)
            .service(ws::ws_entry)
    })
    .disable_signals()
    .workers(1)
    .listen(listener)
    .map_err(|err| err.to_string())?
    .run();

    let handle = server.handle();
    actix_web::rt::spawn(server);

    Ok((format!("http://{addr}"), handle))
}

fn create_runtime_and_local() -> (Runtime, LocalSet) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let local = LocalSet::new();

    (runtime, local)
}

struct HttpStateTestDoubles {
    login: RecordingLoginService,
    users: RecordingUsersQuery,
    profile: RecordingUserProfileQuery,
    interests: RecordingUserInterestsCommand,
    preferences: RecordingUserPreferencesCommand,
    preferences_query: RecordingUserPreferencesQuery,
    route_annotations: RecordingRouteAnnotationsCommand,
    route_annotations_query: RecordingRouteAnnotationsQuery,
    catalogue: RecordingCatalogueRepository,
    descriptors: RecordingDescriptorRepository,
    enrichment_provenance: RecordingEnrichmentProvenanceRepository,
    offline_bundles: RecordingOfflineBundleCommand,
    offline_bundles_query: RecordingOfflineBundleQuery,
    walk_sessions: RecordingWalkSessionCommand,
}

fn build_http_state_for_tests(doubles: HttpStateTestDoubles) -> HttpState {
    let HttpStateTestDoubles {
        login,
        users,
        profile,
        interests,
        preferences,
        preferences_query,
        route_annotations,
        route_annotations_query,
        catalogue,
        descriptors,
        enrichment_provenance,
        offline_bundles,
        offline_bundles_query,
        walk_sessions,
    } = doubles;

    HttpState::new_with_extra(
        HttpStatePorts {
            login: Arc::new(login),
            users: Arc::new(users),
            profile: Arc::new(profile),
            interests: Arc::new(interests),
            preferences: Arc::new(preferences),
            preferences_query: Arc::new(preferences_query),
            route_annotations: Arc::new(route_annotations),
            route_annotations_query: Arc::new(route_annotations_query),
            route_submission: Arc::new(FixtureRouteSubmissionService),
            catalogue: Arc::new(catalogue),
            descriptors: Arc::new(descriptors),
        },
        HttpStateExtraPorts {
            offline_bundles: Arc::new(offline_bundles),
            offline_bundles_query: Arc::new(offline_bundles_query),
            enrichment_provenance: Arc::new(enrichment_provenance),
            walk_sessions: Arc::new(walk_sessions),
            walk_sessions_query: Arc::new(FixtureWalkSessionQuery),
        },
    )
}

#[fixture]
pub(crate) fn world() -> WorldFixture {
    let (runtime, local) = create_runtime_and_local();
    let user_id = create_fixture_user_id();
    let (login, users, profile) = create_user_doubles(&user_id);
    let interests = create_interests_double(&user_id);
    let (preferences, preferences_query) = create_preferences_doubles(&user_id);
    let (route_annotations, route_annotations_query) = create_route_annotations_doubles(&user_id);
    let (catalogue, descriptors) = create_catalogue_doubles();
    let enrichment_provenance = create_enrichment_provenance_double();
    let (offline_bundles, offline_bundles_query, walk_sessions) =
        create_offline_and_walk_doubles(&user_id);
    let onboarding = QueueUserOnboarding::new(Vec::new());
    let http_state = build_http_state_for_tests(HttpStateTestDoubles {
        login: login.clone(),
        users: users.clone(),
        profile: profile.clone(),
        interests: interests.clone(),
        preferences: preferences.clone(),
        preferences_query: preferences_query.clone(),
        route_annotations: route_annotations.clone(),
        route_annotations_query: route_annotations_query.clone(),
        catalogue: catalogue.clone(),
        descriptors: descriptors.clone(),
        enrichment_provenance: enrichment_provenance.clone(),
        offline_bundles: offline_bundles.clone(),
        offline_bundles_query: offline_bundles_query.clone(),
        walk_sessions: walk_sessions.clone(),
    });
    let ws_state = crate::ws_support::ws_state(onboarding.clone());

    let (base_url, server) = local
        .block_on(&runtime, async {
            spawn_adapter_server(http_state, ws_state).await
        })
        .expect("server should start");

    let world = Rc::new(RefCell::new(AdapterWorld {
        runtime,
        local,
        base_url,
        server,
        login,
        users,
        profile,
        interests,
        preferences,
        preferences_query,
        route_annotations,
        route_annotations_query,
        catalogue,
        descriptors,
        enrichment_provenance,
        offline_bundles,
        offline_bundles_query,
        walk_sessions,
        onboarding,
        last_status: None,
        last_body: None,
        last_cache_control: None,
        last_trace_id: None,
        session_cookie: None,
        last_ws_value: None,
        last_ws_close: None,
        last_ws_call_count_baseline: None,
    }));

    WorldFixture { world }
}
