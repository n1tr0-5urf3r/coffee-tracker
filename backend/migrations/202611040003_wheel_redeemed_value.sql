-- Value of a redeemed lucky-wheel free drink, frozen when it is used, so the
-- staff statistics don't change when beverage prices do later.
ALTER TABLE wheel_spins ADD COLUMN redeemed_value_cents INTEGER;

UPDATE wheel_spins
SET redeemed_value_cents = (
    SELECT b.price_cents
    FROM consumptions c
    JOIN beverages b ON b.id = c.beverage_id
    WHERE c.id = wheel_spins.redeemed_consumption_id
)
WHERE redeemed_consumption_id IS NOT NULL;
