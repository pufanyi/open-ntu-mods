# Tooling

## pnpm Workspace

Root scripts fan out to `frontend/` and `worker/`:

```bash
pnpm install
pnpm check
pnpm typecheck
pnpm test
pnpm build
```

## Biome

Biome is the default formatter/linter for TypeScript, JSON, CSS, Markdown, and YAML:

```bash
pnpm check
pnpm check:write
```

## OpenAPI Generation

The backend generates `backend/openapi.json` from Utoipa:

```bash
cargo run --bin export-openapi > backend/openapi.json
pnpm --filter frontend generate:api
```

The frontend imports generated types from `frontend/src/generated/api-types.ts`.

## Rust Checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo audit
cargo deny check
```

Backend tests use SQLx migrations against PostgreSQL.

## mise

`mise.toml` pins common tool versions for Node, pnpm, Rust, `cargo-nextest`, and `cargo-deny`.

## lefthook

`lefthook.yml` defines:

- pre-commit: Biome check and cargo fmt check.
- pre-push: frontend typecheck/build and backend clippy/tests.

Install hooks with:

```bash
lefthook install
```

## CI

GitHub Actions runs separate backend, frontend, and worker jobs. Backend CI starts PostgreSQL, runs migrations, then runs format, clippy, nextest, audit, and deny.

