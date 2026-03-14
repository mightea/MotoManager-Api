-- Drop all tables (both camelCase and legacy snake_case names) to ensure a clean slate.
-- Disable FK checks so drops succeed regardless of dependency order.
PRAGMA foreign_keys = OFF;

DROP TABLE IF EXISTS previousOwners;
DROP TABLE IF EXISTS previous_owners;
DROP TABLE IF EXISTS documentMotorcycles;
DROP TABLE IF EXISTS document_motorcycles;
DROP TABLE IF EXISTS documents;
DROP TABLE IF EXISTS userSettings;
DROP TABLE IF EXISTS user_settings;
DROP TABLE IF EXISTS torqueSpecs;
DROP TABLE IF EXISTS torque_specs;
DROP TABLE IF EXISTS locationRecords;
DROP TABLE IF EXISTS location_records;
DROP TABLE IF EXISTS maintenanceRecords;
DROP TABLE IF EXISTS maintenance_records;
DROP TABLE IF EXISTS issues;
DROP TABLE IF EXISTS locations;
DROP TABLE IF EXISTS motorcycles;
DROP TABLE IF EXISTS currencies;
DROP TABLE IF EXISTS challenges;
DROP TABLE IF EXISTS authenticators;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS users;

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    email TEXT UNIQUE NOT NULL,
    username TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    passwordHash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    lastLoginAt TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expiresAt TEXT NOT NULL,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS authenticators (
    id TEXT PRIMARY KEY NOT NULL,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    publicKey BLOB NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    deviceType TEXT NOT NULL,
    backedUp INTEGER NOT NULL DEFAULT false,
    transports TEXT,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS challenges (
    id TEXT PRIMARY KEY NOT NULL,
    userId INTEGER,
    challenge TEXT NOT NULL,
    expiresAt TEXT NOT NULL,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS motorcycles (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    modelYear TEXT,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vin TEXT,
    engineNumber TEXT,
    vehicleNr TEXT,
    numberPlate TEXT,
    image TEXT,
    isVeteran INTEGER NOT NULL DEFAULT false,
    isArchived INTEGER NOT NULL DEFAULT false,
    firstRegistration TEXT,
    initialOdo INTEGER NOT NULL DEFAULT 0,
    manualOdo INTEGER DEFAULT 0,
    purchaseDate TEXT,
    purchasePrice REAL,
    normalizedPurchasePrice REAL,
    currencyCode TEXT,
    fuelTankSize REAL
);

CREATE TABLE IF NOT EXISTS maintenanceRecords (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    date TEXT NOT NULL,
    odo INTEGER NOT NULL,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE NO ACTION,
    cost REAL,
    normalizedCost REAL,
    currency TEXT,
    description TEXT,
    type TEXT NOT NULL,
    brand TEXT,
    model TEXT,
    tirePosition TEXT,
    tireSize TEXT,
    dotCode TEXT,
    batteryType TEXT,
    fluidType TEXT,
    viscosity TEXT,
    oilType TEXT,
    inspectionLocation TEXT,
    locationId INTEGER REFERENCES locations(id),
    fuelType TEXT,
    fuelAmount REAL,
    pricePerUnit REAL,
    latitude REAL,
    longitude REAL,
    locationName TEXT,
    fuelConsumption REAL,
    tripDistance REAL
);

CREATE TABLE IF NOT EXISTS issues (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE NO ACTION,
    odo INTEGER NOT NULL,
    description TEXT,
    priority TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'new',
    date TEXT DEFAULT (CURRENT_DATE)
);

CREATE TABLE IF NOT EXISTS locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    countryCode TEXT NOT NULL DEFAULT 'CH'
);

CREATE TABLE IF NOT EXISTS locationRecords (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE NO ACTION,
    locationId INTEGER NOT NULL REFERENCES locations(id) ON DELETE NO ACTION,
    odometer INTEGER,
    date TEXT NOT NULL DEFAULT (CURRENT_DATE)
);

CREATE TABLE IF NOT EXISTS torqueSpecs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    name TEXT NOT NULL,
    torque REAL NOT NULL,
    torqueEnd REAL,
    variation REAL,
    toolSize TEXT,
    description TEXT,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS userSettings (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    userId INTEGER UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tireInterval INTEGER NOT NULL DEFAULT 8,
    batteryLithiumInterval INTEGER NOT NULL DEFAULT 10,
    batteryDefaultInterval INTEGER NOT NULL DEFAULT 6,
    engineOilInterval INTEGER NOT NULL DEFAULT 2,
    gearboxOilInterval INTEGER NOT NULL DEFAULT 2,
    finalDriveOilInterval INTEGER NOT NULL DEFAULT 2,
    forkOilInterval INTEGER NOT NULL DEFAULT 4,
    brakeFluidInterval INTEGER NOT NULL DEFAULT 4,
    coolantInterval INTEGER NOT NULL DEFAULT 4,
    chainInterval INTEGER NOT NULL DEFAULT 1,
    tireKmInterval INTEGER,
    engineOilKmInterval INTEGER,
    gearboxOilKmInterval INTEGER,
    finalDriveOilKmInterval INTEGER,
    forkOilKmInterval INTEGER,
    brakeFluidKmInterval INTEGER,
    coolantKmInterval INTEGER,
    chainKmInterval INTEGER,
    updatedAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    title TEXT NOT NULL,
    filePath TEXT NOT NULL,
    previewPath TEXT,
    uploadedBy TEXT,
    ownerId INTEGER REFERENCES users(id) ON DELETE CASCADE,
    isPrivate INTEGER NOT NULL DEFAULT false,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS documentMotorcycles (
    documentId INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    PRIMARY KEY (documentId, motorcycleId)
);

CREATE TABLE IF NOT EXISTS previousOwners (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    surname TEXT NOT NULL,
    purchaseDate TEXT NOT NULL,
    address TEXT,
    city TEXT,
    postcode TEXT,
    country TEXT,
    phoneNumber TEXT,
    email TEXT,
    comments TEXT,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS currencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    code TEXT UNIQUE NOT NULL,
    symbol TEXT NOT NULL,
    label TEXT,
    conversionFactor REAL NOT NULL DEFAULT 1.0,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

INSERT OR IGNORE INTO currencies (code, symbol, label, conversionFactor)
VALUES ('CHF', 'Fr.', 'Schweizer Franken', 1.0);

DROP TABLE IF EXISTS __drizzle_migrations;
