//! Test doubles for route annotations driving ports.

use std::sync::{Arc, Mutex};

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

#[derive(Clone)]
pub(crate) enum UpdateProgressCommandResponse {
    Ok(UpdateProgressResponse),
    Err(Error),
}

#[derive(Clone)]
pub(crate) enum DeleteNoteCommandResponse {
    Ok(DeleteNoteResponse),
    Err(Error),
}

#[derive(Clone)]
pub(crate) struct RecordingRouteAnnotationsCommand {
    upsert_calls: Arc<Mutex<Vec<UpsertNoteRequest>>>,
    update_calls: Arc<Mutex<Vec<UpdateProgressRequest>>>,
    delete_calls: Arc<Mutex<Vec<DeleteNoteRequest>>>,
    upsert_response: Arc<Mutex<UpsertNoteCommandResponse>>,
    update_response: Arc<Mutex<UpdateProgressCommandResponse>>,
    delete_response: Arc<Mutex<DeleteNoteCommandResponse>>,
}

impl RecordingRouteAnnotationsCommand {
    pub(crate) fn new(
        upsert_response: UpsertNoteCommandResponse,
        update_response: UpdateProgressCommandResponse,
        delete_response: DeleteNoteCommandResponse,
    ) -> Self {
        Self {
            upsert_calls: Arc::new(Mutex::new(Vec::new())),
            update_calls: Arc::new(Mutex::new(Vec::new())),
            delete_calls: Arc::new(Mutex::new(Vec::new())),
            upsert_response: Arc::new(Mutex::new(upsert_response)),
            update_response: Arc::new(Mutex::new(update_response)),
            delete_response: Arc::new(Mutex::new(delete_response)),
        }
    }

    pub(crate) fn upsert_calls(&self) -> Vec<UpsertNoteRequest> {
        self.upsert_calls
            .lock()
            .expect("notes upsert calls lock")
            .clone()
    }

    pub(crate) fn update_calls(&self) -> Vec<UpdateProgressRequest> {
        self.update_calls
            .lock()
            .expect("progress update calls lock")
            .clone()
    }

    pub(crate) fn delete_calls(&self) -> Vec<DeleteNoteRequest> {
        self.delete_calls
            .lock()
            .expect("notes delete calls lock")
            .clone()
    }

    pub(crate) fn set_upsert_response(&self, response: UpsertNoteCommandResponse) {
        *self
            .upsert_response
            .lock()
            .expect("notes upsert response lock") = response;
    }

    pub(crate) fn set_update_response(&self, response: UpdateProgressCommandResponse) {
        *self
            .update_response
            .lock()
            .expect("progress update response lock") = response;
    }

    pub(crate) fn set_delete_response(&self, response: DeleteNoteCommandResponse) {
        *self
            .delete_response
            .lock()
            .expect("notes delete response lock") = response;
    }
}

#[async_trait]
impl RouteAnnotationsCommand for RecordingRouteAnnotationsCommand {
    async fn upsert_note(&self, request: UpsertNoteRequest) -> Result<UpsertNoteResponse, Error> {
        self.upsert_calls
            .lock()
            .expect("notes upsert calls lock")
            .push(request);
        match self
            .upsert_response
            .lock()
            .expect("notes upsert response lock")
            .clone()
        {
            UpsertNoteCommandResponse::Ok(response) => Ok(response),
            UpsertNoteCommandResponse::Err(error) => Err(error),
        }
    }

    async fn delete_note(&self, request: DeleteNoteRequest) -> Result<DeleteNoteResponse, Error> {
        self.delete_calls
            .lock()
            .expect("notes delete calls lock")
            .push(request);
        match self
            .delete_response
            .lock()
            .expect("notes delete response lock")
            .clone()
        {
            DeleteNoteCommandResponse::Ok(response) => Ok(response),
            DeleteNoteCommandResponse::Err(error) => Err(error),
        }
    }

    async fn update_progress(
        &self,
        request: UpdateProgressRequest,
    ) -> Result<UpdateProgressResponse, Error> {
        self.update_calls
            .lock()
            .expect("progress update calls lock")
            .push(request);
        match self
            .update_response
            .lock()
            .expect("progress update response lock")
            .clone()
        {
            UpdateProgressCommandResponse::Ok(response) => Ok(response),
            UpdateProgressCommandResponse::Err(error) => Err(error),
        }
    }
}

/// Configurable success or failure outcome for RecordingRouteAnnotationsQuery.
#[derive(Clone)]
pub(crate) enum RouteAnnotationsQueryResponse {
    Ok(RouteAnnotations),
    Err(Error),
}

#[derive(Clone)]
pub(crate) struct RecordingRouteAnnotationsQuery {
    calls: Arc<Mutex<Vec<(Uuid, String)>>>,
    response: Arc<Mutex<RouteAnnotationsQueryResponse>>,
}

impl RecordingRouteAnnotationsQuery {
    pub(crate) fn new(response: RouteAnnotationsQueryResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    pub(crate) fn calls(&self) -> Vec<(Uuid, String)> {
        self.calls
            .lock()
            .expect("annotations query calls lock")
            .clone()
    }

    pub(crate) fn set_response(&self, response: RouteAnnotationsQueryResponse) {
        *self
            .response
            .lock()
            .expect("annotations query response lock") = response;
    }
}

#[async_trait]
impl RouteAnnotationsQuery for RecordingRouteAnnotationsQuery {
    async fn fetch_annotations(
        &self,
        route_id: Uuid,
        user_id: &UserId,
    ) -> Result<RouteAnnotations, Error> {
        self.calls
            .lock()
            .expect("annotations query calls lock")
            .push((route_id, user_id.to_string()));
        match self
            .response
            .lock()
            .expect("annotations query response lock")
            .clone()
        {
            RouteAnnotationsQueryResponse::Ok(response) => Ok(response),
            RouteAnnotationsQueryResponse::Err(error) => Err(error),
        }
    }
}
