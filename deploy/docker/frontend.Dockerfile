FROM oven/bun:1 AS build
WORKDIR /web
COPY packages/tokens /web/packages/tokens
COPY frontend-pwa /web/frontend-pwa
WORKDIR /web/packages/tokens
RUN bun install && bun run build
WORKDIR /web/frontend-pwa
RUN bun install && bun run build

FROM scratch AS export
COPY --from=build /web/frontend-pwa/dist/ /dist/
