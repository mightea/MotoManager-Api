# MotoManager API — Claude Instructions

## Project

Rust REST API for motorcycle management. Target client: React Router SPA.

- **Framework**: Axum 0.7 (async, tower middleware)
- **Database**: SQLite via SQLx 0.7 — runtime `sqlx::query()` calls, no compile-time macros
- **Auth**: Bearer token sessions — `Authorization: Bearer <token>` header
- **Password hashing**: Argon2id via `argon2` crate (OWASP params: m=19456, t=2, p=1)

## Key conventions

### Database
- All table and column names are **camelCase** throughout (e.g. `maintenanceRecords`, `motorcycleId`, `passwordHash`).
- The `motorcycles` table has two columns that happen to be camelCase from the original Drizzle schema: `firstRegistration` and `initialOdo`. No special treatment needed — they follow the same camelCase convention as everything else.
- Migrations live in `migrations/`. SQLx runs them at startup via `sqlx::migrate!("./migrations")`.
- The existing `db.sqlite` must be deleted and recreated when the schema changes (no migration upgrade path from the old Drizzle snake_case schema).

### Auth
- `AuthUser` and `AdminUser` are Axum extractors in `src/auth/mod.rs`. Add them as handler parameters to require auth.
- `extract_bearer_token(&HeaderMap)` pulls the raw token string when needed without full user lookup (e.g. logout).
- Sessions have a fixed 14-day expiry. No sliding window.

### Error handling
- All handlers return `AppResult<T>` which is `Result<T, AppError>`.
- `AppError` variants: `Unauthorized`, `Forbidden`, `NotFound(String)`, `BadRequest(String)`, `Conflict(String)`, `Internal(String)`, `Database`, `Io`, `Image`.

### JSON responses
- JSON keys are camelCase (the API contract with the frontend). DB column names are also camelCase. Map in `row_to_*` functions using `r.get("camelCaseCol")` and `json!({ "camelCaseKey": value })`.
- One exception: `motorcycles.modelYear` is exposed as `fabricationDate` in the JSON API for historical compatibility with the frontend.

### File uploads
- Motorcycle images: multipart → `data/images/`
- Documents: multipart → `data/documents/` with optional preview in `data/previews/`
- File serving routes (`/images/:filename`, `/data/documents/:filename`, `/data/previews/:filename`) are unauthenticated for `<img>` tag compatibility.

## Project structure

```
src/
  lib.rs           — library entry point, AppState, build_app, build_cors
  main.rs          — server startup (uses lib.rs)
  config.rs        — Config from env vars
  error.rs         — AppError, AppResult
  models.rs        — shared model structs (User, PublicUser, Session, etc.)
  auth/
    mod.rs         — AuthUser/AdminUser extractors, extract_bearer_token
    session.rs     — create/get/delete session tokens
    password.rs    — hash_password / verify_password (Argon2id)
  handlers/
    ...
tests/             — integration tests
  motorcycles_test.rs
  documents_test.rs
migrations/
  ...
bruno/                    — Bruno API collection (use environment: local)
```

## Build & run

```sh
cargo build
cargo run
cargo test
```

Tests include unit tests in `src/` modules and integration tests in `tests/`. Integration tests use an in-memory database and isolated data directories.

## Environment variables

| Variable | Default | Notes |
|----------|---------|-------|
| `DATABASE_URL` | `sqlite:./db.sqlite` | |
| `PORT` | `3001` | |
| `ORIGIN` | `http://localhost:3001` | CORS allowed origin |
| `DATA_DIR` | `./data` | Root for images, documents, previews |
| `ENABLE_REGISTRATION` | `false` | `true` allows anyone to register |
| `APP_VERSION` | `2026.1.0` | Returned by `/api/health` |
| `RUST_LOG` | `info` | Set to `debug` for verbose sqlx/tower logs |
