PRAGMA foreign_keys = ON;

-- Default everyone to opted-in for the leaderboard and Mattermost achievement
-- pings. Existing users get backfilled so they match the new default.
UPDATE users SET leaderboard_opt_in = 1 WHERE leaderboard_opt_in = 0;
UPDATE users SET mattermost_opt_in = 1 WHERE mattermost_opt_in = 0;

-- Payment tracking. A payment record settles every consumption strictly
-- before `period_end_ts` for the given user. We do not verify that the money
-- actually moved -- "mark as paid" simply files a record.
CREATE TABLE payments (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    amount_cents INTEGER NOT NULL,
    drink_count INTEGER NOT NULL,
    period_end_ts INTEGER NOT NULL,
    note TEXT,
    paid_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_payments_user_paid_at ON payments(user_id, paid_at DESC);
