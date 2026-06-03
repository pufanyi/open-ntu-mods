# Open NTU Mods

Open NTU Mods is an MVP course wiki and review site for NTU students. Public course information is collaboratively editable by verified NTU users, while personal reviews remain author-owned. Wiki pages are academic-year-aware standalone pages with immutable, viewable, and revertible history.

## Stack

- Backend: Rust, Axum, PostgreSQL, SQLx, tower-http, tracing, cookie sessions, Utoipa OpenAPI.
- Frontend: React, TypeScript, Vite, TanStack Router, TanStack Query, openapi-typescript, openapi-fetch, Biome, Vitest.
- Edge: Cloudflare Workers TypeScript reverse proxy for Railway.
- Tooling: pnpm workspace, mise, lefthook, GitHub Actions, cargo fmt/clippy/nextest/audit/deny.

## Local Setup

```bash
docker compose up -d
cd backend
cp .env.example .env
sqlx migrate run
cargo run
```

In another terminal:

```bash
pnpm install
pnpm --filter frontend generate:api
pnpm --filter frontend dev
```

Open `http://localhost:5173`, use the dev-login form, and select `verified_user`, `moderator`, or `admin` for local testing. Seed data includes demo SC2001 offerings and demo users only.

For Railway deployments, set `RUN_MIGRATIONS_ON_STARTUP=true` on the backend
service so SQLx migrations run before the app serves traffic. Local `sqlx`
commands need a PostgreSQL TCP connection string, not the Railway app HTTP
domain.

## Checks

```bash
pnpm check
pnpm --filter frontend build
pnpm --filter worker typecheck
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo audit
cargo deny check
```

## Deployment

Deploy PostgreSQL and the Rust app on Railway. Build with `backend/Dockerfile`; the image builds the React app and serves `frontend/dist` from the Rust backend. Put Cloudflare Workers in front of Railway, set `RAILWAY_ORIGIN` and `ORIGIN_SECRET`, and require `X-Origin-Secret` in production.

See `docs/` for architecture, API, tooling, deployment, and local development details.
