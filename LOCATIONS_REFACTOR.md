# Locations Refactor Plan

## Context

The current codebase has a half-built locations concept:

- `locations` table exists (user-scoped: `name`, `country_code`, `userId`), with full CRUD at `/api/locations`.
- `locationRecords` correctly tracks motorcycle ↔ location ↔ odometer over time (the "where is the bike" use case).
- But `maintenanceRecords` stores location data as a **scattered mix**: `locationId` (FK, optional), `inspectionLocation` (free text), `locationName` (free text), `latitude`, `longitude`. This duplicates the locations table and makes coordinates a per-record property instead of a per-place property.
- The `locations` table itself has no concept of *what kind* of place it is (storage vs. shop vs. fuel station) and stores no coordinates.

Goal: One canonical, user-scoped, **typed** `Location` entity with optional coordinates, referenced by FK from every place a location is needed. Free-text and per-record coordinate columns on `maintenanceRecords` go away.

**Design decisions (locked):**

- **Closed enum** for location type, enforced both in Rust (`LocationType`) and SQL (`CHECK` constraint).
- **Migrate + drop**: best-effort backfill of existing maintenance free-text/coord data into Location rows, then drop the dead columns.

---

## Target Schema

### `locations` (evolved, not replaced)

| Column         | Type    | Constraints                                                          |
|----------------|---------|----------------------------------------------------------------------|
| `id`           | INTEGER | PK, AUTOINCREMENT                                                    |
| `userId`       | INTEGER | NOT NULL, FK → users(id) ON DELETE CASCADE                           |
| `name`         | TEXT    | NOT NULL                                                             |
| `type`         | TEXT    | NOT NULL, CHECK (type IN ('storage','maintenance_shop','fuel_station','inspection','other')) |
| `latitude`     | REAL    | NULL, CHECK (latitude IS NULL OR (latitude BETWEEN -90 AND 90))      |
| `longitude`    | REAL    | NULL, CHECK (longitude IS NULL OR (longitude BETWEEN -180 AND 180))  |
| `country_code` | TEXT    | NOT NULL DEFAULT 'CH' (kept; used for display/locale)                |
| `createdAt`    | TEXT    | NOT NULL DEFAULT CURRENT_TIMESTAMP                                   |
| `updatedAt`    | TEXT    | NULL                                                                 |

Indexes: `(userId, type)`, partial index on `(userId, name, type)` for find-or-create dedup during migration.

> **Note:** SQLite cannot add a CHECK to an existing table via `ALTER`. The migration uses the table-rebuild dance (`CREATE TABLE locations_new`, `INSERT INTO locations_new SELECT …`, `DROP TABLE locations`, `ALTER TABLE locations_new RENAME TO locations`).

### `maintenanceRecords` (slimmed down)

Drop: `inspectionLocation`, `locationName`, `latitude`, `longitude`.
Keep: `locationId` (FK → locations.id, **stays optional** — not every record has a place).

### `locationRecords` — unchanged

Already shaped right.

---

## Migration (one file: `migrations/005_locations_typed_coords.sql`)

Single migration, applied in order:

1. **Build the new `locations` shape** with `type`/`latitude`/`longitude`/`createdAt`/`updatedAt`, backfill existing rows with `type='other'`.
2. **Find-or-create** Location rows from maintenance free-text/coord data:
   ```sql
   INSERT INTO locations (userId, name, type, latitude, longitude, country_code, createdAt)
   SELECT DISTINCT
     m.userId,
     COALESCE(NULLIF(TRIM(mr.locationName), ''),
              NULLIF(TRIM(mr.inspectionLocation), ''),
              'Unknown'),
     'maintenance_shop',
     mr.latitude, mr.longitude,
     'CH',
     CURRENT_TIMESTAMP
   FROM maintenanceRecords mr
   JOIN motorcycles m ON mr.motorcycleId = m.id
   WHERE mr.locationId IS NULL
     AND (mr.locationName IS NOT NULL
          OR mr.inspectionLocation IS NOT NULL
          OR mr.latitude IS NOT NULL)
     AND NOT EXISTS (
       SELECT 1 FROM locations l
       WHERE l.userId = m.userId
         AND l.name = COALESCE(NULLIF(TRIM(mr.locationName), ''),
                               NULLIF(TRIM(mr.inspectionLocation), ''),
                               'Unknown')
         AND l.type = 'maintenance_shop'
     );
   ```
3. **Backfill `maintenanceRecords.locationId`** via `UPDATE … FROM` (SQLite ≥3.33) matching on (userId, name, type).
4. **Drop the four dead columns** from `maintenanceRecords` (`ALTER TABLE … DROP COLUMN`, SQLite ≥3.35).

The migration is idempotent-safe: it only acts on rows where `locationId IS NULL`, and the `NOT EXISTS` clause prevents duplicate insertions on re-runs of a partial migration.

**Dedup caveat (acknowledged, not solved here):** records with slightly different spellings (`"Garage Müller"` vs `"Garage Mueller"`) get separate Location rows. A post-migration cleanup tool is out of scope — flag this in the changelog so the user can dedup manually via the API.

---

## Code Changes

### New / changed (in dependency order)

1. **`src/models.rs`**
   - New: `LocationType` enum (`Storage`, `MaintenanceShop`, `FuelStation`, `Inspection`, `Other`) with `#[derive(sqlx::Type, serde::Serialize, serde::Deserialize)]`, `#[sqlx(rename_all = "snake_case")]`, `#[serde(rename_all = "camelCase")]`.
   - Update `Location` struct: add `r#type: LocationType`, `latitude: Option<f64>`, `longitude: Option<f64>`, `created_at: String`, `updated_at: Option<String>`.
   - Update `MaintenanceRecord` struct: remove `inspection_location`, `location_name`, `latitude`, `longitude`. Keep `location_id: Option<i64>`.

2. **`src/handlers/locations.rs`**
   - Request bodies (`CreateLocationRequest`, `UpdateLocationRequest`): add `type: LocationType`, `latitude: Option<f64>`, `longitude: Option<f64>`.
   - Add coord sanity validation (lat ∈ [-90,90], lon ∈ [-180,180], both-or-neither). The DB CHECKs catch bad data too, but explicit validation gives better error messages.
   - Add a new helper `pub async fn verify_location_ownership(pool, location_id, user_id) -> AppResult<()>` mirroring `verify_motorcycle_ownership` at `src/handlers/motorcycles.rs:?`.
   - Optional `?type=storage,maintenance_shop` filter on list (mirrors the `?types=` pattern already in `src/handlers/maintenance.rs:85-94`).

3. **`src/handlers/maintenance.rs`**
   - Strip the four removed fields from `MaintenanceRequest` (`src/handlers/maintenance.rs:130-137`), the INSERT column list (`:174`), and the UPDATE column list (`:325-326`). Keep `location_id`.
   - On create/update with `location_id`: call `verify_location_ownership(&pool, lid, user.id)` before binding.

4. **`src/handlers/motorcycles.rs`** (`src/handlers/motorcycles.rs:258-281`)
   - Replace the "distinct (locationName, lat, lon) from maintenance" aggregation with a join:
     ```sql
     SELECT DISTINCT l.* FROM locations l
     JOIN maintenanceRecords mr ON mr.locationId = l.id
     WHERE mr.motorcycleId = ?
     ```
   - Response field `maintenanceLocations` now contains full Location objects (with type + coords) instead of ad-hoc `{name, latitude, longitude}` blobs. **This is a breaking response-shape change** — flag in changelog; frontend update needed.

5. **`src/handlers/home.rs`** (`:68-79`)
   - Already location-aware; just confirm it works against the new schema. The fallback that derived current location from `maintenance.locationId` keeps working unchanged.

6. **`src/lib.rs`**
   - No router changes — the existing `/api/locations` endpoints absorb the new fields. Maintenance routes are unchanged in shape.

### Tests

- `tests/motorcycles_test.rs`: update `test_maintenance_lifecycle` and `test_maintenance_filtering` to pass `locationId` instead of `locationName`/coords. Add coverage for the rejected case: maintenance record with another user's `locationId` → 404 (not "leaked" as 400).
- New `tests/locations_test.rs`:
  - Create/list/update/delete with type + coords.
  - Coord validation: out-of-range lat/lon rejected with 400.
  - Type filter on list works.
  - User isolation: user A cannot read/update/delete user B's locations.

---

## Breaking Changes (changelog entries)

- **API request**: `POST/PUT /api/locations` body now requires `type` (one of: `storage`, `maintenance_shop`, `fuel_station`, `inspection`, `other`). Optional `latitude`/`longitude`.
- **API request**: `POST/PUT /api/motorcycles/{id}/maintenance` no longer accepts `inspectionLocation`, `locationName`, `latitude`, `longitude`. Use `locationId` only (create the Location first via `/api/locations`).
- **API response**: `GET /api/motorcycles/{id}` field `maintenanceLocations` now returns full Location objects (`{id, name, type, latitude, longitude, ...}`) instead of `{name, latitude, longitude}` triples.
- **DB**: four columns dropped from `maintenanceRecords`. Existing data is preserved by being migrated to Location rows; some duplication may occur where free-text names varied.

---

## Verification

1. `cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
2. `cargo test` — full suite green, including the new `tests/locations_test.rs`.
3. Manual smoke against a copy of `db.sqlite.bak`:
   - `sqlite3 db.sqlite.test < migrations/005_locations_typed_coords.sql` runs clean.
   - `SELECT COUNT(*) FROM maintenanceRecords WHERE locationId IS NULL AND <old field set>` → 0 after migration (every record that *had* location data now has a `locationId`).
   - `SELECT type, COUNT(*) FROM locations GROUP BY type` — sanity check the typed split.
4. Boot the API against the migrated DB, hit `/api/motorcycles/{id}` and `/api/locations`, confirm shapes match this plan.

---

## Out of scope (explicitly punted)

- Geocoding / reverse-geocoding from address.
- Auto-dedup of similarly-named Location rows post-migration.
- Sharing locations between users (everything stays strictly user-scoped, matching `[[project_motomanager]]`'s auth model).
- Map UI changes — backend-only refactor.
