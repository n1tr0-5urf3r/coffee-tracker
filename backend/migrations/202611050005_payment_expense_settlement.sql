-- The amount due is the coffee tab minus the payer's expense balance, but a
-- payment only booked the tab: a negative expense balance (Kasse money in the
-- payer's hands) was paid but never cleared, a positive one credited the
-- recipient with money that never moved. This is the part of a payment that
-- settles the payer's expense balance; the recipient receives
-- amount_cents + expense_settled_cents.
ALTER TABLE payments ADD COLUMN expense_settled_cents INTEGER NOT NULL DEFAULT 0;
