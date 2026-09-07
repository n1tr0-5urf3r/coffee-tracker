PRAGMA foreign_keys = ON;

ALTER TABLE payments ADD COLUMN received_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX idx_payments_received_by_user_paid_at
    ON payments(received_by_user_id, paid_at DESC);
