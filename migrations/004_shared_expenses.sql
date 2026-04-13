-- Create expenses and junction table for shared costs
CREATE TABLE IF NOT EXISTS expenses (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    amount REAL NOT NULL,
    currency TEXT NOT NULL,
    category TEXT NOT NULL,
    description TEXT,
    intervalMonths INTEGER,
    createdAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updatedAt TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE IF NOT EXISTS expenseMotorcycles (
    expenseId INTEGER NOT NULL REFERENCES expenses(id) ON DELETE CASCADE,
    motorcycleId INTEGER NOT NULL REFERENCES motorcycles(id) ON DELETE CASCADE,
    PRIMARY KEY (expenseId, motorcycleId)
);

CREATE INDEX IF NOT EXISTS idx_expenses_userId ON expenses(userId);
CREATE INDEX IF NOT EXISTS idx_expenseMotorcycles_motorcycleId ON expenseMotorcycles(motorcycleId);
