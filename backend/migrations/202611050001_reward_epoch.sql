-- Bumped every time one of the user's drinks is undone. Reward writes
-- (crates, achievement unlocks) computed from data read before the bump are
-- skipped, so a sync that still saw the deleted drink cannot bring back the
-- rewards the undo revoked.
ALTER TABLE users ADD COLUMN reward_epoch INTEGER NOT NULL DEFAULT 0;
