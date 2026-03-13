# MotoManager API

Rust backend for MotoManager — a motorcycle maintenance and management application. Built with [Axum](https://github.com/tokio-rs/axum) and [SQLx](https://github.com/launchbadge/sqlx) against a SQLite database.

## Stack

- **Framework**: Axum 0.7
- **Database**: SQLite via SQLx 0.7 (runtime queries)
- **Auth**: Cookie-based sessions (`mb_session`) + WebAuthn passkey support
- **Password hashing**: scrypt (Node.js-compatible format)

## Setup

1. Copy the example env file and configure it:
   ```sh
   cp .env.example .env
   ```

2. Edit `.env`:
   ```
   DATABASE_URL=sqlite:./db.sqlite   # path to your SQLite database
   PORT=3001
   ORIGIN=http://localhost:3001      # frontend origin for CORS
   DATA_DIR=./data                   # where uploaded files are stored
   APP_VERSION=2026.1.0
   ENABLE_REGISTRATION=false         # allow new registrations (false = first user only)
   RP_ID=localhost                   # WebAuthn relying party ID
   RP_NAME=MotoManager
   ```

3. Build and run:
   ```sh
   cargo run
   ```

The server will run migrations on startup and create the `data/` directories automatically.

## API Routes

### Auth
| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth/login` | Log in with email/username + password |
| POST | `/api/auth/logout` | Log out (clears session cookie) |
| POST | `/api/auth/register` | Register a new account |
| GET | `/api/auth/me` | Get the current user |

### Motorcycles
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/motorcycles` | List all motorcycles |
| POST | `/api/motorcycles` | Create a motorcycle |
| GET | `/api/motorcycles/:id` | Get a motorcycle |
| PUT | `/api/motorcycles/:id` | Update a motorcycle |
| DELETE | `/api/motorcycles/:id` | Delete a motorcycle |

### Maintenance Records
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/motorcycles/:id/maintenance` | List maintenance records |
| POST | `/api/motorcycles/:id/maintenance` | Create a maintenance record |
| PUT | `/api/motorcycles/:id/maintenance/:mid` | Update a maintenance record |
| DELETE | `/api/motorcycles/:id/maintenance/:mid` | Delete a maintenance record |

### Issues
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/motorcycles/:id/issues` | List issues |
| POST | `/api/motorcycles/:id/issues` | Create an issue |
| PUT | `/api/motorcycles/:id/issues/:issue_id` | Update an issue |
| DELETE | `/api/motorcycles/:id/issues/:issue_id` | Delete an issue |

### Torque Specs
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/motorcycles/:id/torque-specs` | List torque specs |
| POST | `/api/motorcycles/:id/torque-specs` | Create a torque spec |
| POST | `/api/motorcycles/:id/torque-specs/import` | Import specs from another motorcycle |
| PUT | `/api/motorcycles/:id/torque-specs/:tid` | Update a torque spec |
| DELETE | `/api/motorcycles/:id/torque-specs/:tid` | Delete a torque spec |

### Previous Owners
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/motorcycles/:id/previous-owners` | List previous owners |
| POST | `/api/motorcycles/:id/previous-owners` | Add a previous owner |
| PUT | `/api/motorcycles/:id/previous-owners/:oid` | Update a previous owner |
| DELETE | `/api/motorcycles/:id/previous-owners/:oid` | Delete a previous owner |

### Documents
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/documents` | List documents (own private + all public) |
| POST | `/api/documents` | Upload a document (multipart) |
| PUT | `/api/documents/:doc_id` | Update a document |
| DELETE | `/api/documents/:doc_id` | Delete a document |

### Locations
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/locations` | List locations |
| POST | `/api/locations` | Create a location |
| PUT | `/api/locations/:lid` | Update a location |
| DELETE | `/api/locations/:lid` | Delete a location |

### Settings
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/settings` | Get user settings + authenticators |
| PUT | `/api/settings` | Update user settings |
| POST | `/api/settings/change-password` | Change password |
| DELETE | `/api/settings/authenticators/:id` | Remove a passkey |

### Admin
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/admin/users` | List all users |
| POST | `/api/admin/users` | Create a user |
| PUT | `/api/admin/users/:uid` | Update a user |
| DELETE | `/api/admin/users/:uid` | Delete a user |
| GET | `/api/admin/currencies` | List currencies |
| POST | `/api/admin/currencies` | Add a currency |
| PUT | `/api/admin/currencies/:cid` | Update a currency |
| DELETE | `/api/admin/currencies/:cid` | Delete a currency |

### Public
| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/currencies` | List currencies (no auth required) |
| GET | `/api/stats` | App statistics |
| GET | `/api/health` | Health check |

### File Serving
| Method | Path | Description |
|--------|------|-------------|
| GET | `/images/:filename` | Serve a motorcycle image |
| GET | `/data/documents/:filename` | Serve a document file |
| GET | `/data/previews/:filename` | Serve a document preview |
