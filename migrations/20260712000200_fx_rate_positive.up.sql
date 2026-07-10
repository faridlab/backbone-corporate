-- The FX maturity backstop (money-correctness): a rate MUST be strictly positive. `upsert_rate` checks this,
-- but the generic 12-endpoint CRUD stack (create / upsert / bulk_create) and any raw writer bypass that
-- service — and the schema's `@non_negative`/`@precision` were never emitted to DDL (the column is a bare
-- `rate NUMERIC NOT NULL`). Without this CHECK a `rate=0` row inserted through the CRUD API makes
-- `convert` return amount 0 (a foreign invoice booked as Rp 0) WITH a rate_id stamp — money silently
-- destroyed, worse than a NoRate error; a negative rate flips a receivable into a payable. The CHECK closes
-- every write path at the one trust boundary that matters.
ALTER TABLE corporate.currency_exchanges
  ADD CONSTRAINT currency_exchanges_rate_positive CHECK (rate > 0);
