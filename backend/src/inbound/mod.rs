//! Inbound adapters (HTTP, WebSocket, etc.) that translate external requests
//! into domain service calls while keeping framework details at the edge.
//!
//! HTTP handlers live under [`http`]; WebSocket handlers live under [`ws`],
//! with future inbound transports expected to sit alongside them.

pub mod http;
pub mod ws;
