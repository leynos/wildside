ARG RUST_VERSION=1.79
FROM rust:${RUST_VERSION}-alpine AS build

ARG BUILD_BASE_VERSION=0.5-r3
ARG MUSL_DEV_VERSION=1.2.4-r3
ARG PKGCONF_VERSION=1.9.5-r0
ARG OPENSSL_DEV_VERSION=3.1.8-r0

RUN apk add --no-cache \
        build-base=${BUILD_BASE_VERSION} \
        musl-dev=${MUSL_DEV_VERSION} \
        pkgconf=${PKGCONF_VERSION} \
        openssl-dev=${OPENSSL_DEV_VERSION} && \
    rustup target add x86_64-unknown-linux-musl
ENV OPENSSL_STATIC=1
WORKDIR /app
# Cache dependencies independently from source to speed up rebuilds.
COPY backend/Cargo.toml backend/Cargo.lock backend/
RUN cargo fetch --locked --manifest-path backend/Cargo.toml
COPY backend/ backend/
RUN cargo build --locked --release --target x86_64-unknown-linux-musl \
    --manifest-path backend/Cargo.toml

ARG ALPINE_VERSION=3.18
FROM alpine:${ALPINE_VERSION} AS runtime

ARG CURL_VERSION=8.12.1-r0
ARG CERTS_VERSION=20241121-r1
RUN apk add --no-cache \
        curl=${CURL_VERSION} \
        ca-certificates=${CERTS_VERSION} && \
    adduser -D -u 1000 app
WORKDIR /srv
COPY --from=build --chown=1000:1000 /app/target/x86_64-unknown-linux-musl/release/backend /srv/app
USER app

ARG HEALTHCHECK_PORT=8080
ARG HEALTHCHECK_PATH=/health
ENV HEALTHCHECK_PORT=${HEALTHCHECK_PORT}
ENV HEALTHCHECK_PATH=${HEALTHCHECK_PATH}
EXPOSE ${HEALTHCHECK_PORT}

ENV RUST_LOG=info
# Basic liveness probe; override port or path with build args or env vars.
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD \
    curl -f http://localhost:${HEALTHCHECK_PORT}${HEALTHCHECK_PATH} || exit 1

ENTRYPOINT ["/srv/app"]
