FROM oven/bun:1 AS build
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
FROM nginx:1.27-alpine AS runtime
COPY --from=build /web/frontend-pwa/dist/ /usr/share/nginx/html
EXPOSE 80
