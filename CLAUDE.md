# MotoManager API — Claude Instructions

## Project

Rust REST API for motorcycle management. Target client: React Router SPA.

- **Framework**: Axum 0.7 (async, tower middleware)
- **Database**: SQLite via SQLx 0.7 — runtime `sqlx::query()` calls, no compile-time macros
- **Auth**: Bearer token sessions — `Authorization: Bearer <token>` header
- **Password hashing**: Argon2id via `argon2` crate (OWASP params: m=19456, t=2, p=1)

## Key conventions

### Database
- The DB was originally created by Drizzle ORM. All table and column names are **snake_case**.
- Exception: `motorcycles` table has two mixed-case columns that must stay as-is: `firstRegistration` and `initialOdo`. Always quote them in SQL: `"firstRegistration"`, `"initialOdo"`.
- Always query the actual tables. Never reference the old camelCase table names (`maintenanceRecords`, `torqueSpecs`, etc.).
- Migrations live in `migrations/`. SQLx runs them at startup via `sqlx::migrate!("./migrations")`.

### Auth
- `AuthUser` and `AdminUser` are Axum extractors in `src/auth/mod.rs`. Add them as handler parameters to require auth.
- `extract_bearer_token(&HeaderMap)` pulls the raw token string when needed without full user lookup (e.g. logout).
- Sessions have a fixed 14-day expiry. No sliding window.

### Error handling
- All handlers return `AppResult<T>` which is `Result<T, AppError>`.
- `AppError` variants: `Unauthorized`, `Forbidden`, `NotFound(String)`, `BadRequest(String)`, `Conflict(String)`, `Internal(String)`, `Database`, `Io`, `Image`.

### JSON responses
- JSON keys are camelCase (the API contract with the frontend). DB column names are snake_case. Map between them in `row_to_*` functions using `r.get("snake_case_col")` and `json!({ "camelCaseKey": value })`.

### File uploads
- Motorcycle images: multipart → `data/images/`
- Documents: multipart → `data/documents/` with optional preview in `data/previews/`
- File serving routes (`/images/:filename`, `/data/documents/:filename`, `/data/previews/:filename`) are unauthenticated for `<img>` tag compatibility.

## Project structure

```
src/
  main.rs          — router, AppState, CORS, server startup
  config.rs        — Config from env vars
  error.rs         — AppError, AppResult
  models.rs        — shared model structs (User, PublicUser, Session, etc.)
  auth/
    mod.rs         — AuthUser/AdminUser extractors, extract_bearer_token
    session.rs     — create/get/delete session tokens
    password.rs    — hash_password / verify_password (Argon2id)
  handlers/
    auth.rs        — login, logout, register, me
    motorcycles.rs — CRUD + multipart image upload; also exports maintenance_row_to_value, verify_motorcycle_ownership
    maintenance.rs — maintenance record CRUD + fuel consumption calculation
    issues.rs      — issue CRUD
    torque_specs.rs — torque spec CRUD + import
    previous_owners.rs
    documents.rs   — document upload/CRUD with preview generation
    locations.rs
    settings.rs    — user settings + change password
    admin.rs       — user management + currency management
    stats.rs
    files.rs       — static file serving with optional image resize
migrations/
  001_initial_schema.sql  — CREATE TABLE IF NOT EXISTS for all tables
bruno/                    — Bruno API collection (use environment: local)
```

## Build & run

```sh
cargo build
cargo run
```

No test suite currently. Verify changes by running the server and using the Bruno collection.

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
