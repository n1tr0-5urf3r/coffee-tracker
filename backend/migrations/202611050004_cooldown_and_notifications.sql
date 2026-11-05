-- When the user last logged a drink. The 5-second cooldown checks this
-- instead of the newest drink, so log + undo loops cannot skip it.
ALTER TABLE users ADD COLUMN last_logged_at INTEGER;
UPDATE users
SET last_logged_at = (SELECT MAX(consumed_at) FROM consumptions c WHERE c.user_id = users.id);

-- Achievements already announced in the team chat. An undo revokes an unlock
-- and logging again re-earns it; this keeps the chat from hearing it twice.
CREATE TABLE achievement_notifications (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    achievement_id TEXT NOT NULL,
    notified_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, achievement_id)
);
INSERT OR IGNORE INTO achievement_notifications (user_id, achievement_id, notified_at)
SELECT user_id, achievement_id, unlocked_at FROM achievement_unlocks;
