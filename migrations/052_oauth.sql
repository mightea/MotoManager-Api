-- OAuth 2.1 authorization server for the MCP endpoint (phase 2 of the MCP
-- integration): lets connector-style clients (claude.ai, Claude Desktop, the
-- mobile apps) obtain an API token through a browser consent screen instead
-- of a pasted secret. Tokens issued this way live in apiTokens like personal
-- tokens (same middleware, same scope gate, same audit and revocation UI);
-- the extra columns record the grant and its rotating refresh token.

-- Dynamically registered clients (RFC 7591). Public clients (PKCE only) are
-- the norm; a client may ask for a secret, whose hash is stored here.
CREATE TABLE IF NOT EXISTS oauthClients (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    clientId TEXT UNIQUE NOT NULL,
    clientName TEXT NOT NULL,
    -- JSON array of exact-match redirect URIs.
    redirectUris TEXT NOT NULL,
    tokenEndpointAuthMethod TEXT NOT NULL
        CHECK (tokenEndpointAuthMethod IN ('none', 'client_secret_post', 'client_secret_basic')),
    clientSecretHash TEXT,
    clientUri TEXT,
    createdAt TEXT NOT NULL,
    lastUsedAt TEXT
);

-- Single-use authorization codes (10 minute lifetime), bound to the PKCE
-- challenge, redirect URI and requested resource. Pruned on insert.
CREATE TABLE IF NOT EXISTS oauthAuthorizationCodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    codeHash TEXT UNIQUE NOT NULL,
    clientId INTEGER NOT NULL REFERENCES oauthClients(id) ON DELETE CASCADE,
    userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('read', 'write')),
    redirectUri TEXT NOT NULL,
    codeChallenge TEXT NOT NULL,
    resource TEXT,
    createdAt TEXT NOT NULL,
    expiresAt TEXT NOT NULL,
    usedAt TEXT,
    -- Token minted from this code; revoked if the code is ever replayed.
    issuedTokenId INTEGER REFERENCES apiTokens(id) ON DELETE SET NULL
);

ALTER TABLE apiTokens ADD COLUMN kind TEXT NOT NULL DEFAULT 'personal';
ALTER TABLE apiTokens ADD COLUMN oauthClientId INTEGER REFERENCES oauthClients(id) ON DELETE SET NULL;
ALTER TABLE apiTokens ADD COLUMN refreshTokenHash TEXT;
ALTER TABLE apiTokens ADD COLUMN refreshExpiresAt TEXT;
-- The refresh token this one replaced. Presenting it again means the token
-- was stolen and replayed, so the whole grant is revoked (OAuth 2.1 §4.3.1).
ALTER TABLE apiTokens ADD COLUMN previousRefreshTokenHash TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_apiTokens_refreshTokenHash
    ON apiTokens(refreshTokenHash) WHERE refreshTokenHash IS NOT NULL;
