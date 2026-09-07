PRAGMA foreign_keys = ON;

INSERT OR IGNORE INTO beverages (id, name, price_cents, caffeine_mg, created_at)
VALUES ('eis', 'Eis', 50, 0, unixepoch());

INSERT OR IGNORE INTO achievement_definitions
    (id, name, description, hint, criteria_kind, threshold, active, created_at)
VALUES
    ('first_espresso_pads', 'Pad pilot', 'Logged the first Espresso (pads).', 'Try an Espresso from pads.', 'beverage_count:espresso_pads', 1, 1, unixepoch()),
    ('espresso_pads_regular', 'Pad regular', 'Logged 10 Espresso (pads).', 'Log 10 Espresso pads.', 'beverage_count:espresso_pads', 10, 1, unixepoch()),
    ('first_eis', 'Eis break', 'Logged the first Eis.', 'Treat yourself to an Eis.', 'beverage_count:eis', 1, 1, unixepoch()),
    ('eis_regular', 'Freezer regular', 'Logged 10 Eis.', 'Log 10 Eis.', 'beverage_count:eis', 10, 1, unixepoch()),
    ('full_menu', 'Full menu', 'Logged every regular menu option.', 'Try Coffee, Espresso, and Eis.', 'regular_menu_variety', 3, 1, unixepoch()),
    ('two_week_streak', 'Two-week streak', 'Logged drinks on 14 consecutive days.', 'Keep a 14-day streak.', 'streak_days', 14, 1, unixepoch()),
    ('month_streak', 'Monthly ritual', 'Logged drinks on 30 consecutive days.', 'Keep a 30-day streak.', 'streak_days', 30, 1, unixepoch()),
    ('busy_day_10', 'Busy day', 'Logged 10 drinks in one day.', 'Log 10 drinks in one day.', 'single_day_count', 10, 1, unixepoch()),
    ('double_century', 'Double century', 'Logged 200 coffees.', 'Reach 200 coffees.', 'lifetime_cups', 200, 1, unixepoch()),
    ('five_hundred', 'Five hundred', 'Logged 500 coffees.', 'Reach 500 coffees.', 'lifetime_cups', 500, 1, unixepoch()),
    ('fifth_payment', 'Payment regular', 'Marked five coffee payments as paid.', 'Settle five coffee tabs.', 'payment_count', 5, 1, unixepoch()),
    ('ten_euro_settlement', 'Big clean slate', 'Paid off at least 10 EUR in one payment.', 'Settle a tab of at least 10 EUR.', 'single_payment_amount', 1000, 1, unixepoch());
