//! WebSocket session handler tests.

use super::*;
use crate::domain::UserOnboardingService;
use crate::inbound::ws;
use crate::inbound::ws::state::WsState;
use actix_web::{App, HttpServer, dev::Server, dev::ServerHandle, http::header};
use awc::{BoxedSocket, ws::Codec, ws::Frame, ws::Message};
use futures_util::{SinkExt, StreamExt};
use rstest::{fixture, rstest};
use serde_json::Value;
use std::{error::Error as StdError, io, sync::Arc};
use uuid::Uuid;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

#[fixture]
async fn start_ws_server() -> TestResult<(String, Server)> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    let ws_state = WsState::new(Arc::new(UserOnboardingService));
    let server = HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::Data::new(ws_state.clone()))
            .service(ws::ws_entry)
    })
    .listen(listener)?
    .disable_signals()
    .run();
    let url = format!("http://{addr}");
    Ok((url, server))
}

#[fixture]
async fn ws_client(
    #[future] start_ws_server: TestResult<(String, Server)>,
) -> TestResult<(actix_codec::Framed<BoxedSocket, Codec>, ServerHandle)> {
    let (url, server) = start_ws_server.await?;
    let handle = server.handle();
    actix_web::rt::spawn(server);

    let (_resp, socket) = awc::Client::default()
        .ws(format!("{url}/ws"))
        .set_header(header::ORIGIN, "http://localhost:3000")
        .connect()
        .await?;

    Ok((socket, handle))
}

fn handshake_request_payload(name: &str) -> String {
    serde_json::json!({
        "traceId": Uuid::nil(),
        "displayName": name
    })
    .to_string()
}

async fn next_text_frame(
    socket: &mut actix_codec::Framed<BoxedSocket, Codec>,
) -> TestResult<Vec<u8>> {
    loop {
        let frame = socket
            .next()
            .await
            .ok_or_else(|| io::Error::other("response frame"))??;
        match frame {
            Frame::Text(bytes) => return Ok(bytes.to_vec()),
            Frame::Ping(_) | Frame::Pong(_) => continue,
            other => panic!("expected text frame, got {other:?}"),
        }
    }
}

#[rstest]
#[actix_rt::test]
async fn sends_user_created_event_for_valid_payload(
    #[future] ws_client: TestResult<(actix_codec::Framed<BoxedSocket, Codec>, ServerHandle)>,
) -> TestResult {
    let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await?;
    socket
        .send(Message::Text(handshake_request_payload("Bob").into()))
        .await?;

    let text = next_text_frame(&mut socket).await?;
    let value: Value = serde_json::from_slice(&text)?;
    assert_eq!(
        value.get("displayName").and_then(Value::as_str),
        Some("Bob")
    );
    assert!(value.get("id").is_some(), "user id present");
    assert_eq!(
        value.get("traceId").and_then(Value::as_str),
        Some(Uuid::nil().to_string().as_str())
    );
    Ok(())
}

#[rstest]
#[actix_rt::test]
async fn sends_rejection_for_invalid_payload(
    #[future] ws_client: TestResult<(actix_codec::Framed<BoxedSocket, Codec>, ServerHandle)>,
) -> TestResult {
    let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await?;
    socket
        .send(Message::Text(handshake_request_payload("bad$char").into()))
        .await?;

    let text = next_text_frame(&mut socket).await?;
    let value: Value = serde_json::from_slice(&text)?;
    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some("invalid_chars")
    );
    assert_eq!(
        value
            .get("details")
            .and_then(|v| v.get("field"))
            .and_then(Value::as_str),
        Some("displayName")
    );
    Ok(())
}

#[rstest]
#[actix_rt::test]
async fn closes_on_malformed_json(
    #[future] ws_client: TestResult<(actix_codec::Framed<BoxedSocket, Codec>, ServerHandle)>,
) -> TestResult {
    let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await?;
    socket
        .send(awc::ws::Message::Text("not-json".into()))
        .await?;

    // Skip any Ping/Pong frames that may arrive before the Close frame.
    loop {
        let frame = socket
            .next()
            .await
            .ok_or_else(|| io::Error::other("response frame"))??;
        match frame {
            Frame::Ping(_) | Frame::Pong(_) => continue,
            Frame::Close(reason) => {
                assert_eq!(
                    reason.ok_or_else(|| io::Error::other("reason"))?.code,
                    CloseCode::Policy
                );
                return Ok(());
            }
            other => panic!("expected close frame, got {other:?}"),
        }
    }
}

#[rstest]
#[actix_rt::test]
async fn closes_after_timeout_without_client_messages(
    #[future] ws_client: TestResult<(actix_codec::Framed<BoxedSocket, Codec>, ServerHandle)>,
) -> TestResult {
    let (mut socket, _server): (actix_codec::Framed<_, _>, _) = ws_client.await?;
    tokio::time::sleep(CLIENT_TIMEOUT + HEARTBEAT_INTERVAL * 3).await;

    use std::time::Duration;

    let observed_close = tokio::time::timeout(Duration::from_secs(2), async {
        let mut observed = None;
        while let Some(frame) = socket.next().await {
            let frame = match frame {
                Ok(frame) => frame,
                Err(error) => return Some(Err::<_, actix_ws::ProtocolError>(error)),
            };
            match frame {
                Frame::Ping(_) | Frame::Pong(_) => continue,
                Frame::Close(reason) => {
                    observed = Some(match reason {
                        Some(reason) => Ok::<_, actix_ws::ProtocolError>(reason),
                        None => Err(actix_ws::ProtocolError::UnmaskedFrame),
                    });
                    break;
                }
                other => panic!("unexpected frame before close: {other:?}"),
            }
        }
        observed
    })
    .await
    .map_err(|_| io::Error::other("close frame missing within timeout"))?
    .ok_or_else(|| io::Error::other("close frame missing after timeout"))??;

    let reason = observed_close;
    assert_eq!(reason.code, CloseCode::Normal);
    assert_eq!(reason.description.as_deref(), Some("heartbeat timeout"));
    Ok(())
}
