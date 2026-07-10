-- The FX maturity backstop: a directed currency pair (within one company scope) must have NON-OVERLAPPING
-- effective windows, or `convert` for a historical date would match two rows and pick one
-- nondeterministically — the same past transaction re-translating to a different number. `upsert_rate`
-- rejects an overlap in the app; this EXCLUDE constraint is the last line of defence against any other
-- writer (a raw insert, the generic CRUD handler, a bad migration).
--
-- company_id is coalesced to a zero sentinel so two GLOBAL (null-company) rates for the same pair also
-- conflict (a NULL would otherwise never collide under EXCLUDE). daterange is inclusive-inclusive '[]';
-- a null effective_to means unbounded (the current rate).
CREATE EXTENSION IF NOT EXISTS btree_gist;

ALTER TABLE corporate.currency_exchanges
  ADD CONSTRAINT currency_exchanges_no_overlap
  EXCLUDE USING gist (
    COALESCE(company_id, '00000000-0000-0000-0000-000000000000'::uuid) WITH =,
    from_currency WITH =,
    to_currency WITH =,
    daterange(effective_from, effective_to, '[]') WITH &&
  )
  WHERE ((metadata->>'deleted_at') IS NULL);
