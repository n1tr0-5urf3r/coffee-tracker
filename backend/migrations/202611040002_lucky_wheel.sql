PRAGMA foreign_keys = ON;

-- Lucky wheel: every draw is one row (the ledger doubles as the statistics).
-- Paid spins carry their price and land on the user's open tab; free spins
-- (welcome spin, re-spin vouchers) cost nothing. A won free drink is redeemed
-- by the next priced drink the user logs; deleting that drink clears the link
-- again, so the voucher comes back.
CREATE TABLE wheel_spins (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    spun_at INTEGER NOT NULL,
    week_key TEXT NOT NULL,
    cost_cents INTEGER NOT NULL CHECK (cost_cents >= 0),
    source TEXT NOT NULL CHECK (source IN ('welcome', 'voucher', 'paid')),
    prize TEXT NOT NULL CHECK (prize IN (
        'nothing', 'oh_no', 'crate_1', 'crate_2', 'crate_3', 'free_drink', 'respin'
    )),
    artwork_id TEXT,
    redeemed_consumption_id TEXT REFERENCES consumptions(id) ON DELETE SET NULL
);

CREATE INDEX idx_wheel_spins_user_week ON wheel_spins(user_id, week_key);
CREATE INDEX idx_wheel_spins_user_time ON wheel_spins(user_id, spun_at);
CREATE INDEX idx_wheel_spins_redeemed ON wheel_spins(redeemed_consumption_id);
-- At most one welcome spin per user.
CREATE UNIQUE INDEX idx_wheel_spins_welcome ON wheel_spins(user_id) WHERE source = 'welcome';

-- Wheel prizes are crates too. SQLite cannot alter a CHECK constraint, so the
-- grants table is rebuilt with the wider list.
CREATE TABLE lootbox_grants_v3 (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source TEXT NOT NULL CHECK (source IN (
        'achievement', 'shopping', 'stamp_card', 'cleaning',
        'daily', 'streak', 'advent', 'milestone', 'payment', 'wheel'
    )),
    source_ref TEXT NOT NULL,
    granted_at INTEGER NOT NULL,
    UNIQUE (user_id, source, source_ref)
);

INSERT INTO lootbox_grants_v3 (id, user_id, source, source_ref, granted_at)
SELECT id, user_id, source, source_ref, granted_at FROM lootbox_grants;

DROP TABLE lootbox_grants;
ALTER TABLE lootbox_grants_v3 RENAME TO lootbox_grants;
