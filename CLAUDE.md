# MotoManager API

Rust/Axum backend for the MotoManager fleet manager, serving two clients: the SwiftUI iOS app (`../MotoManagerApp`, distributed via TestFlight) and the React Router webapp (`../MotoManager`). SQLite via SQLx; deployed at `https://moto-api.herrmann.ltd`.

> `GEMINI.md` in this directory holds the detailed workflow guide (entity checklist, migration mechanics, upload helpers). The rules below apply on top of it — the compatibility section is the most important thing in this file.

## API backwards compatibility — non-negotiable

Installed iOS apps lag behind the deployed backend (TestFlight rollout is not instant, and users update slowly). **An older iOS app must always be able to connect to a newer backend and sync its data, even when it knows nothing about newer features.** Every API change must be additive:

- **Never remove or rename** existing endpoints, path/query parameters, or JSON keys in requests or responses. Adding new keys is fine — old clients ignore them.
- **New request fields must be optional** (`Option<T>` / `#[serde(default)]`) with server-side defaults. A request body that was valid for the previous release must still be accepted and must produce the same behavior it used to.
- **Never change the type, nullability, or semantics** of an existing response field, and never turn a previously present field into an absent one. Old decoders (Swift `Codable` is strict) will fail on missing or retyped keys.
- **The sync protocol is a hard compatibility surface**: `clientId`-keyed idempotent push and `?since=` cursor pull (migrations `011`/`012`) must keep working unchanged for the entities already sync-enabled (`maintenanceRecords`, `torqueSpecs`, `issues`, parts inventory). New syncable entities or fields are added alongside — old clients simply don't send or receive them, and that must be harmless.
- **Migrations that back API responses must be additive**: new columns nullable or with defaults; if a column must go away, keep emitting the old JSON key (e.g. derived in the handler / `row_to_value`) until no supported client reads it.
- **Don't repurpose status codes or the error shape** existing clients branch on (notably: 401 triggers the iOS logout flow).
- If a change genuinely cannot be made additively, it needs a new endpoint (or versioned route) plus a coordinated client rollout — that's the escape hatch, not the default.
- When adding fields, extend the integration tests in `tests/` with a request in the **old** shape (field absent) to prove the previous contract still holds.

## Essentials

- **Layout**: core logic in `src/lib.rs` (library); `src/main.rs` is only the server entry. Integration tests in `tests/` use the library via `setup_test_app` for an in-memory environment.
- **Stack**: Axum 0.8 (`{id}` path-param syntax), SQLx 0.9, SQLite. Handlers return `AppResult<T>` and use the existing extractors (`AuthUser`, `AdminUser`, `State(pool)`).
- **Naming**: all DB tables/columns and JSON keys are **camelCase**.
- **Migrations**: new file in `migrations/` with zero-padded prefix (currently up to `044_`). CI and the Dockerfile apply all of `migrations/*.sql` via a glob. Keep the local `db.sqlite` fully migrated so compile-time `sqlx` macros validate against the real schema.
- **Quality gate after every change**: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` (zero warnings), `cargo fmt --all`. Bug fixes start with a failing integration test.
