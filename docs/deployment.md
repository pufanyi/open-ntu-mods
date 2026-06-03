# Deployment

## Railway PostgreSQL

1. Create a Railway PostgreSQL service.
2. Reference its internal `DATABASE_URL` in the backend app service environment.
   This URL is for Railway services, not for local terminal access.
3. Keep automated backups enabled and periodically test restore into a staging database.

For local commands such as `sqlx migrate run`, use the PostgreSQL service's
public TCP connection string from Railway's Connect tab. Do not use the backend
app's `*.up.railway.app` HTTP domain as `DATABASE_URL`; SQLx will fail with
Postgres protocol/TLS errors such as `unexpected response from SSLRequest`.

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
RUN_MIGRATIONS_ON_STARTUP=true
NTU_ALLOWED_DOMAINS=e.ntu.edu.sg,ntu.edu.sg
EMAIL_LOGIN_ENABLED=true
EMAIL_LOGIN_DELIVERY=log
EMAIL_LOGIN_ALLOWED_DOMAINS=e.ntu.edu.sg,ntu.edu.sg
EMAIL_FROM=
RESEND_API_KEY=
ENABLE_DEV_LOGIN=false
RUST_LOG=info
```

Railway provides `PORT`; the backend binds to `0.0.0.0:$PORT`.

For a private beta using personal email addresses, set:

```env
EMAIL_LOGIN_ALLOWED_DOMAINS=*
```

Switch it back to `e.ntu.edu.sg,ntu.edu.sg` before treating accounts as NTU
verified. `EMAIL_LOGIN_DELIVERY=log` prints codes in Railway logs and is only
appropriate for early testing. For real email delivery, set
`EMAIL_LOGIN_DELIVERY=resend`, `RESEND_API_KEY`, and an `EMAIL_FROM` sender
verified in Resend.

## Migrations

For the MVP Railway deployment, set:

```env
RUN_MIGRATIONS_ON_STARTUP=true
```

The backend will apply embedded SQLx migrations before serving traffic. This is
the simplest path for a single-replica Railway service.

Alternatively, run migrations manually before release with a PostgreSQL public
TCP connection string:

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
pnpm wrangler secret put ORIGIN_SECRET
pnpm deploy
```

Set Cloudflare Worker variables:

```env
RAILWAY_ORIGIN=https://your-railway-app.up.railway.app
ORIGIN_SECRET=the-same-secret-as-backend
```

`RAILWAY_ORIGIN` can be a plain Worker variable. `ORIGIN_SECRET` must be a Worker secret.

For Cloudflare Git deployments from the repository root, do not use `npx wrangler deploy` at the workspace root. Use:

```bash
pnpm --filter worker build
pnpm deploy:worker
```

Or set the Cloudflare project root directory to `worker/` and use:

```bash
pnpm build
pnpm deploy
```

Route your public domain to the Worker. The Worker forwards the origin secret to Railway and adds security headers.

The Worker caches only safe public reads:

- Hashed frontend build assets are cached long-term because filenames are
  content-hashed.
- Anonymous `GET /api/courses*`, `GET /api/offerings*`, and
  `GET /api/sections*` responses are cached briefly at the edge.
- Requests with cookies bypass API caching so logged-in users see fresh content
  after edits, review changes, and moderation actions.

## Backups

Use Railway PostgreSQL backups plus periodic logical dumps. Keep at least one recent backup outside the primary Railway project for disaster recovery.
