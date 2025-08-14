FROM rust:1.79-alpine AS build
RUN apk add --no-cache build-base musl-dev pkgconfig openssl-dev
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /app/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN cargo fetch --locked
COPY backend/ ./
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM gcr.io/distroless/static:nonroot
WORKDIR /srv
COPY --from=build /app/backend/target/x86_64-unknown-linux-musl/release/backend /srv/app
USER nonroot:nonroot
EXPOSE 8080
ENV RUST_LOG=info
ENTRYPOINT ["/srv/app"]
