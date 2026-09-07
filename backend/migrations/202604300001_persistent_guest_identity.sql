PRAGMA foreign_keys = ON;

ALTER TABLE users ADD COLUMN guest_recovery_hash TEXT NULL;

CREATE UNIQUE INDEX idx_users_guest_recovery_hash
ON users(guest_recovery_hash)
WHERE guest_recovery_hash IS NOT NULL;

CREATE TABLE guest_identities (
    id_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL
);
