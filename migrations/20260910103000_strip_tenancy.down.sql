-- Hand-authored (user-owned). Not regenerated.
--
-- Best-effort restore sketch for the tenancy strip (ADR-0029). This is a breaking module
-- release against dev-stage databases: the down re-adds the company_id column as nullable
-- with its plain index, rebuilds the old company-coalesced no-overlap EXCLUDE, and restores
-- the split read/write policy shape (reads admit global NULL-company fallback rates, writes
-- stay own-only) — but restores NO data. Rows written after the strip (or after the
-- decorator re-keyed them) carry org_unit_id only. The composing service's tenancy decorator
-- remains the live fence; treat this down as a schema-shape sketch for archaeology, not a
-- usable rollback.

ALTER TABLE corporate.currency_exchanges ADD COLUMN IF NOT EXISTS company_id uuid;

CREATE INDEX IF NOT EXISTS idx_currency_exchanges_company_id
  ON corporate.currency_exchanges (company_id);

ALTER TABLE corporate.currency_exchanges
  DROP CONSTRAINT IF EXISTS currency_exchanges_no_overlap;
ALTER TABLE corporate.currency_exchanges
  ADD CONSTRAINT currency_exchanges_no_overlap
  EXCLUDE USING gist (
    COALESCE(company_id, '00000000-0000-0000-0000-000000000000'::uuid) WITH =,
    from_currency WITH =,
    to_currency WITH =,
    daterange(effective_from, effective_to, '[]') WITH &&
  )
  WHERE ((metadata->>'deleted_at') IS NULL);

ALTER TABLE corporate.currency_exchanges ENABLE ROW LEVEL SECURITY;
ALTER TABLE corporate.currency_exchanges FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS currency_exchanges_company_isolation ON corporate.currency_exchanges;
CREATE POLICY currency_exchanges_company_isolation ON corporate.currency_exchanges
    FOR ALL
    USING      (company_id IS NULL
                OR company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
