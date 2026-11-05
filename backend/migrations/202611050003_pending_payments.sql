-- A payment someone marks as received by another person stays `pending`
-- until that recipient confirms it. Pending payments close the tab (so it is
-- not billed twice) but don't count as money received by the recipient and
-- earn no rewards. Rejecting one deletes it and reopens the tab.
ALTER TABLE payments ADD COLUMN status TEXT NOT NULL DEFAULT 'confirmed'
    CHECK (status IN ('pending', 'confirmed'));
ALTER TABLE payments ADD COLUMN confirmed_at INTEGER;
UPDATE payments SET confirmed_at = paid_at;

-- At most one payment per user awaits confirmation, so a rejected one is
-- always the latest and deleting it cleanly reopens the tab.
CREATE UNIQUE INDEX idx_payments_one_pending ON payments(user_id) WHERE status = 'pending';
