# Local Development

## Start PostgreSQL

```bash
docker compose up -d
```

## Backend

```bash
cd backend
cp .env.example .env
sqlx migrate run
cargo run
```

The API runs on `http://localhost:3000`, serves `/openapi.json`, and serves the built frontend when `frontend/dist` exists.

## Frontend

```bash
pnpm install
pnpm --filter frontend generate:api
pnpm --filter frontend dev
```

Open `http://localhost:5173`. The Vite server proxies `/api`, `/auth`, `/health`, and `/openapi.json` to the backend.

## Demo Users

Use dev login with:

- `student@e.ntu.edu.sg`, role `verified_user`
- `editor@e.ntu.edu.sg`, role `trusted_editor`
- `admin@e.ntu.edu.sg`, role `admin`

Seed data includes SC2001 Algorithm Design and Analysis, AY2024/25 Sem 1, AY2025/26 Sem 1, inherited sections, and one demo review. It does not include real NTU materials.

## Manual Vertical Slice

1. Login as `student@e.ntu.edu.sg`.
2. Open SC2001.
3. Open AY2025/26 Sem 1.
4. Open a section that shows `Inherited`.
5. Edit it and save.
6. Confirm it now shows `Local`.
7. Open history and compare versions.
8. Login as `admin@e.ntu.edu.sg`.
9. Use `/admin` with the commit ID to revert, or section/version IDs to restore.

