PRAGMA foreign_keys = ON;

INSERT OR IGNORE INTO beverages (id, name, price_cents, caffeine_mg, created_at)
VALUES ('tea', 'Tea', 25, 0, unixepoch());

INSERT OR IGNORE INTO stock_items
    (id, name, unit, current_amount, critical_amount, decrement_per_log, alerted, updated_at)
VALUES
    ('tea', 'Tea', 'kg', 0, 0, 0, 1, unixepoch());

INSERT OR IGNORE INTO achievement_definitions
    (id, name, description, hint, criteria_kind, threshold, active, created_at)
VALUES
    ('first_tea', 'Tea time', 'Logged the first Tea.', 'Try a Tea.', 'beverage_count:tea', 1, 1, unixepoch()),
    ('tea_regular', 'Tea regular', 'Logged 10 Tea.', 'Log 10 Tea.', 'beverage_count:tea', 10, 1, unixepoch()),
    ('tea_fan', 'Tea fan', 'Logged 50 Tea.', 'Log 50 Tea.', 'beverage_count:tea', 50, 1, unixepoch());

UPDATE achievement_definitions
SET description = 'Logged every regular menu option.',
    hint = 'Try Coffee, Espresso, Eis, and Tea.',
    threshold = 4
WHERE id = 'full_menu';
