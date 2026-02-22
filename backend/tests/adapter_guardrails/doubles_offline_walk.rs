//! Test doubles for offline bundle and walk-session driving ports.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::domain::Error;
use backend::domain::ports::{
    CreateWalkSessionRequest, CreateWalkSessionResponse, DeleteOfflineBundleRequest,
    DeleteOfflineBundleResponse, GetOfflineBundleRequest, GetOfflineBundleResponse,
    ListOfflineBundlesRequest, ListOfflineBundlesResponse, OfflineBundleCommand,
    OfflineBundleQuery, UpsertOfflineBundleRequest, UpsertOfflineBundleResponse,
    WalkSessionCommand,
};

use super::recording_double_macro::recording_double;

/// Configurable success or failure outcome for offline bundle upserts.
#[derive(Clone)]
pub(crate) enum UpsertOfflineBundleCommandResponse {
    Ok(UpsertOfflineBundleResponse),
    Err(Error),
}

/// Configurable success or failure outcome for offline bundle deletes.
#[derive(Clone)]
pub(crate) enum DeleteOfflineBundleCommandResponse {
    Ok(DeleteOfflineBundleResponse),
    Err(Error),
}

/// Configurable success or failure outcome for offline bundle list queries.
#[derive(Clone)]
pub(crate) enum OfflineBundleListQueryResponse {
    Ok(ListOfflineBundlesResponse),
    Err(Error),
}

/// Configurable success or failure outcome for offline bundle get queries.
#[derive(Clone)]
pub(crate) enum OfflineBundleGetQueryResponse {
    Ok(GetOfflineBundleResponse),
    Err(Error),
}

trait CommandResponse {
    type Success;

    fn into_result(self) -> Result<Self::Success, Error>;
}

impl CommandResponse for UpsertOfflineBundleCommandResponse {
    type Success = UpsertOfflineBundleResponse;

    fn into_result(self) -> Result<Self::Success, Error> {
        match self {
            UpsertOfflineBundleCommandResponse::Ok(response) => Ok(response),
            UpsertOfflineBundleCommandResponse::Err(error) => Err(error),
        }
    }
}

impl CommandResponse for DeleteOfflineBundleCommandResponse {
    type Success = DeleteOfflineBundleResponse;

    fn into_result(self) -> Result<Self::Success, Error> {
        match self {
            DeleteOfflineBundleCommandResponse::Ok(response) => Ok(response),
            DeleteOfflineBundleCommandResponse::Err(error) => Err(error),
        }
    }
}

impl CommandResponse for OfflineBundleListQueryResponse {
    type Success = ListOfflineBundlesResponse;

    fn into_result(self) -> Result<Self::Success, Error> {
        match self {
            OfflineBundleListQueryResponse::Ok(response) => Ok(response),
            OfflineBundleListQueryResponse::Err(error) => Err(error),
        }
    }
}

impl CommandResponse for OfflineBundleGetQueryResponse {
    type Success = GetOfflineBundleResponse;

    fn into_result(self) -> Result<Self::Success, Error> {
        match self {
            OfflineBundleGetQueryResponse::Ok(response) => Ok(response),
            OfflineBundleGetQueryResponse::Err(error) => Err(error),
        }
    }
}

#[derive(Clone)]
struct CallRecorder<Req, Resp> {
    calls: Arc<Mutex<Vec<Req>>>,
    response: Arc<Mutex<Resp>>,
}

impl<Req, Resp> CallRecorder<Req, Resp>
where
    Resp: Clone + CommandResponse,
{
    fn record_and_respond(&self, request: Req) -> Result<Resp::Success, Error> {
        self.calls.lock().expect("offline calls lock").push(request);
        let response = self.response.lock().expect("offline response lock").clone();
        response.into_result()
    }
}

/// Recording double for `OfflineBundleCommand`.
#[derive(Clone)]
pub(crate) struct RecordingOfflineBundleCommand {
    upsert_recorder: CallRecorder<UpsertOfflineBundleRequest, UpsertOfflineBundleCommandResponse>,
    delete_recorder: CallRecorder<DeleteOfflineBundleRequest, DeleteOfflineBundleCommandResponse>,
}

impl RecordingOfflineBundleCommand {
    pub(crate) fn new(
        upsert_response: UpsertOfflineBundleCommandResponse,
        delete_response: DeleteOfflineBundleCommandResponse,
    ) -> Self {
        Self {
            upsert_recorder: CallRecorder {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Arc::new(Mutex::new(upsert_response)),
            },
            delete_recorder: CallRecorder {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Arc::new(Mutex::new(delete_response)),
            },
        }
    }

    pub(crate) fn upsert_calls(&self) -> Vec<UpsertOfflineBundleRequest> {
        self.upsert_recorder
            .calls
            .lock()
            .expect("offline upsert calls lock")
            .clone()
    }

    pub(crate) fn delete_calls(&self) -> Vec<DeleteOfflineBundleRequest> {
        self.delete_recorder
            .calls
            .lock()
            .expect("offline delete calls lock")
            .clone()
    }

    pub(crate) fn set_upsert_response(&self, response: UpsertOfflineBundleCommandResponse) {
        *self
            .upsert_recorder
            .response
            .lock()
            .expect("offline upsert response lock") = response;
    }

    pub(crate) fn set_delete_response(&self, response: DeleteOfflineBundleCommandResponse) {
        *self
            .delete_recorder
            .response
            .lock()
            .expect("offline delete response lock") = response;
    }
}

#[async_trait]
impl OfflineBundleCommand for RecordingOfflineBundleCommand {
    async fn upsert_bundle(
        &self,
        request: UpsertOfflineBundleRequest,
    ) -> Result<UpsertOfflineBundleResponse, Error> {
        self.upsert_recorder.record_and_respond(request)
    }

    async fn delete_bundle(
        &self,
        request: DeleteOfflineBundleRequest,
    ) -> Result<DeleteOfflineBundleResponse, Error> {
        self.delete_recorder.record_and_respond(request)
    }
}

/// Recording double for `OfflineBundleQuery`.
#[derive(Clone)]
pub(crate) struct RecordingOfflineBundleQuery {
    list_recorder: CallRecorder<ListOfflineBundlesRequest, OfflineBundleListQueryResponse>,
    get_recorder: CallRecorder<GetOfflineBundleRequest, OfflineBundleGetQueryResponse>,
}

impl RecordingOfflineBundleQuery {
    pub(crate) fn new(
        list_response: OfflineBundleListQueryResponse,
        get_response: OfflineBundleGetQueryResponse,
    ) -> Self {
        Self {
            list_recorder: CallRecorder {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Arc::new(Mutex::new(list_response)),
            },
            get_recorder: CallRecorder {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Arc::new(Mutex::new(get_response)),
            },
        }
    }

    pub(crate) fn list_calls(&self) -> Vec<ListOfflineBundlesRequest> {
        self.list_recorder
            .calls
            .lock()
            .expect("offline list calls lock")
            .clone()
    }

    pub(crate) fn set_list_response(&self, response: OfflineBundleListQueryResponse) {
        *self
            .list_recorder
            .response
            .lock()
            .expect("offline list response lock") = response;
    }
}

#[async_trait]
impl OfflineBundleQuery for RecordingOfflineBundleQuery {
    async fn list_bundles(
        &self,
        request: ListOfflineBundlesRequest,
    ) -> Result<ListOfflineBundlesResponse, Error> {
        self.list_recorder.record_and_respond(request)
    }

    async fn get_bundle(
        &self,
        request: GetOfflineBundleRequest,
    ) -> Result<GetOfflineBundleResponse, Error> {
        self.get_recorder.record_and_respond(request)
    }
}

recording_double! {
    /// Configurable success or failure outcome for `RecordingWalkSessionCommand`.
    pub(crate) enum WalkSessionCommandResponse {
        Ok(CreateWalkSessionResponse),
        Err(Error),
    }

    pub(crate) struct RecordingWalkSessionCommand {
        calls: CreateWalkSessionRequest,
        trait: WalkSessionCommand,
        method: create_session(&self, request: CreateWalkSessionRequest)
            -> Result<CreateWalkSessionResponse, Error>,
        record: request,
        calls_lock: "walk session command calls lock",
        response_lock: "walk session command response lock",
    }
}
