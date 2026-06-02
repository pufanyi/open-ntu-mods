# syntax=docker/dockerfile:1.7

FROM node:24-slim AS frontend-builder
WORKDIR /repo
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml biome.json ./
COPY frontend/package.json frontend/tsconfig.json frontend/vite.config.ts frontend/vitest.setup.ts frontend/index.html ./frontend/
COPY frontend/public ./frontend/public
COPY frontend/src ./frontend/src
COPY backend/openapi.json ./backend/openapi.json
RUN corepack enable pnpm && pnpm install --frozen-lockfile
RUN pnpm --filter frontend generate:api && pnpm --filter frontend build

FROM rust:1.96-bookworm AS backend-builder
WORKDIR /repo
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml ./backend/Cargo.toml
COPY backend/src ./backend/src
COPY backend/migrations ./backend/migrations
RUN cargo build --release --package open-ntu-mods-backend --bin open-ntu-mods

FROM debian:bookworm-slim AS runtime
WORKDIR /app/backend
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
COPY --from=backend-builder /repo/target/release/open-ntu-mods /usr/local/bin/open-ntu-mods
COPY --from=frontend-builder /repo/frontend/dist /app/frontend/dist
ENV FRONTEND_DIST_DIR=/app/frontend/dist
ENV PORT=3000
EXPOSE 3000
CMD ["open-ntu-mods"]
