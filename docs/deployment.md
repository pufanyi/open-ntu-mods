# Deployment

## Railway PostgreSQL

1. Create a Railway PostgreSQL service.
2. Copy its internal `DATABASE_URL` into the app service environment.
3. Keep automated backups enabled and periodically test restore into a staging database.

## Railway App Service

Use `backend/Dockerfile` as the build image. It builds the frontend, compiles the Rust backend, copies `frontend/dist`, and runs the API binary.

Required production environment:

```env
DATABASE_URL=postgresql://...
APP_PUBLIC_URL=https://your-domain.example
BACKEND_PUBLIC_URL=https://your-railway-app.up.railway.app
SESSION_SECRET=replace-with-long-random-secret
COOKIE_SECURE=true
REQUIRE_ORIGIN_SECRET=true
ORIGIN_SECRET=replace-with-long-random-secret
MICROSOFT_CLIENT_ID=...
MICROSOFT_CLIENT_SECRET=...
MICROSOFT_ISSUER=https://login.microsoftonline.com/organizations/v2.0
NTU_ALLOWED_DOMAINS=e.ntu.edu.sg,ntu.edu.sg
NTU_TENANT_ID=
ENABLE_DEV_LOGIN=false
RUST_LOG=info
```

Railway provides `PORT`; the backend binds to `0.0.0.0:$PORT`.

## Migrations

Run migrations before or during release:

```bash
cd backend
sqlx migrate run
```

The MVP seed migration inserts demo data. For production, replace demo seed handling with an environment-specific seed process before launch.

## Cloudflare Worker

Deploy `worker/` with:

```bash
cd worker
pnpm install
cp wrangler.toml.example wrangler.toml
pnpm wrangler secret put ORIGIN_SECRET
pnpm deploy
```

Set:

```env
RAILWAY_ORIGIN=https://your-railway-app.up.railway.app
ORIGIN_SECRET=the-same-secret-as-backend
```

Route your public domain to the Worker. The Worker forwards the origin secret to Railway and adds security headers.

## Backups

Use Railway PostgreSQL backups plus periodic logical dumps. Keep at least one recent backup outside the primary Railway project for disaster recovery.

