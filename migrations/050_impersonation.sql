-- Admin impersonation: a session created on behalf of another user records
-- who is acting. NULL for every regular session, so existing sessions and
-- clients are unaffected. The row doubles as the audit trail while active;
-- start/end are additionally logged server-side.
ALTER TABLE sessions ADD COLUMN impersonatorId INTEGER REFERENCES users(id);
