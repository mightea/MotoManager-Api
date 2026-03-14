CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    username TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    last_login_at TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS authenticators (
    id TEXT PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    public_key BLOB NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    device_type TEXT NOT NULL,
    backed_up INTEGER NOT NULL DEFAULT false,
    transports TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS challenges (
    id TEXT PRIMARY KEY NOT NULL,
    user_id INTEGER,
    challenge TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS motorcycles (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    model_year TEXT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vin TEXT,
    engine_number TEXT,
    vehicle_nr TEXT,
    number_plate TEXT,
    image TEXT,
    is_veteran INTEGER NOT NULL DEFAULT false,
    is_archived INTEGER NOT NULL DEFAULT false,
    firstRegistration TEXT,
    initialOdo INTEGER NOT NULL DEFAULT 0,
    manual_odo INTEGER DEFAULT 0,
    purchase_date TEXT,
    purchase_price REAL,
    normalized_purchase_price REAL,
    currency_code TEXT,
    fuel_tank_size REAL
);

CREATE TABLE IF NOT EXISTS maintenance_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    date TEXT NOT NULL,
    odo INTEGER NOT NULL,
    motorcycle_id INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE NO ACTION,
    cost REAL,
    normalized_cost REAL,
    currency TEXT,
    description TEXT,
    type TEXT NOT NULL,
    brand TEXT,
    model TEXT,
    tire_position TEXT,
    tire_size TEXT,
    dot_code TEXT,
    battery_type TEXT,
    fluid_type TEXT,
    viscosity TEXT,
    oil_type TEXT,
    inspection_location TEXT,
    location_id INTEGER REFERENCES locations(id),
    fuel_type TEXT,
    fuel_amount REAL,
    price_per_unit REAL,
    latitude REAL,
    longitude REAL,
    location_name TEXT,
    fuel_consumption REAL,
    trip_distance REAL
);

CREATE TABLE IF NOT EXISTS issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycle_id INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE NO ACTION,
    odo INTEGER NOT NULL,
    description TEXT,
    priority TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'new',
    date TEXT DEFAULT (CURRENT_DATE)
);

CREATE TABLE IF NOT EXISTS locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    country_code TEXT NOT NULL DEFAULT 'CH'
);

CREATE TABLE IF NOT EXISTS location_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycle_id INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE NO ACTION,
    location_id INTEGER NOT NULL REFERENCES locations(id) ON DELETE NO ACTION,
    odometer INTEGER,
    date TEXT NOT NULL DEFAULT (CURRENT_DATE)
);

CREATE TABLE IF NOT EXISTS torque_specs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycle_id INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    name TEXT NOT NULL,
    torque REAL NOT NULL,
    torque_end REAL,
    variation REAL,
    tool_size TEXT,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS user_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tire_interval INTEGER NOT NULL DEFAULT 8,
    battery_lithium_interval INTEGER NOT NULL DEFAULT 10,
    battery_default_interval INTEGER NOT NULL DEFAULT 6,
    engine_oil_interval INTEGER NOT NULL DEFAULT 2,
    gearbox_oil_interval INTEGER NOT NULL DEFAULT 2,
    final_drive_oil_interval INTEGER NOT NULL DEFAULT 2,
    fork_oil_interval INTEGER NOT NULL DEFAULT 4,
    brake_fluid_interval INTEGER NOT NULL DEFAULT 4,
    coolant_interval INTEGER NOT NULL DEFAULT 4,
    chain_interval INTEGER NOT NULL DEFAULT 1,
    tire_km_interval INTEGER,
    engine_oil_km_interval INTEGER,
    gearbox_oil_km_interval INTEGER,
    final_drive_oil_km_interval INTEGER,
    fork_oil_km_interval INTEGER,
    brake_fluid_km_interval INTEGER,
    coolant_km_interval INTEGER,
    chain_km_interval INTEGER,
    updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    title TEXT NOT NULL,
    file_path TEXT NOT NULL,
    preview_path TEXT,
    uploaded_by TEXT,
    owner_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    is_private INTEGER NOT NULL DEFAULT false,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS document_motorcycles (
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    motorcycle_id INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    PRIMARY KEY (document_id, motorcycle_id)
);

CREATE TABLE IF NOT EXISTS previous_owners (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycle_id INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    surname TEXT NOT NULL,
    purchase_date TEXT NOT NULL,
    address TEXT,
    city TEXT,
    postcode TEXT,
    country TEXT,
    phone_number TEXT,
    email TEXT,
    comments TEXT,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS currencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    code TEXT UNIQUE NOT NULL,
    symbol TEXT NOT NULL,
    label TEXT,
    conversion_factor REAL NOT NULL DEFAULT 1.0,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

INSERT OR IGNORE INTO currencies (code, symbol, label, conversion_factor)
VALUES ('CHF', 'Fr.', 'Schweizer Franken', 1.0);

DROP TABLE IF EXISTS __drizzle_migrations;
