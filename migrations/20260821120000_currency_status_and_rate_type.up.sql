-- Currency lifecycle: is_active boolean -> CurrencyStatus enum (active|inactive), the house
-- lifecycle shape (an inactive currency is hidden from new documents while history keeps its
-- booked amounts) instead of a bare flag.
-- CurrencyExchange gains rate provenance: rate_type (what the rate is — a point-in-time spot,
-- a period average, or a period-end closing rate) and source (where it came from).

-- The enum types themselves are created by the preceding create_enums migration (guarded,
-- UNQUALIFIED public types). In a shared database those guards ADOPT types another module
-- already created — that adoption is the cross-module rollover contract, never widened here:
-- both modules must keep declaring identical variant sets.

ALTER TABLE corporate.currencies
    ADD COLUMN status currency_status NOT NULL DEFAULT 'active';

-- Backfill: every existing row keeps the truth its flag carried.
UPDATE corporate.currencies
   SET status = CASE WHEN is_active THEN 'active'::currency_status ELSE 'inactive'::currency_status END;

ALTER TABLE corporate.currencies DROP COLUMN is_active;

DROP INDEX IF EXISTS idx_currencies_is_active;
CREATE INDEX IF NOT EXISTS idx_currencies_status ON corporate.currencies (status);

-- Rate provenance. Existing rows are unstamped (source stays NULL — provenance unknown is
-- honest; rows copied from a retired rate table during a rollover are stamped by that copy).
ALTER TABLE corporate.currency_exchanges
    ADD COLUMN rate_type rate_type NOT NULL DEFAULT 'spot',
    ADD COLUMN source TEXT;
