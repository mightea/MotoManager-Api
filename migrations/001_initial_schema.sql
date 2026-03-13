CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT UNIQUE NOT NULL,
    username TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    passwordHash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'user',
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updatedAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    lastLoginAt TEXT
);

CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token TEXT UNIQUE NOT NULL,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expiresAt TEXT NOT NULL,
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS authenticators (
    id TEXT PRIMARY KEY,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    publicKey BLOB NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    deviceType TEXT NOT NULL,
    backedUp INTEGER NOT NULL DEFAULT 0,
    transports TEXT,
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS challenges (
    id TEXT PRIMARY KEY,
    userId INTEGER,
    challenge TEXT NOT NULL,
    expiresAt TEXT NOT NULL,
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS motorcycles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    make TEXT NOT NULL,
    model TEXT NOT NULL,
    fabricationDate TEXT NOT NULL,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vin TEXT,
    engineNumber TEXT,
    vehicleIdNr TEXT,
    numberPlate TEXT,
    image TEXT,
    isVeteran INTEGER NOT NULL DEFAULT 0,
    isArchived INTEGER NOT NULL DEFAULT 0,
    firstRegistration TEXT,
    initialOdo INTEGER NOT NULL DEFAULT 0,
    manualOdo INTEGER,
    purchaseDate TEXT,
    purchasePrice REAL,
    normalizedPurchasePrice REAL,
    currencyCode TEXT,
    fuelTankSize REAL
);

CREATE TABLE IF NOT EXISTS maintenanceRecords (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    odo INTEGER NOT NULL,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
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
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    odo INTEGER NOT NULL,
    description TEXT,
    priority TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'new',
    date TEXT NOT NULL DEFAULT CURRENT_DATE
);

CREATE TABLE IF NOT EXISTS locations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    countryCode TEXT NOT NULL DEFAULT 'CH',
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS locationRecords (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    locationId INTEGER NOT NULL REFERENCES locations(id),
    odometer INTEGER,
    date TEXT NOT NULL DEFAULT CURRENT_DATE
);

CREATE TABLE IF NOT EXISTS torqueSpecs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    name TEXT NOT NULL,
    torque REAL NOT NULL,
    torqueEnd REAL,
    variation REAL,
    toolSize TEXT,
    description TEXT,
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS userSettings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    userId INTEGER UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tireInterval INTEGER NOT NULL DEFAULT 5,
    batteryLithiumInterval INTEGER NOT NULL DEFAULT 5,
    batteryDefaultInterval INTEGER NOT NULL DEFAULT 3,
    engineOilInterval INTEGER NOT NULL DEFAULT 1,
    gearboxOilInterval INTEGER NOT NULL DEFAULT 2,
    finalDriveOilInterval INTEGER NOT NULL DEFAULT 2,
    forkOilInterval INTEGER NOT NULL DEFAULT 2,
    brakeFluidInterval INTEGER NOT NULL DEFAULT 2,
    coolantInterval INTEGER NOT NULL DEFAULT 2,
    chainInterval INTEGER NOT NULL DEFAULT 1,
    tireKmInterval INTEGER,
    engineOilKmInterval INTEGER,
    gearboxOilKmInterval INTEGER,
    finalDriveOilKmInterval INTEGER,
    forkOilKmInterval INTEGER,
    brakeFluidKmInterval INTEGER,
    coolantKmInterval INTEGER,
    chainKmInterval INTEGER,
    updatedAt TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    filePath TEXT NOT NULL,
    previewPath TEXT,
    uploadedBy TEXT,
    ownerId INTEGER REFERENCES users(id) ON DELETE CASCADE,
    isPrivate INTEGER NOT NULL DEFAULT 0,
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updatedAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS documentMotorcycles (
    documentId INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    PRIMARY KEY (documentId, motorcycleId)
);

CREATE TABLE IF NOT EXISTS previousOwners (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
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
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updatedAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS currencySettings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT UNIQUE NOT NULL,
    symbol TEXT NOT NULL,
    label TEXT,
    conversionFactor REAL NOT NULL DEFAULT 1.0,
    createdAt TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Insert default CHF currency
INSERT OR IGNORE INTO currencySettings (code, symbol, label, conversionFactor)
VALUES ('CHF', 'Fr.', 'Schweizer Franken', 1.0);
