PRAGMA foreign_keys = OFF;

CREATE TEMP TABLE sessions_backup AS SELECT * FROM sessions;
CREATE TEMP TABLE consumptions_backup AS SELECT * FROM consumptions;
CREATE TEMP TABLE achievement_unlocks_backup AS SELECT * FROM achievement_unlocks;
CREATE TEMP TABLE payments_backup AS SELECT * FROM payments;
CREATE TEMP TABLE guest_identities_backup AS SELECT * FROM guest_identities;

CREATE TABLE users_new (
    id TEXT PRIMARY KEY,
    user_type TEXT NOT NULL CHECK (user_type IN ('member', 'guest')),
    gitlab_id INTEGER UNIQUE,
    email TEXT,
    guest_recovery_hash TEXT NULL,
    display_name TEXT NOT NULL,
    leaderboard_opt_in INTEGER NOT NULL DEFAULT 0,
    achievement_sharing_opt_out INTEGER NOT NULL DEFAULT 0,
    mattermost_opt_in INTEGER NOT NULL DEFAULT 0,
    stock_manager INTEGER NOT NULL DEFAULT 0,
    paypal_link TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

INSERT INTO users_new (
    id,
    user_type,
    gitlab_id,
    email,
    guest_recovery_hash,
    display_name,
    leaderboard_opt_in,
    achievement_sharing_opt_out,
    mattermost_opt_in,
    stock_manager,
    paypal_link,
    created_at,
    updated_at
)
SELECT
    id,
    CASE user_type WHEN 'gitlab' THEN 'member' ELSE user_type END,
    gitlab_id,
    NULL,
    guest_recovery_hash,
    display_name,
    leaderboard_opt_in,
    achievement_sharing_opt_out,
    mattermost_opt_in,
    stock_manager,
    paypal_link,
    created_at,
    updated_at
FROM users;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

INSERT OR IGNORE INTO sessions
SELECT * FROM sessions_backup;

INSERT OR IGNORE INTO consumptions
SELECT * FROM consumptions_backup;

INSERT OR IGNORE INTO achievement_unlocks
SELECT * FROM achievement_unlocks_backup;

INSERT OR IGNORE INTO payments
SELECT * FROM payments_backup;

INSERT OR IGNORE INTO guest_identities
SELECT * FROM guest_identities_backup;

DROP TABLE sessions_backup;
DROP TABLE consumptions_backup;
DROP TABLE achievement_unlocks_backup;
DROP TABLE payments_backup;
DROP TABLE guest_identities_backup;

CREATE UNIQUE INDEX idx_users_email
ON users(email)
WHERE email IS NOT NULL;

CREATE UNIQUE INDEX idx_users_guest_recovery_hash
ON users(guest_recovery_hash)
WHERE guest_recovery_hash IS NOT NULL;

CREATE TABLE auth_identities (
    provider TEXT NOT NULL,
    subject TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (provider, subject)
);

INSERT INTO auth_identities (provider, subject, user_id, created_at, updated_at)
SELECT 'gitlab', CAST(gitlab_id AS TEXT), id, created_at, updated_at
FROM users
WHERE gitlab_id IS NOT NULL;

PRAGMA foreign_keys = ON;
