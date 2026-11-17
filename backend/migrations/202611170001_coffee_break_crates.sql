PRAGMA foreign_keys = ON;

-- Coffee breaks: a crate per day on which someone else drank within ten
-- minutes of one of your drinks. SQLite cannot alter a CHECK constraint, so
-- the grants table is rebuilt with the wider list.
CREATE TABLE lootbox_grants_v4 (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source TEXT NOT NULL CHECK (source IN (
        'achievement', 'shopping', 'stamp_card', 'cleaning',
        'daily', 'streak', 'advent', 'milestone', 'payment', 'wheel',
        'coffee_break'
    )),
    source_ref TEXT NOT NULL,
    granted_at INTEGER NOT NULL,
    UNIQUE (user_id, source, source_ref)
);

INSERT INTO lootbox_grants_v4 (id, user_id, source, source_ref, granted_at)
SELECT id, user_id, source, source_ref, granted_at FROM lootbox_grants;

DROP TABLE lootbox_grants;
ALTER TABLE lootbox_grants_v4 RENAME TO lootbox_grants;
