# MotoManager API

Rust backend for MotoManager — a motorcycle maintenance and management application. Built with [Axum](https://github.com/tokio-rs/axum) and [SQLx](https://github.com/launchbadge/sqlx) against a SQLite database.

## Stack

- **Framework**: Axum 0.8
- **Database**: SQLite via SQLx 0.8 (runtime queries, camelCase table and column names)
- **Auth**: Passkey (WebAuthn) and Bearer token sessions (`Authorization: Bearer <token>`)
- **Password hashing**: Argon2id (OWASP-recommended parameters)

## Setup

1. Copy the example env file and configure it:
   ```sh
   cp .env.example .env
   ```

2. Edit `.env`:
   ```
   DATABASE_URL=sqlite:./db.sqlite   # path to your SQLite database
   PORT=3001
   ORIGIN=http://localhost:5173      # frontend origin for CORS
   DATA_DIR=./data                   # where uploaded files are stored
   ENABLE_REGISTRATION=false         # false = only the first user can register
   RUST_LOG=info
   ```

3. Build and run:
   ```sh
   cargo run
   ```

## Testing

The project includes a comprehensive test suite covering unit tests and integration tests for the HTTP layer.

### Run all tests
```sh
cargo test
```

### Linting
```sh
cargo clippy --all-targets --all-features -- -D warnings
```

### Integration Tests
Integration tests use an in-memory SQLite database and isolated `DATA_DIR` for file operations. They are located in the `tests/` directory:
- `tests/motorcycles_test.rs`: Covers motorcycles, issues, and maintenance records lifecycle.
- `tests/documents_test.rs`: Covers document listing and deletion.

The server runs migrations on startup and creates the `data/` and `cache/` directories automatically.

## Authentication

All protected endpoints require an `Authorization` header:

```
Authorization: Bearer <token>
```

The token is returned by authentication endpoints. Sessions expire after 14 days. On logout the session is deleted server-side.

## API Routes

All routes are prefixed with `/api`. Routes marked **auth** require a valid bearer token. Routes marked **admin** additionally require the `admin` role.

### Auth

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/auth/login` | — | Log in with password, returns `{ user, token }` |
| POST | `/api/auth/logout` | auth | Invalidate the current session |
| POST | `/api/auth/register` | — | Register (first user or `ENABLE_REGISTRATION=true`) |
| GET | `/api/auth/me` | auth | Get the authenticated user |
| GET | `/api/auth/passkey/login-options` | — | Get options for passkey login |
| POST | `/api/auth/passkey/login-verify` | — | Verify passkey login |
| GET | `/api/auth/passkey/register-options` | auth | Get options for passkey registration |
| POST | `/api/auth/passkey/register-verify` | auth | Verify passkey registration |

### Motorcycles

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/motorcycles` | auth | List user's motorcycles |
| POST | `/api/motorcycles` | auth | Create a motorcycle (multipart) |
| GET | `/api/motorcycles/{id}` | auth | Get a motorcycle details |
| PUT | `/api/motorcycles/{id}` | auth | Update a motorcycle (multipart) |
| DELETE | `/api/motorcycles/{id}` | auth | Delete a motorcycle |

### Maintenance Records

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/motorcycles/{id}/maintenance` | auth | List maintenance records |
| POST | `/api/motorcycles/{id}/maintenance` | auth | Create a maintenance record |
| PUT | `/api/motorcycles/{id}/maintenance/{mid}` | auth | Update a maintenance record |
| DELETE | `/api/motorcycles/{id}/maintenance/{mid}` | auth | Delete a maintenance record |

### Issues

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/motorcycles/{id}/issues` | auth | List issues |
| POST | `/api/motorcycles/{id}/issues` | auth | Create an issue |
| PUT | `/api/motorcycles/{id}/issues/{issue_id}` | auth | Update an issue |
| DELETE | `/api/motorcycles/{id}/issues/{issue_id}` | auth | Delete an issue |

### Torque Specs

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/motorcycles/{id}/torque-specs` | auth | List torque specs |
| POST | `/api/motorcycles/{id}/torque-specs` | auth | Create a torque spec |
| POST | `/api/motorcycles/{id}/torque-specs/import` | auth | Import specs from another motorcycle |
| PUT | `/api/motorcycles/{id}/torque-specs/{tid}` | auth | Update a torque spec |
| DELETE | `/api/motorcycles/{id}/torque-specs/{tid}` | auth | Delete a torque spec |

### Previous Owners

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/motorcycles/{id}/previous-owners` | auth | List previous owners |
| POST | `/api/motorcycles/{id}/previous-owners` | auth | Add a previous owner |
| PUT | `/api/motorcycles/{id}/previous-owners/{oid}` | auth | Update a previous owner |
| DELETE | `/api/motorcycles/{id}/previous-owners/{oid}` | auth | Delete a previous owner |

### Documents

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/documents` | auth | List documents |
| POST | `/api/documents` | auth | Upload a document (multipart) |
| PUT | `/api/documents/{doc_id}` | auth | Update a document |
| DELETE | `/api/documents/{doc_id}` | auth | Delete a document |

### Locations

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/locations` | auth | List locations |
| POST | `/api/locations` | auth | Create a location |
| PUT | `/api/locations/{lid}` | auth | Update a location |
| DELETE | `/api/locations/{lid}` | auth | Delete a location |

### Settings

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/settings` | auth | Get user settings |
| PUT | `/api/settings` | auth | Update user settings |
| POST | `/api/settings/change-password` | auth | Change password |
| GET | `/api/settings/authenticators` | auth | List registered passkeys |
| DELETE | `/api/settings/authenticators/{id}` | auth | Delete a passkey |

### Admin

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/admin/users` | admin | List all users |
| POST | `/api/admin/users` | admin | Create a user |
| PUT | `/api/admin/users/{uid}` | admin | Update a user |
| DELETE | `/api/admin/users/{uid}` | admin | Delete a user |
| GET | `/api/admin/currencies` | admin | List currencies |
| POST | `/api/admin/currencies` | admin | Add a currency |
| PUT | `/api/admin/currencies/{cid}` | admin | Update a currency |
| DELETE | `/api/admin/currencies/{cid}` | admin | Delete a currency |
| POST | `/api/admin/regenerate-previews` | admin | Regenerate all image/PDF previews |

### Public / Common

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/currencies` | — | List currencies |
| GET | `/api/stats` | auth | App statistics |
| GET | `/api/home` | auth | Home dashboard data |
| GET | `/api/health` | — | Health check |

### File Serving

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/images/{filename}` | — | Serve a motorcycle image |
| GET | `/documents/{filename}` | — | Serve a document file |
| GET | `/previews/{filename}` | — | Serve a document preview |
