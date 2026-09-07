PRAGMA foreign_keys = ON;

CREATE TABLE users (
    id TEXT PRIMARY KEY,
    user_type TEXT NOT NULL CHECK (user_type IN ('gitlab', 'guest')),
    gitlab_id INTEGER UNIQUE,
    display_name TEXT NOT NULL,
    leaderboard_opt_in INTEGER NOT NULL DEFAULT 0,
    achievement_sharing_opt_out INTEGER NOT NULL DEFAULT 0,
    mattermost_opt_in INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE sessions (
    id_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE TABLE machines (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    location TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE beverages (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    price_cents INTEGER NOT NULL,
    caffeine_mg INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE consumptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    machine_id TEXT NOT NULL REFERENCES machines(id),
    beverage_id TEXT NOT NULL REFERENCES beverages(id),
    consumed_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_consumptions_user_time ON consumptions(user_id, consumed_at DESC);
CREATE INDEX idx_consumptions_time ON consumptions(consumed_at);

CREATE TABLE achievement_definitions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    hint TEXT NOT NULL,
    criteria_kind TEXT NOT NULL,
    threshold INTEGER,
    active INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE achievement_unlocks (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    achievement_id TEXT NOT NULL REFERENCES achievement_definitions(id),
    unlocked_at INTEGER NOT NULL,
    UNIQUE (user_id, achievement_id)
);

CREATE INDEX idx_achievement_unlocks_user ON achievement_unlocks(user_id, unlocked_at DESC);
