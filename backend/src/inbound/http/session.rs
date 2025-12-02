//! Session helpers to keep HTTP handlers free of framework-specific logic.
//!
//! Provides a thin wrapper around Actix sessions so handlers only deal with
//! domain-friendly operations such as persisting or retrieving a user id.

use actix_session::Session;
use actix_web::{dev::Payload, FromRequest, HttpRequest};
use futures_util::future::LocalBoxFuture;

use crate::domain::{Error, UserId};

pub(crate) const USER_ID_KEY: &str = "user_id";

/// Newtype wrapper that exposes higher-level session operations.
#[derive(Clone)]
pub struct SessionContext(Session);

impl SessionContext {
    /// Construct a new wrapper from the underlying Actix session.
    pub fn new(session: Session) -> Self {
        Self(session)
    }

    /// Persist the authenticated user's id in the session cookie.
    pub fn persist_user(&self, user_id: &UserId) -> Result<(), Error> {
        self.0
            .insert(USER_ID_KEY, user_id.as_ref())
            .map_err(|error| Error::internal(format!("failed to persist session: {error}")))
    }

    /// Fetch the current user id from the session, if present.
    pub fn user_id(&self) -> Result<Option<UserId>, Error> {
        let id = self
            .0
            .get::<String>(USER_ID_KEY)
            .map_err(|error| Error::internal(format!("failed to read session: {error}")))?;
        match id {
            Some(raw) => match UserId::new(raw) {
                Ok(id) => Ok(Some(id)),
                Err(error) => {
                    tracing::warn!("invalid user id in session cookie: {error}");
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    /// Require an authenticated user id or return `401 Unauthorized`.
    pub fn require_user_id(&self) -> Result<UserId, Error> {
        self.user_id()?
            .ok_or_else(|| Error::unauthorized("login required"))
    }
}

impl FromRequest for SessionContext {
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let fut = Session::from_request(req, payload);
        Box::pin(async move { fut.await.map(SessionContext::new) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_session::Session;
    use actix_web::http::StatusCode;
    use actix_web::{test, web, App, HttpResponse};

    fn session_test_app() -> App<
        impl actix_web::dev::ServiceFactory<
            actix_web::dev::ServiceRequest,
            Config = (),
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
            InitError = (),
        >,
    > {
        App::new().wrap(crate::inbound::http::test_utils::test_session_middleware())
    }

    #[actix_web::test]
    async fn round_trips_user_id() {
        let app = test::init_service(
            session_test_app()
                .route(
                    "/set",
                    web::get().to(|session: SessionContext| async move {
                        let id = UserId::new("3fa85f64-5717-4562-b3fc-2c963f66afa6")
                            .expect("fixture id");
                        session.persist_user(&id)?;
                        Ok::<_, Error>(HttpResponse::Ok())
                    }),
                )
                .route(
                    "/get",
                    web::get().to(|session: SessionContext| async move {
                        let id = session.require_user_id()?;
                        Ok::<_, Error>(HttpResponse::Ok().body(id.to_string()))
                    }),
                ),
        )
        .await;

        let set_res =
            test::call_service(&app, test::TestRequest::get().uri("/set").to_request()).await;
        assert_eq!(set_res.status(), StatusCode::OK);
        let cookie = set_res
            .response()
            .cookies()
            .find(|cookie| cookie.name() == "session")
            .expect("session cookie set");

        let get_res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/get")
                .cookie(cookie)
                .to_request(),
        )
        .await;
        assert_eq!(get_res.status(), StatusCode::OK);
        let body = test::read_body(get_res).await;
        assert_eq!(body, "3fa85f64-5717-4562-b3fc-2c963f66afa6");
    }

    #[actix_web::test]
    async fn missing_user_is_unauthorised() {
        let app = test::init_service(session_test_app().route(
            "/require",
            web::get().to(|session: SessionContext| async move {
                let _ = session.require_user_id()?;
                Ok::<_, Error>(HttpResponse::Ok())
            }),
        ))
        .await;

        let res =
            test::call_service(&app, test::TestRequest::get().uri("/require").to_request()).await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn tampered_user_id_is_unauthorised() {
        let app = test::init_service(
            session_test_app()
                .route(
                    "/set-invalid",
                    web::get().to(|session: Session| async move {
                        session
                            .insert(USER_ID_KEY, "not-a-uuid")
                            .expect("set invalid user id");
                        HttpResponse::Ok()
                    }),
                )
                .route(
                    "/require",
                    web::get().to(|session: SessionContext| async move {
                        let _ = session.require_user_id()?;
                        Ok::<_, Error>(HttpResponse::Ok())
                    }),
                ),
        )
        .await;

        let set_res = test::call_service(
            &app,
            test::TestRequest::get().uri("/set-invalid").to_request(),
        )
        .await;
        let cookie = set_res
            .response()
            .cookies()
            .find(|cookie| cookie.name() == "session")
            .expect("session cookie set");

        let res = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/require")
                .cookie(cookie)
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
