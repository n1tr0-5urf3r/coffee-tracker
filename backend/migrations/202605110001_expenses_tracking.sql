PRAGMA foreign_keys = ON;

CREATE TABLE expense_entries (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_by_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    amount_cents INTEGER NOT NULL CHECK (amount_cents <> 0),
    logged_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_expense_entries_user_logged_at
ON expense_entries(user_id, logged_at DESC);
