ARG RUST_VERSION=1.90.0
FROM rust:${RUST_VERSION}-bookworm AS build

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        libpq-dev \
        libsqlite3-dev \
        libssl-dev \
        pkg-config \
        protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
# Copy the workspace so path dependencies and git-locked dependencies resolve
# exactly as they do in local and CI builds.
COPY . .
RUN cargo fetch --locked --manifest-path backend/Cargo.toml && \
    cargo build --locked --release --bin backend --manifest-path backend/Cargo.toml

FROM debian:bookworm-slim AS runtime

ARG APP_UID=10001
ARG APP_GID=10001
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libpq5 \
        libsqlite3-0 \
        libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --system --gid "${APP_GID}" app && \
    useradd --system --uid "${APP_UID}" --gid "${APP_GID}" \
        --home-dir /srv --shell /usr/sbin/nologin app
WORKDIR /srv
COPY --from=build --chown=${APP_UID}:${APP_GID} /app/target/release/backend /srv/app
USER ${APP_UID}:${APP_GID}

ARG HEALTHCHECK_PORT=8080
ARG HEALTHCHECK_PATH=/health/live
ENV HEALTHCHECK_PORT=${HEALTHCHECK_PORT}
ENV HEALTHCHECK_PATH=${HEALTHCHECK_PATH}
ENV HOST=0.0.0.0
ENV PORT=${HEALTHCHECK_PORT}
ENV RUST_LOG=info
EXPOSE ${HEALTHCHECK_PORT}

# Basic liveness probe; override port or path with build args or env vars.
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD \
    curl -fsS "http://127.0.0.1:${HEALTHCHECK_PORT}${HEALTHCHECK_PATH}" || exit 1

ENTRYPOINT ["/srv/app"]
