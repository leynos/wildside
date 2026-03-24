//! Redis test server helpers.
//!
//! Provides a `RedisTestServer` utility that spawns a real `redis-server`
//! process on an ephemeral port for adapter contract tests.

#![cfg_attr(any(test, feature = "test-support"), allow(clippy::expect_used))]

use std::{
    net::SocketAddr,
    process::{Child, Command, Stdio},
    time::Duration,
};

use bb8_redis::{
    RedisConnectionManager,
    bb8::Pool,
    redis::{RedisError, cmd},
};
use tempfile::TempDir;
use tokio::time::sleep;

#[cfg(feature = "test-support")]
use crate::outbound::cache::RedisPool;

/// Real `redis-server` process for adapter contract tests.
#[derive(Debug)]
pub struct RedisTestServer {
    address: SocketAddr,
    process: Child,
    _temp_dir: TempDir,
}

impl RedisTestServer {
    /// Start a fresh Redis server on an ephemeral local port.
    pub async fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve redis port");
        let address = listener.local_addr().expect("redis server address");
        drop(listener);

        let temp_dir = TempDir::new().expect("create redis temp dir");
        let process = Command::new("redis-server")
            .arg("--bind")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(address.port().to_string())
            .arg("--save")
            .arg("")
            .arg("--appendonly")
            .arg("no")
            .arg("--dir")
            .arg(temp_dir.path())
            .arg("--loglevel")
            .arg("warning")
            .arg("--protected-mode")
            .arg("no")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn redis-server");

        let mut server = Self {
            address,
            process,
            _temp_dir: temp_dir,
        };
        server.wait_until_ready().await;
        server
    }

    /// Return the Redis URL for the running test server.
    pub fn redis_url(&self) -> String {
        format!("redis://{}/", self.address)
    }

    /// Build a `bb8-redis` pool against the running test server.
    #[cfg(feature = "test-support")]
    pub async fn pool(&self) -> Result<RedisPool, RedisError> {
        let manager = RedisConnectionManager::new(self.redis_url().as_str())?;
        Pool::builder().build(manager).await
    }

    /// Seed raw bytes directly into Redis for unhappy-path assertions.
    #[cfg(feature = "test-support")]
    pub async fn seed_raw_bytes(&self, key: &str, bytes: Vec<u8>) -> Result<(), RedisError> {
        let pool = self.pool().await?;
        let mut connection = pool.get().await.map_err(convert_run_error)?;
        cmd("SET")
            .arg(key)
            .arg(bytes)
            .query_async::<()>(&mut *connection)
            .await
    }

    async fn wait_until_ready(&mut self) {
        let manager = RedisConnectionManager::new(self.redis_url().as_str())
            .expect("build redis manager for readiness check");

        let mut attempts = 0;
        while attempts < 50
            && Pool::builder()
                .max_size(1)
                .build(manager.clone())
                .await
                .is_err()
        {
            // Check if the process has exited early
            if let Ok(Some(status)) = self.process.try_wait() {
                // Process exited, collect stderr for diagnostics
                let stderr = read_process_stderr(&mut self.process);
                panic!(
                    "redis-server exited early with status {} at {}\nstderr: {}",
                    status,
                    self.redis_url(),
                    stderr
                );
            }

            sleep(Duration::from_millis(100)).await;
            attempts += 1;
        }

        if attempts < 50 {
            return;
        }

        panic!("redis-server did not become ready at {}", self.redis_url());
    }
}

impl Drop for RedisTestServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Read stderr from a process, returning captured output or a placeholder.
fn read_process_stderr(process: &mut Child) -> String {
    use std::io::Read;

    process
        .stderr
        .as_mut()
        .and_then(|pipe| {
            let mut buf = String::new();
            pipe.read_to_string(&mut buf).ok().map(|_| buf)
        })
        .unwrap_or_else(|| "(stderr not captured)".to_owned())
}

/// Get an unused Redis URL for testing connection failures.
///
/// Binds an ephemeral port, drops the listener, and returns a Redis URL
/// for that address. Since the listener is dropped, connection attempts
/// will fail.
pub async fn unused_redis_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let address = listener.local_addr().expect("ephemeral port address");
    drop(listener);
    format!("redis://{address}/")
}

#[cfg(feature = "test-support")]
fn convert_run_error(error: bb8_redis::bb8::RunError<RedisError>) -> RedisError {
    match error {
        bb8_redis::bb8::RunError::User(error) => error,
        bb8_redis::bb8::RunError::TimedOut => {
            RedisError::from((bb8_redis::redis::ErrorKind::Io, "pool checkout timed out"))
        }
    }
}
