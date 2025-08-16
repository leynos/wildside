FROM rust:1.79-alpine AS build
RUN apk add --no-cache build-base musl-dev pkgconfig openssl-dev
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /app/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN cargo fetch --locked
COPY backend/ ./
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.18 AS runtime
RUN apk add --no-cache curl
RUN adduser -D -u 1000 app
WORKDIR /srv
COPY --from=build --chown=1000:1000 /app/backend/target/x86_64-unknown-linux-musl/release/backend /srv/app
USER app
EXPOSE 8080
ENV RUST_LOG=info
ENTRYPOINT ["/srv/app"]
