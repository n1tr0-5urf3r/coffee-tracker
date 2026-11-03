PRAGMA foreign_keys = ON;

-- More ways to earn crates: daily activity, streak bonuses, the Advent
-- calendar, drink milestones, paid tabs and more chores. SQLite cannot alter
-- a CHECK constraint, so the grants table is rebuilt with the wider list.
CREATE TABLE lootbox_grants_v2 (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    source TEXT NOT NULL CHECK (source IN (
        'achievement', 'shopping', 'stamp_card', 'cleaning',
        'daily', 'streak', 'advent', 'milestone', 'payment'
    )),
    source_ref TEXT NOT NULL,
    granted_at INTEGER NOT NULL,
    UNIQUE (user_id, source, source_ref)
);

INSERT INTO lootbox_grants_v2 (id, user_id, source, source_ref, granted_at)
SELECT id, user_id, source, source_ref, granted_at FROM lootbox_grants;

DROP TABLE lootbox_grants;
ALTER TABLE lootbox_grants_v2 RENAME TO lootbox_grants;

-- Machine cleanings become a small chores board. Existing rows are cleanings.
-- Each chore has its own weekly crate, still claimed by the first member.
ALTER TABLE machine_cleanings ADD COLUMN chore TEXT NOT NULL DEFAULT 'clean';

DROP INDEX idx_machine_cleanings_weekly_reward;
CREATE UNIQUE INDEX idx_machine_cleanings_weekly_reward
ON machine_cleanings(chore, week_key) WHERE rewarded = 1;
CREATE INDEX idx_machine_cleanings_user_chore
ON machine_cleanings(user_id, chore, cleaned_at DESC);
