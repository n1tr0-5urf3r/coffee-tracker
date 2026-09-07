PRAGMA foreign_keys = ON;

-- Add nullable paypal_link column to users for per-staff PayPal.me links
ALTER TABLE users ADD COLUMN paypal_link TEXT NULL;
