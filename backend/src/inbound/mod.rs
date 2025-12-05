//! Inbound adapters (HTTP, WebSocket, etc.) that translate external requests
//! into domain service calls while keeping framework details at the edge.
//!
//! HTTP handlers live under [`http`], with future inbound transports (e.g.
//! WebSocket) expected to sit alongside it.

pub mod http;
pub mod ws;
