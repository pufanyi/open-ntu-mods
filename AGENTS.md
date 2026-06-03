# AGENTS.md

Project guidance for agents working on `open-ntu-mods`.

## Maintenance Rule

- Keep this file current. When changing architecture, deployment, tooling,
  test commands, environment variables, repository layout, or important project
  conventions, update this `AGENTS.md` in the same change.
- Treat this file as operational documentation for future agents, not a static
  bootstrap note.
- Do not store secrets here. Use `.env.example` files and deployment docs for
  variable names only.

## Project Overview

- Product: NTU course wiki/review MVP with academic-year-aware standalone
  offerings, immutable wiki versions, author-owned reviews, and
  moderation/admin workflows.
- Backend: Rust, Axum, PostgreSQL, SQLx, tower-http, tracing, cookie sessions,
  Microsoft Entra OIDC structure, dev-login for local/staging only, Utoipa
  OpenAPI.
- Frontend: React, TypeScript, Vite, TanStack Router, TanStack Query,
  generated OpenAPI types, openapi-fetch, Biome, Vitest, Playwright smoke spec.
- Worker: Cloudflare Worker TypeScript reverse proxy to the Railway backend.
- Deployment: Railway runs PostgreSQL and the Rust backend. The backend
  Dockerfile builds and serves the frontend. Cloudflare Worker is the public
  gateway and injects `X-Origin-Secret`.

## Repository Management

- This repository is managed by Jujutsu. Prefer `jj status`, `jj log`,
  `jj diff`, `jj bookmark move`, and `jj git push`.
- Push `main` with `jj git push --bookmark main` after moving the bookmark to
  the intended change.
- Avoid committing generated build output such as `frontend/dist`, `target`, or
  `node_modules`.

## Local Development

Backend:

```bash
docker compose up -d
cd backend
cp .env.example .env
sqlx migrate run
cargo run
```

`RUN_MIGRATIONS_ON_STARTUP=true` can be used for Railway or throwaway local
environments. Keep manual `sqlx migrate run` as the default local workflow when
working directly against a local Postgres container.

Frontend:

```bash
pnpm install
pnpm --filter frontend generate:api
pnpm --filter frontend dev
```

Open `http://localhost:5173`. Vite proxies `/api`, `/auth`, `/health`, and
`/openapi.json` to the backend.

## Verification Commands

Run what is practical locally before pushing:

```bash
pnpm check
pnpm --filter frontend typecheck
pnpm --filter frontend test
pnpm --filter frontend build
pnpm --filter worker typecheck
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --no-run
```

When PostgreSQL and tools are available, also run:

```bash
cargo nextest run --workspace
cargo audit
cargo deny check
```

## OpenAPI

- Backend OpenAPI is exported with:

```bash
cargo run --quiet --bin export-openapi > backend/openapi.json
pnpm --filter frontend generate:api
```

- Regenerate and commit `backend/openapi.json` and
  `frontend/src/generated/api-types.ts` whenever API schemas or routes change.

## Database Changes

- Never edit an already committed/applied migration file. Treat files under
  `backend/migrations/00*_*.sql` as append-only once pushed, because SQLx stores
  migration checksums and production Railway startup migrations will fail if an
  old migration changes.
- Change schema or production data with a new numbered migration, for example
  `backend/migrations/004_describe_change.sql`.
- Keep migrations forward-only for this MVP. Do not add destructive schema or
  data changes without an explicit backup/rollback plan documented in the same
  change.
- If a migration needs to transform existing production rows, make it
  idempotent where practical and preserve immutable history tables. For wiki
  content, prefer creating new `wiki_commits`, `wiki_versions`, and
  `wiki_commit_changes` instead of overwriting or deleting old versions.
- After changing database shape, update the Rust SQLx models/queries, OpenAPI
  schemas, generated frontend API types, tests, and docs in the same change.
- Verify migrations with `sqlx migrate run` against a disposable/local
  PostgreSQL database when available. GitHub Actions also runs migrations
  against Postgres before Rust checks.
- Production Railway has `RUN_MIGRATIONS_ON_STARTUP=true`; successful backend
  deployment applies new migrations before serving traffic.
- Do not commit database URLs, dumps, Railway variable values, or other secrets.

## Deployment Notes

- Railway should have only the backend service plus PostgreSQL for this repo.
  Do not deploy `frontend/` or `worker/` as Railway services.
- Railway backend builder should use Dockerfile with path:

```txt
backend/Dockerfile
```

- Root directory should stay at repository root because the Dockerfile needs
  `frontend/`, `package.json`, `pnpm-lock.yaml`, and Rust workspace files.
- If `frontend/tsconfig.json` or Vite config starts depending on additional
  root-level frontend files, make sure `backend/Dockerfile` copies those files
  into the frontend build stage.
- Cloudflare Worker deploys from `worker/`, not Railway.
- `worker/wrangler.toml` keeps runtime variables and secrets managed in the
  Cloudflare dashboard with `keep_vars = true`. Do not commit `ORIGIN_SECRET`;
  keep `RAILWAY_ORIGIN` set as a plain Worker variable in Cloudflare unless the
  production origin is intentionally moved into version-controlled config.
- Do not run `wrangler deploy` from the repository root. Use
  `pnpm deploy:worker` from the root or run `pnpm deploy` inside `worker/`.
- Keep `ORIGIN_SECRET` identical between Railway backend and Cloudflare Worker.
- Set `RUN_MIGRATIONS_ON_STARTUP=true` on Railway for the MVP deployment so the
  backend applies SQLx migrations before serving traffic.
- Worker production variables must include `RAILWAY_ORIGIN=https://...` as a
  plain variable and `ORIGIN_SECRET` as a secret. Missing Worker variables are
  the most common cause of Cloudflare 1101 errors.
- Backend origin protection intentionally exempts `/health` so Railway
  healthchecks can pass without custom headers. Do not exempt API/Auth routes.

## Auth Notes

- Production Microsoft Entra login may require NTU admin consent. Do not treat
  Microsoft login as a blocker for deploying the public read-only site.
- `ENABLE_DEV_LOGIN=true` is acceptable for local development and tightly
  controlled staging only. Keep it `false` for public production unless an
  explicit temporary testing plan is in place.

## Security Notes

- Never commit real `.env` files or secrets.
- `.cargo/audit.toml` ignores `RUSTSEC-2023-0071` because `sqlx` records
  optional MySQL macro support in `Cargo.lock`; this backend disables SQLx
  default features and compiles PostgreSQL only.
- If dependency features change, re-evaluate that audit ignore.
