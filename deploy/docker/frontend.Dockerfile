ARG BUN_VERSION=1
ARG NGINX_VERSION=1.27
ARG ALPINE_VERSION=3.20

FROM oven/bun:${BUN_VERSION} AS build
WORKDIR /web

# Install dependencies first to leverage Docker layer caching. Lockfile
# enforcement keeps CI and local installs in sync.
COPY package.json bun.lock ./
COPY packages/tokens/package.json packages/tokens/
COPY frontend-pwa/package.json frontend-pwa/
RUN cd packages/tokens && bun install --frozen-lockfile && \
    cd ../.. && cd frontend-pwa && bun install --frozen-lockfile

# With dependencies cached, copy the remaining sources and build both the
# design tokens and the PWA.
COPY packages/tokens packages/tokens
COPY frontend-pwa frontend-pwa
RUN cd packages/tokens && bun run build && \
    cd ../frontend-pwa && bun run build

# Nginx serves the built assets in the final image for local parity. In CI, the
# contents of /usr/share/nginx/html can still be exported to object storage.
FROM nginx:${NGINX_VERSION}-alpine${ALPINE_VERSION} AS runtime
ARG HEALTHCHECK_PORT=80
ARG HEALTHCHECK_PATH=/
ARG HEALTHCHECK_INTERVAL=30s
ARG HEALTHCHECK_TIMEOUT=3s
ARG HEALTHCHECK_RETRIES=3
COPY --from=build /web/frontend-pwa/dist/ /usr/share/nginx/html
HEALTHCHECK --interval=${HEALTHCHECK_INTERVAL} \
           --timeout=${HEALTHCHECK_TIMEOUT} \
           --retries=${HEALTHCHECK_RETRIES} \
  CMD wget --no-verbose --tries=1 --spider http://localhost:${HEALTHCHECK_PORT}${HEALTHCHECK_PATH} || exit 1
EXPOSE ${HEALTHCHECK_PORT}
