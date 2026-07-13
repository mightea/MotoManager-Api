# Database Backup Proposal — MotoManager API

**Status:** Implemented (in-backend) · **Target env:** Docker on a Linux host/VPS · **Offsite:** none yet (local disk, phase 1)

> **Implemented as an in-process feature** (chosen over the external sidecar so
> admins can monitor and trigger backups from the webapp). See §7 at the bottom
> for exactly what shipped; §2–§4 explain the underlying mechanics, which the
> implementation follows.

## 1. What we're protecting

The API stores everything under the `/app/data` volume (`DATA_DIR`):

| Path | Contents | Backup? | Notes |
|------|----------|---------|-------|
| `data/db.sqlite` (+ `-wal`, `-shm`) | The SQLite database | **Yes — critical** | Runs in **WAL mode** (`src/main.rs:43`). Recent commits live in the `-wal` sibling, so a plain `cp db.sqlite` can produce a stale or torn copy. |
| `data/images/` | Uploaded part/motorcycle images | **Yes** | Referenced by DB rows — losing these orphans records. |
| `data/documents/` | Uploaded PDFs / invoices | **Yes** | Same — referenced by DB rows. |
| `cache/` (`CACHE_DIR`) | Regenerable derived files | **No** | Rebuilt on demand; skip to keep backups small. |
| `.env` / deploy secrets | `DATABASE_URL`, `LLM_*`, WebAuthn `RP_ID`/`ORIGIN` | **Yes, separately** | Not in the DB. Store once in a password manager / secrets store, not in every snapshot. |

**Key constraint:** the DB and the files under `data/` are *coupled* — DB rows point at files on disk. A restore is only useful if both come from (roughly) the same moment.

## 2. The one rule that matters for SQLite + WAL

Do **not** `cp`/`rsync` the live `db.sqlite` file. Use SQLite's **online backup**, which takes a consistent snapshot while the API keeps running — no downtime, no locking the app out:

```sh
sqlite3 /path/to/data/db.sqlite ".backup '/path/to/snapshot.sqlite'"
```

`.backup` (and the equivalent `VACUUM INTO`) opens its own connection, coordinates with the running server via SQLite's locking + `busy_timeout`, and writes a single clean file with the WAL already folded in. That single file is the whole database — no `-wal`/`-shm` needed alongside it.

Because the runtime container image doesn't ship the `sqlite3` CLI, run it **on the host** against the volume's file (recommended), or add `sqlite3` to the runtime image and `docker exec`. Host-side is simpler and keeps backups working even if the container is unhealthy.

### Consistency between DB and files

Order: **snapshot the DB first, then rsync `images/` + `documents/`.** Uploaded files are additive (new upload → new id), so every file the DB snapshot references already exists on disk. A file written *after* the snapshot just ends up unreferenced (harmless). This gives a restore that never has a dangling DB reference.

## 3. Proposed mechanism

A single host script, run on a schedule by a **systemd timer** (preferred over cron: logs to the journal, `Persistent=true` catches missed runs after downtime).

- **Frequency:** daily at 03:00, plus one on-demand path (`backup.sh now`) to run before deploys/migrations.
- **Snapshot format:** DB snapshot + files tarred and `zstd`-compressed into one dated archive.
- **Integrity gate:** every snapshot is verified with `PRAGMA integrity_check` before it's kept — a backup that doesn't restore is worse than none.
- **Retention (GFS):** 14 daily · 8 weekly · 12 monthly, pruned automatically.
- **Location (phase 1):** a **separate disk/mount** on the host if one exists (e.g. `/mnt/backup`), *not* the same volume as `data/` — otherwise one disk failure loses both.

### `backup.sh` (reference implementation)

```sh
#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="${DATA_DIR:-/srv/motomanager/data}"   # host path of the /app/data volume
DEST="${DEST:-/mnt/backup/motomanager}"          # ideally a different disk than DATA_DIR
STAMP="$(date +%Y%m%d-%H%M%S)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$DEST"

# 1. Consistent, online DB snapshot (WAL-safe)
sqlite3 "$DATA_DIR/db.sqlite" ".backup '$WORK/db.sqlite'"

# 2. Verify it before trusting it
if [ "$(sqlite3 "$WORK/db.sqlite" 'PRAGMA integrity_check;')" != "ok" ]; then
  echo "integrity_check FAILED — aborting, keeping previous backups" >&2
  exit 1
fi

# 3. Bundle DB snapshot + uploaded files into one compressed archive
tar -C "$WORK" -c db.sqlite \
    -C "$DATA_DIR" images documents \
  | zstd -q -o "$DEST/motomanager-$STAMP.tar.zst"

# 4. Retention: keep 14 daily, then thin to weekly/monthly
#    (simple version: keep newest 14 dailies; see notes for GFS pruning)
ls -1t "$DEST"/motomanager-*.tar.zst | tail -n +15 | xargs -r rm -f

echo "backup ok: $DEST/motomanager-$STAMP.tar.zst"
```

### systemd units

```ini
# /etc/systemd/system/motomanager-backup.service
[Unit]
Description=MotoManager DB + files backup
[Service]
Type=oneshot
ExecStart=/usr/local/bin/motomanager-backup.sh
```

```ini
# /etc/systemd/system/motomanager-backup.timer
[Unit]
Description=Daily MotoManager backup
[Timer]
OnCalendar=*-*-* 03:00:00
Persistent=true
[Install]
WantedBy=timers.target
```

```sh
sudo systemctl enable --now motomanager-backup.timer
```

## 4. Restore procedure (and drill it!)

A backup you've never restored is a hypothesis. Test it once now and after any big change:

```sh
# 1. Stop the API so nothing writes during restore
docker compose stop api           # or: docker stop <container>

# 2. Unpack the chosen archive
mkdir -p /tmp/restore && zstd -dc motomanager-YYYYMMDD-HHMMSS.tar.zst | tar -C /tmp/restore -x

# 3. Put files back (the .backup output IS the whole DB — no -wal/-shm)
cp /tmp/restore/db.sqlite      "$DATA_DIR/db.sqlite"
rm -f "$DATA_DIR/db.sqlite-wal" "$DATA_DIR/db.sqlite-shm"   # clear any stale WAL
rsync -a --delete /tmp/restore/images/    "$DATA_DIR/images/"
rsync -a --delete /tmp/restore/documents/ "$DATA_DIR/documents/"

# 4. Start and verify
docker compose start api
```

On startup the app re-enables WAL and runs migrations, so restoring an older schema onto a newer binary migrates forward automatically.

## 5. Rollout

1. Create the backup dir (ideally on a second disk), drop in `backup.sh` + the two systemd units.
2. Run once manually, confirm the archive appears and `integrity_check` passes.
3. **Do a full restore drill into a throwaway/staging path** to prove the archive is usable end-to-end.
4. Enable the timer. Add a `backup.sh` call to your deploy script *before* migrations run.
5. Skim the journal weekly (`journalctl -u motomanager-backup.service`) for silent failures — or wire a healthcheck ping.

## 6. Known gap / next step

**Phase 1 keeps backups on the same host** (per current decision). This protects against app bugs, bad migrations, and accidental deletes — but **not** disk failure, theft, ransomware, or the box dying. The single highest-value follow-up is an **offsite copy**: the archives are already self-contained files, so shipping them to Backblaze B2 / Cloudflare R2 / S3 is one extra line (`rclone copy "$DEST" remote:motomanager-backups`) once you're ready. Recommend scheduling that as phase 2.

## 7. What actually shipped (in-backend implementation)

Rather than the external sidecar, the backup runs **inside the API process** so an admin can see and control it. The mechanics from §2–§4 still hold, with two substitutions that make it dependency-free: the snapshot uses SQLite **`VACUUM INTO`** (a plain SQL statement over the existing pool — no `sqlite3` binary) and the archive is **`.tar.gz`** via pure-Rust `tar`+`flate2` (no `zstd` C toolchain, so it cross-compiles cleanly for arm64/amd64).

**Backend** (`MotoManagerApi`):
- `src/backup.rs` — `perform_backup` (VACUUM INTO → `PRAGMA integrity_check` → tar.gz of `db.sqlite` + `images/` + `documents/` → prune), a `spawn_scheduler` loop whose cadence is derived from the last success in the DB (so restarts don't re-snapshot and long downtime triggers a catch-up), and `reset_stale_running` for rows orphaned by a crash.
- `migrations/033_backups.sql` — `backups` history table (one row per attempt: status, trigger, size, path, error).
- Admin endpoints (`src/handlers/backups.rs`, all `AdminUser`-gated):
  - `GET /api/admin/backups` — schedule config, running flag, last-success / next-scheduled, run history.
  - `POST /api/admin/backups` — back up now (409 if one is already running).
  - `GET /api/admin/backups/{id}/download` — stream the archive.
  - `DELETE /api/admin/backups/{id}` — remove archive + row.
- Config / env: `BACKUP_ENABLED` (gates the scheduler only), `BACKUP_INTERVAL_HOURS` (24), `BACKUP_KEEP` (14). Archives live in `DATA_DIR/backups`.
- Tests: `tests/backups_test.rs` exercises the full pipeline and asserts the archived `db.sqlite` contains committed data, plus retention pruning.

**Webapp** (`MotoManager`): a **Backups** admin page (`/settings/backups`, linked from `/settings/admin`) — status cards, a "Jetzt Backup erstellen" button, and a run-history table with per-row download/delete. Service in `app/services/backups.ts`.

**Restore** (the archive is gzip now, and `db.sqlite` inside it is the whole DB — no `-wal`/`-shm`):
```sh
docker compose stop api
mkdir -p /tmp/restore && tar -xzf motomanager-YYYYMMDD-HHMMSS.tar.gz -C /tmp/restore
cp /tmp/restore/db.sqlite "$DATA_DIR/db.sqlite"
rm -f "$DATA_DIR/db.sqlite-wal" "$DATA_DIR/db.sqlite-shm"
rsync -a --delete /tmp/restore/images/    "$DATA_DIR/images/"
rsync -a --delete /tmp/restore/documents/ "$DATA_DIR/documents/"
docker compose start api
```

Sections 2–6 above remain the reference for the *why* (WAL-safety, DB↔files coupling, retention, the offsite gap); the external sidecar they describe is now an **optional** independent/offsite layer, not the primary mechanism.
