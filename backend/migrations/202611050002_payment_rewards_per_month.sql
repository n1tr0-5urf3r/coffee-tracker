-- Payment crates are now earned once per month instead of once per
-- payment, so splitting a tab into many 1 EUR payments cannot farm crates.
-- Existing grants move to the month key; extra grants from the same month
-- keep their old payment-id key (crates already earned are never taken away).
-- The app keys months in the configured timezone; SQL only knows UTC, which
-- differs just for payments in the first hours of a month.
UPDATE OR IGNORE lootbox_grants
SET source_ref = strftime('%Y-%m', granted_at, 'unixepoch')
WHERE source = 'payment';

-- Payment achievements now count months with a payment, not single payments.
UPDATE achievement_definitions
SET description = 'Settled your first coffee tab.'
WHERE id = 'first_payment';
UPDATE achievement_definitions
SET description = 'Settled your coffee tab in three different months.',
    hint = 'Settle your tab in three different months.'
WHERE id = 'third_payment';
UPDATE achievement_definitions
SET description = 'Settled your coffee tab in five different months.',
    hint = 'Settle your tab in five different months.'
WHERE id = 'fifth_payment';
UPDATE achievement_definitions
SET description = 'Settled your coffee tab in 10 different months.',
    hint = 'Settle your tab in 10 different months.'
WHERE id = 'tenth_payment';
