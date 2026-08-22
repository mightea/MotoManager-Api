-- Last iOS app version seen for this user, taken from the X-App-Version /
-- X-App-Build request headers by the auth extractor. Written only when the
-- value actually changes (roughly once per app update per user), never
-- cleared by clients that send no headers (webapp, older app builds).
-- NULL means the user has never connected with a header-sending app build.
ALTER TABLE users ADD COLUMN appVersion TEXT;
ALTER TABLE users ADD COLUMN appBuild INTEGER;
