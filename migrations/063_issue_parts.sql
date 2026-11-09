-- Spare parts linked to a defect (issue): "this part fixes that Mangel".
-- A plain join table without sync columns — the iOS app does not sync it, and
-- linking/unlinking deliberately does not bump issues.updatedAt so the issue
-- sync protocol stays untouched. Ownership derives through both ends
-- (issues → motorcycles.userId, parts.userId). Both entities are normally
-- soft-deleted, so readers filter on deletedAt; the cascades only cover the
-- hard deletes (motorcycle cascade delete, account deletion).
CREATE TABLE issueParts (
    issueId INTEGER NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    partId INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    createdAt TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (issueId, partId)
);
CREATE INDEX idx_issue_parts_part ON issueParts(partId);
