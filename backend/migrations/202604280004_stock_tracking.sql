PRAGMA foreign_keys = ON;

ALTER TABLE users ADD COLUMN stock_manager INTEGER NOT NULL DEFAULT 0;

UPDATE beverages
SET name = 'Coffee (Vollautomat)', price_cents = 50
WHERE id = 'coffee';

INSERT OR IGNORE INTO beverages (id, name, price_cents, caffeine_mg, created_at)
VALUES ('espresso_pads', 'Espresso (pads)', 50, 90, unixepoch());

CREATE TABLE stock_items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    unit TEXT NOT NULL CHECK (unit IN ('count', 'kg')),
    current_amount REAL NOT NULL DEFAULT 0,
    critical_amount REAL NOT NULL DEFAULT 0,
    decrement_per_log REAL NOT NULL DEFAULT 0,
    alerted INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

INSERT INTO stock_items
    (id, name, unit, current_amount, critical_amount, decrement_per_log, alerted, updated_at)
VALUES
    ('beans', 'Beans', 'kg', 0, 0.5, 0, 1, unixepoch()),
    ('espresso_pads', 'Espresso pads', 'count', 0, 10, 1, 1, unixepoch());
