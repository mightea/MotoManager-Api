-- Personal API tokens for the MCP server (/mcp). A token identifies exactly one
-- user and carries a scope; it is never a session and never grants admin
-- rights, so the MCP surface stays user-level even for administrators.
-- Only the SHA-256 hash of the secret is stored; the prefix is for display.
CREATE TABLE IF NOT EXISTS apiTokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    tokenHash TEXT UNIQUE NOT NULL,
    tokenPrefix TEXT NOT NULL,
    scope TEXT NOT NULL CHECK (scope IN ('read', 'write')),
    createdAt TEXT NOT NULL,
    lastUsedAt TEXT,
    expiresAt TEXT,
    revokedAt TEXT
);
CREATE INDEX IF NOT EXISTS idx_apiTokens_userId ON apiTokens(userId);

-- Every MCP tool call, so a user can review what an AI client did with a
-- token. Rows older than 90 days are pruned on insert.
CREATE TABLE IF NOT EXISTS mcpAuditLog (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    tokenId INTEGER NOT NULL REFERENCES apiTokens(id) ON DELETE CASCADE,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tool TEXT NOT NULL,
    arguments TEXT,
    outcome TEXT NOT NULL,
    detail TEXT,
    createdAt TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mcpAuditLog_userId_createdAt ON mcpAuditLog(userId, createdAt);
