PRAGMA foreign_keys = ON;

-- First time a user's locker was synced. Every unlocked achievement earns a
-- crate (old ones included); shopping trips and full stamp cards only earn
-- crates after this moment, so past ones are not paid out in bulk.
CREATE TABLE lootbox_profiles (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    onboarded_at INTEGER NOT NULL
);
