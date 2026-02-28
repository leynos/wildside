//! Builders for HTTP state ports and repository-backed service pairs.

use std::sync::Arc;

use actix_web::web;
use async_trait::async_trait;

use backend::domain::ports::{
    CatalogueRepository, DescriptorRepository, FixtureCatalogueRepository,
    FixtureDescriptorRepository, FixtureLoginService, FixtureOfflineBundleCommand,
    FixtureOfflineBundleQuery, FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery,
    FixtureUserInterestsCommand, FixtureUserPreferencesCommand, FixtureUserPreferencesQuery,
    FixtureUserProfileQuery, FixtureUsersQuery, FixtureWalkSessionCommand, FixtureWalkSessionQuery,
    LoginService, OfflineBundleCommand, OfflineBundleQuery, RouteAnnotationsCommand,
    RouteAnnotationsQuery, RouteSubmissionService, UserPreferencesCommand, UserPreferencesQuery,
    UserRepository, UsersQuery, WalkSessionCommand, WalkSessionQuery,
};
use backend::domain::{
    Error, LoginCredentials, OfflineBundleCommandService, OfflineBundleQueryService,
    RouteAnnotationsService, User, UserId, UserPreferencesService, WalkSessionCommandService,
    WalkSessionQueryService,
};
use backend::inbound::http::state::{HttpState, HttpStateExtraPorts, HttpStatePorts};
use backend::outbound::persistence::DieselIdempotencyRepository;
use backend::outbound::persistence::{
    DbPool, DieselCatalogueRepository, DieselDescriptorRepository, DieselOfflineBundleRepository,
    DieselRouteAnnotationRepository, DieselUserPreferencesRepository, DieselUserRepository,
    DieselWalkSessionRepository,
};

use super::ServerConfig;

/// Build a command/query service pair using real services when a pool is
/// available, otherwise using fixture implementations.
fn build_service_pair<Pool, S, Cmd, Query, MakeService, Cast>(
    pool: &Option<Pool>,
    make_service: MakeService,
    fixtures: (Arc<Cmd>, Arc<Query>),
    cast: Cast,
) -> (Arc<Cmd>, Arc<Query>)
where
    S: 'static,
    Cmd: ?Sized + 'static,
    Query: ?Sized + 'static,
    MakeService: FnOnce(&Pool) -> S,
    Cast: FnOnce(Arc<S>) -> (Arc<Cmd>, Arc<Query>),
{
    match pool {
        Some(pool) => {
            let service = Arc::new(make_service(pool));
            cast(service)
        }
        None => fixtures,
    }
}

/// Helper to construct a service that depends on both a domain repository and
/// an idempotency repository, avoiding duplication of `Arc` wrapping.
fn build_idempotent_service<R, S>(
    pool: &DbPool,
    make_repo: impl FnOnce(DbPool) -> R,
    make_service: impl FnOnce(Arc<R>, Arc<DieselIdempotencyRepository>) -> S,
) -> S
where
    R: 'static,
{
    let repo = Arc::new(make_repo(pool.clone()));
    let idempotency_repo = Arc::new(DieselIdempotencyRepository::new(pool.clone()));
    make_service(repo, idempotency_repo)
}

/// Type alias for a function pointer that takes `Arc<S>` and returns a pair of
/// service trait/object `Arc`s (`Arc<Cmd>`, `Arc<Query>`) used to cast and
/// construct command and query services.
type ServiceCast<S, Cmd, Query> = fn(Arc<S>) -> (Arc<Cmd>, Arc<Query>);

/// Struct holding prepared fixtures (`fixtures: (Arc<Cmd>, Arc<Query>)`) and
/// the cast function (`cast: ServiceCast<S, Cmd, Query>`) to produce
/// command/query service pairs for tests or initialization.
struct ServicePairFactory<S, Cmd: ?Sized, Query: ?Sized> {
    fixtures: (Arc<Cmd>, Arc<Query>),
    cast: ServiceCast<S, Cmd, Query>,
}

const FIXTURE_LOGIN_USERNAME: &str = "admin";
const FIXTURE_LOGIN_PASSWORD: &str = "password";
const FIXTURE_LOGIN_USER_ID: &str = "123e4567-e89b-12d3-a456-426614174000";

/// Database-backed login/users adapter.
///
/// Login keeps the current fixture credential contract until credential
/// persistence lands, while users lookup reads from the SQL-backed repository.
#[derive(Clone)]
struct DieselLoginUsersAdapter {
    users: Arc<DieselUserRepository>,
}

impl DieselLoginUsersAdapter {
    fn new(pool: DbPool) -> Self {
        Self {
            users: Arc::new(DieselUserRepository::new(pool)),
        }
    }
}

#[async_trait]
impl LoginService for DieselLoginUsersAdapter {
    async fn authenticate(&self, credentials: &LoginCredentials) -> Result<UserId, Error> {
        if credentials.username() == FIXTURE_LOGIN_USERNAME
            && credentials.password() == FIXTURE_LOGIN_PASSWORD
        {
            UserId::new(FIXTURE_LOGIN_USER_ID)
                .map_err(|err| Error::internal(format!("invalid fixture user id: {err}")))
        } else {
            Err(Error::unauthorized("invalid credentials"))
        }
    }
}

#[async_trait]
impl UsersQuery for DieselLoginUsersAdapter {
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error> {
        let user = self
            .users
            .find_by_id(authenticated_user)
            .await
            .map_err(|err| Error::internal(format!("users query failed: {err}")))?;
        Ok(user.into_iter().collect())
    }
}

fn build_login_users_pair_with_pool<Pool, Service>(
    pool: &Option<Pool>,
    make_service: impl FnOnce(&Pool) -> Service,
) -> (Arc<dyn LoginService>, Arc<dyn UsersQuery>)
where
    Service: LoginService + UsersQuery + 'static,
{
    build_service_pair(
        pool,
        make_service,
        (
            Arc::new(FixtureLoginService) as Arc<dyn LoginService>,
            Arc::new(FixtureUsersQuery) as Arc<dyn UsersQuery>,
        ),
        |service| {
            (
                service.clone() as Arc<dyn LoginService>,
                service as Arc<dyn UsersQuery>,
            )
        },
    )
}

fn build_login_users_pair(config: &ServerConfig) -> (Arc<dyn LoginService>, Arc<dyn UsersQuery>) {
    build_login_users_pair_with_pool(&config.db_pool, |pool| {
        DieselLoginUsersAdapter::new(pool.clone())
    })
}

macro_rules! build_idempotent_pair {
    (
        $fn_name:ident,
        $cmd_trait:ty,
        $query_trait:ty,
        $repo_ctor:expr,
        $service_ctor:expr,
        $fixture_cmd:path,
        $fixture_query:path
    ) => {
        fn $fn_name(config: &ServerConfig) -> (Arc<$cmd_trait>, Arc<$query_trait>) {
            build_idempotent_service_pair(
                config,
                $repo_ctor,
                $service_ctor,
                ServicePairFactory {
                    fixtures: (
                        Arc::new($fixture_cmd) as Arc<$cmd_trait>,
                        Arc::new($fixture_query) as Arc<$query_trait>,
                    ),
                    cast: |service| {
                        (
                            service.clone() as Arc<$cmd_trait>,
                            service as Arc<$query_trait>,
                        )
                    },
                },
            )
        }
    };
}

/// Build a command/query pair for services backed by a domain repository and
/// an idempotency repository.
///
/// This helper delegates branch selection to `build_service_pair`, and uses
/// `build_idempotent_service` when `ServerConfig.db_pool` is `Some(pool)`.
fn build_idempotent_service_pair<R, S, Cmd, Query>(
    config: &ServerConfig,
    make_repo: impl FnOnce(DbPool) -> R,
    make_service: impl FnOnce(Arc<R>, Arc<DieselIdempotencyRepository>) -> S,
    pair_factory: ServicePairFactory<S, Cmd, Query>,
) -> (Arc<Cmd>, Arc<Query>)
where
    R: 'static,
    S: 'static,
    Cmd: ?Sized + 'static,
    Query: ?Sized + 'static,
{
    build_service_pair(
        &config.db_pool,
        |pool| build_idempotent_service(pool, make_repo, make_service),
        pair_factory.fixtures,
        pair_factory.cast,
    )
}

/// Construct and return `(CatalogueRepository, DescriptorRepository)` by
/// selecting `DieselCatalogueRepository`/`DieselDescriptorRepository` when
/// `config.db_pool` is present, otherwise selecting
/// `FixtureCatalogueRepository`/`FixtureDescriptorRepository`.
fn build_catalogue_services(
    config: &ServerConfig,
) -> (Arc<dyn CatalogueRepository>, Arc<dyn DescriptorRepository>) {
    match &config.db_pool {
        Some(pool) => (
            Arc::new(DieselCatalogueRepository::new(pool.clone())),
            Arc::new(DieselDescriptorRepository::new(pool.clone())),
        ),
        None => (
            Arc::new(FixtureCatalogueRepository),
            Arc::new(FixtureDescriptorRepository),
        ),
    }
}

build_idempotent_pair!(
    build_user_preferences_pair,
    dyn UserPreferencesCommand,
    dyn UserPreferencesQuery,
    DieselUserPreferencesRepository::new,
    UserPreferencesService::new,
    FixtureUserPreferencesCommand,
    FixtureUserPreferencesQuery
);

build_idempotent_pair!(
    build_route_annotations_pair,
    dyn RouteAnnotationsCommand,
    dyn RouteAnnotationsQuery,
    DieselRouteAnnotationRepository::new,
    RouteAnnotationsService::new,
    FixtureRouteAnnotationsCommand,
    FixtureRouteAnnotationsQuery
);

fn build_offline_bundles_pair(
    config: &ServerConfig,
) -> (Arc<dyn OfflineBundleCommand>, Arc<dyn OfflineBundleQuery>) {
    match &config.db_pool {
        Some(pool) => {
            let repo = Arc::new(DieselOfflineBundleRepository::new(pool.clone()));
            let idempotency_repo = Arc::new(DieselIdempotencyRepository::new(pool.clone()));
            (
                Arc::new(OfflineBundleCommandService::new(
                    repo.clone(),
                    idempotency_repo,
                    Arc::new(mockable::DefaultClock),
                )),
                Arc::new(OfflineBundleQueryService::new(repo)),
            )
        }
        None => (
            Arc::new(FixtureOfflineBundleCommand),
            Arc::new(FixtureOfflineBundleQuery),
        ),
    }
}

fn build_walk_sessions_pair(
    config: &ServerConfig,
) -> (Arc<dyn WalkSessionCommand>, Arc<dyn WalkSessionQuery>) {
    match &config.db_pool {
        Some(pool) => {
            let repo = Arc::new(DieselWalkSessionRepository::new(pool.clone()));
            (
                Arc::new(WalkSessionCommandService::new(repo.clone())),
                Arc::new(WalkSessionQueryService::new(repo)),
            )
        }
        None => (
            Arc::new(FixtureWalkSessionCommand),
            Arc::new(FixtureWalkSessionQuery),
        ),
    }
}

/// Build the shared HTTP state from configured ports and fixture fallbacks.
pub(super) fn build_http_state(
    config: &ServerConfig,
    route_submission: Arc<dyn RouteSubmissionService>,
) -> web::Data<HttpState> {
    // TODO(#27): Wire remaining fixture ports (profile, interests)
    // to real DB-backed implementations once their adapters are ready.
    let (login, users) = build_login_users_pair(config);
    let (preferences, preferences_query) = build_user_preferences_pair(config);
    let (route_annotations, route_annotations_query) = build_route_annotations_pair(config);
    let (offline_bundles, offline_bundles_query) = build_offline_bundles_pair(config);
    let (walk_sessions, walk_sessions_query) = build_walk_sessions_pair(config);
    let (catalogue, descriptors) = build_catalogue_services(config);

    web::Data::new(HttpState::new_with_extra(
        HttpStatePorts {
            login,
            users,
            profile: Arc::new(FixtureUserProfileQuery),
            interests: Arc::new(FixtureUserInterestsCommand),
            preferences,
            preferences_query,
            route_annotations,
            route_annotations_query,
            route_submission,
            catalogue,
            descriptors,
        },
        HttpStateExtraPorts {
            offline_bundles,
            offline_bundles_query,
            walk_sessions,
            walk_sessions_query,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use backend::domain::DisplayName;
    use rstest::rstest;

    const DB_LOGIN_USERNAME: &str = "db-admin";
    const DB_LOGIN_PASSWORD: &str = "db-password";
    const DB_LOGIN_USER_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const DB_USER_ID: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    const DB_DISPLAY_NAME: &str = "DB Backed User";
    const FIXTURE_USERS_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
    const FIXTURE_DISPLAY_NAME: &str = "Ada Lovelace";

    #[derive(Clone, Copy)]
    struct StubDbBackedLoginUsers;

    #[async_trait]
    impl LoginService for StubDbBackedLoginUsers {
        async fn authenticate(&self, credentials: &LoginCredentials) -> Result<UserId, Error> {
            if credentials.username() == DB_LOGIN_USERNAME
                && credentials.password() == DB_LOGIN_PASSWORD
            {
                UserId::new(DB_LOGIN_USER_ID)
                    .map_err(|err| Error::internal(format!("invalid db user id: {err}")))
            } else {
                Err(Error::unauthorized("invalid credentials"))
            }
        }
    }

    #[async_trait]
    impl UsersQuery for StubDbBackedLoginUsers {
        async fn list_users(&self, _authenticated_user: &UserId) -> Result<Vec<User>, Error> {
            let user_id = UserId::new(DB_USER_ID)
                .map_err(|err| Error::internal(format!("invalid db user id: {err}")))?;
            let display_name = DisplayName::new(DB_DISPLAY_NAME)
                .map_err(|err| Error::internal(format!("invalid db display name: {err}")))?;
            Ok(vec![User::new(user_id, display_name)])
        }
    }

    #[rstest]
    #[tokio::test]
    async fn db_pool_present_selects_db_backed_login_and_users() {
        let (login, users) =
            build_login_users_pair_with_pool(&Some(()), |_| StubDbBackedLoginUsers);

        let fixture_credentials =
            LoginCredentials::try_from_parts(FIXTURE_LOGIN_USERNAME, FIXTURE_LOGIN_PASSWORD)
                .expect("fixture credentials shape");
        let db_credentials = LoginCredentials::try_from_parts(DB_LOGIN_USERNAME, DB_LOGIN_PASSWORD)
            .expect("db credentials shape");
        assert!(login.authenticate(&fixture_credentials).await.is_err());

        let authenticated_user = login
            .authenticate(&db_credentials)
            .await
            .expect("db-backed login should succeed");
        assert_eq!(authenticated_user.as_ref(), DB_LOGIN_USER_ID);

        let listed_users = users
            .list_users(&authenticated_user)
            .await
            .expect("db-backed users query should succeed");
        assert_eq!(listed_users.len(), 1);
        assert_eq!(listed_users[0].id().as_ref(), DB_USER_ID);
        assert_eq!(listed_users[0].display_name().as_ref(), DB_DISPLAY_NAME);
    }

    #[rstest]
    #[tokio::test]
    async fn db_pool_absent_keeps_fixture_login_and_users() {
        let (login, users) =
            build_login_users_pair_with_pool::<(), StubDbBackedLoginUsers>(&None, |_| {
                StubDbBackedLoginUsers
            });

        let fixture_credentials =
            LoginCredentials::try_from_parts(FIXTURE_LOGIN_USERNAME, FIXTURE_LOGIN_PASSWORD)
                .expect("fixture credentials shape");
        let db_credentials = LoginCredentials::try_from_parts(DB_LOGIN_USERNAME, DB_LOGIN_PASSWORD)
            .expect("db credentials shape");

        assert!(login.authenticate(&db_credentials).await.is_err());
        let authenticated_user = login
            .authenticate(&fixture_credentials)
            .await
            .expect("fixture login should succeed");
        assert_eq!(authenticated_user.as_ref(), FIXTURE_LOGIN_USER_ID);

        let listed_users = users
            .list_users(&authenticated_user)
            .await
            .expect("fixture users query should succeed");
        assert_eq!(listed_users.len(), 1);
        assert_eq!(listed_users[0].id().as_ref(), FIXTURE_USERS_ID);
        assert_eq!(
            listed_users[0].display_name().as_ref(),
            FIXTURE_DISPLAY_NAME
        );
    }
}
