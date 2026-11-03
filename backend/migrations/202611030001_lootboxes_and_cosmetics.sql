PRAGMA foreign_keys = ON;

-- Every lootbox a user has earned. `source_ref` makes grants idempotent:
-- an achievement, a shopping expense, a filled stamp card or a weekly machine
-- cleaning grants exactly one box.
CREATE TABLE lootbox_grants (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source TEXT NOT NULL CHECK (source IN ('achievement', 'shopping', 'stamp_card', 'cleaning')),
    source_ref TEXT NOT NULL,
    granted_at INTEGER NOT NULL,
    UNIQUE (user_id, source, source_ref)
);

CREATE TABLE lootbox_openings (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    rarity TEXT NOT NULL,
    duplicate INTEGER NOT NULL DEFAULT 0,
    beans_awarded INTEGER NOT NULL DEFAULT 0,
    opened_at INTEGER NOT NULL
);

CREATE INDEX idx_lootbox_openings_user ON lootbox_openings(user_id, opened_at DESC);

CREATE TABLE cosmetic_crafts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    beans_spent INTEGER NOT NULL CHECK (beans_spent > 0),
    crafted_at INTEGER NOT NULL
);

CREATE INDEX idx_cosmetic_crafts_user ON cosmetic_crafts(user_id);

-- Items are defined in code (src/cosmetics/catalog.rs); only ownership lives here.
CREATE TABLE user_cosmetics (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('lootbox', 'craft')),
    acquired_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, item_id)
);

CREATE TABLE user_equipped_cosmetics (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    slot TEXT NOT NULL,
    item_id TEXT NOT NULL,
    PRIMARY KEY (user_id, slot)
);

-- Trust-based "I cleaned the coffee machine" log. Only the first cleaning of a
-- local ISO week (`week_key`, e.g. 2026-W45) earns a lootbox, across all users.
CREATE TABLE machine_cleanings (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cleaned_at INTEGER NOT NULL,
    week_key TEXT NOT NULL,
    rewarded INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_machine_cleanings_cleaned_at ON machine_cleanings(cleaned_at DESC);
CREATE INDEX idx_machine_cleanings_user ON machine_cleanings(user_id, cleaned_at DESC);
CREATE UNIQUE INDEX idx_machine_cleanings_weekly_reward
ON machine_cleanings(week_key) WHERE rewarded = 1;
