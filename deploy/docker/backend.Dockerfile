FROM rust:1.79-alpine AS build
RUN apk add --no-cache \
        build-base=0.5-r3 \
        musl-dev=1.2.4-r3 \
        pkgconf=1.9.5-r0 \
        openssl-dev=3.1.8-r0 && \
    rustup target add x86_64-unknown-linux-musl
ENV OPENSSL_STATIC=1
WORKDIR /app
# Cache dependencies independently from source to speed up rebuilds.
COPY backend/Cargo.toml backend/Cargo.lock backend/
RUN cargo fetch --locked --manifest-path backend/Cargo.toml
COPY backend/ backend/
RUN cargo build --locked --release --target x86_64-unknown-linux-musl \
    --manifest-path backend/Cargo.toml

FROM alpine:3.18 AS runtime
RUN apk add --no-cache curl=8.12.1-r0 ca-certificates=20241121-r1 && adduser -D -u 1000 app
WORKDIR /srv
COPY --from=build --chown=1000:1000 /app/target/x86_64-unknown-linux-musl/release/backend /srv/app
USER app
EXPOSE 8080
ENV RUST_LOG=info
# Basic liveness probe; production deployments may override the path.
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -f http://localhost:8080/health || exit 1
ENTRYPOINT ["/srv/app"]
