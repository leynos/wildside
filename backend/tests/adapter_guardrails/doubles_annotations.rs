//! Test doubles for route annotations driving ports.

use std::sync::{Arc, Mutex};

use super::recording_double_macro::recording_double;
use async_trait::async_trait;
use backend::domain::ports::{
    DeleteNoteRequest, DeleteNoteResponse, RouteAnnotationsCommand, RouteAnnotationsQuery,
    UpdateProgressRequest, UpdateProgressResponse, UpsertNoteRequest, UpsertNoteResponse,
};
use backend::domain::{Error, RouteAnnotations, UserId};
use uuid::Uuid;

/// Configurable success or failure outcome for RecordingRouteAnnotationsCommand.
#[derive(Clone)]
pub(crate) enum UpsertNoteCommandResponse {
    Ok(UpsertNoteResponse),
    Err(Error),
}

/// Configurable success or failure outcome for progress updates.
#[derive(Clone)]
pub(crate) enum UpdateProgressCommandResponse {
    Ok(UpdateProgressResponse),
    Err(Error),
}

/// Configurable success or failure outcome for note deletion.
#[derive(Clone)]
pub(crate) enum DeleteNoteCommandResponse {
    Ok(DeleteNoteResponse),
    Err(Error),
}

trait CommandResponse {
    type Success;

    fn into_result(self) -> Result<Self::Success, Error>;
}

impl CommandResponse for UpsertNoteCommandResponse {
    type Success = UpsertNoteResponse;

    fn into_result(self) -> Result<Self::Success, Error> {
        match self {
            UpsertNoteCommandResponse::Ok(response) => Ok(response),
            UpsertNoteCommandResponse::Err(error) => Err(error),
        }
    }
}

impl CommandResponse for UpdateProgressCommandResponse {
    type Success = UpdateProgressResponse;

    fn into_result(self) -> Result<Self::Success, Error> {
        match self {
            UpdateProgressCommandResponse::Ok(response) => Ok(response),
            UpdateProgressCommandResponse::Err(error) => Err(error),
        }
    }
}

impl CommandResponse for DeleteNoteCommandResponse {
    type Success = DeleteNoteResponse;

    fn into_result(self) -> Result<Self::Success, Error> {
        match self {
            DeleteNoteCommandResponse::Ok(response) => Ok(response),
            DeleteNoteCommandResponse::Err(error) => Err(error),
        }
    }
}

/// Records requests and returns the configured response for command doubles.
#[derive(Clone)]
struct CallRecorder<Req, RespEnum> {
    calls: Arc<Mutex<Vec<Req>>>,
    response: Arc<Mutex<RespEnum>>,
}

impl<Req, RespEnum> CallRecorder<Req, RespEnum>
where
    RespEnum: Clone + CommandResponse,
{
    fn record_and_respond(&self, request: Req) -> Result<RespEnum::Success, Error> {
        self.calls
            .lock()
            .expect("test double calls lock")
            .push(request);
        let response = self
            .response
            .lock()
            .expect("test double response lock")
            .clone();
        response.into_result()
    }
}

#[derive(Clone)]
pub(crate) struct RecordingRouteAnnotationsCommand {
    upsert_recorder: CallRecorder<UpsertNoteRequest, UpsertNoteCommandResponse>,
    update_recorder: CallRecorder<UpdateProgressRequest, UpdateProgressCommandResponse>,
    delete_recorder: CallRecorder<DeleteNoteRequest, DeleteNoteCommandResponse>,
}

impl RecordingRouteAnnotationsCommand {
    pub(crate) fn new(
        upsert_response: UpsertNoteCommandResponse,
        update_response: UpdateProgressCommandResponse,
        delete_response: DeleteNoteCommandResponse,
    ) -> Self {
        Self {
            upsert_recorder: CallRecorder {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Arc::new(Mutex::new(upsert_response)),
            },
            update_recorder: CallRecorder {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Arc::new(Mutex::new(update_response)),
            },
            delete_recorder: CallRecorder {
                calls: Arc::new(Mutex::new(Vec::new())),
                response: Arc::new(Mutex::new(delete_response)),
            },
        }
    }

    pub(crate) fn upsert_calls(&self) -> Vec<UpsertNoteRequest> {
        self.upsert_recorder
            .calls
            .lock()
            .expect("notes upsert calls lock")
            .clone()
    }

    pub(crate) fn update_calls(&self) -> Vec<UpdateProgressRequest> {
        self.update_recorder
            .calls
            .lock()
            .expect("progress update calls lock")
            .clone()
    }

    pub(crate) fn delete_calls(&self) -> Vec<DeleteNoteRequest> {
        self.delete_recorder
            .calls
            .lock()
            .expect("notes delete calls lock")
            .clone()
    }

    pub(crate) fn set_upsert_response(&self, response: UpsertNoteCommandResponse) {
        *self
            .upsert_recorder
            .response
            .lock()
            .expect("notes upsert response lock") = response;
    }

    pub(crate) fn set_update_response(&self, response: UpdateProgressCommandResponse) {
        *self
            .update_recorder
            .response
            .lock()
            .expect("progress update response lock") = response;
    }

    pub(crate) fn set_delete_response(&self, response: DeleteNoteCommandResponse) {
        *self
            .delete_recorder
            .response
            .lock()
            .expect("notes delete response lock") = response;
    }
}

#[async_trait]
impl RouteAnnotationsCommand for RecordingRouteAnnotationsCommand {
    async fn upsert_note(&self, request: UpsertNoteRequest) -> Result<UpsertNoteResponse, Error> {
        self.upsert_recorder.record_and_respond(request)
    }

    async fn delete_note(&self, request: DeleteNoteRequest) -> Result<DeleteNoteResponse, Error> {
        self.delete_recorder.record_and_respond(request)
    }

    async fn update_progress(
        &self,
        request: UpdateProgressRequest,
    ) -> Result<UpdateProgressResponse, Error> {
        self.update_recorder.record_and_respond(request)
    }
}

recording_double! {
    /// Configurable success or failure outcome for RecordingRouteAnnotationsQuery.
    pub(crate) enum RouteAnnotationsQueryResponse {
        Ok(RouteAnnotations),
        Err(Error),
    }

    pub(crate) struct RecordingRouteAnnotationsQuery {
        calls: (Uuid, String),
        trait: RouteAnnotationsQuery,
        method: fetch_annotations(&self, route_id: Uuid, user_id: &UserId)
            -> Result<RouteAnnotations, Error>,
        record: (route_id, user_id.to_string()),
        calls_lock: "annotations query calls lock",
        response_lock: "annotations query response lock",
    }
}
