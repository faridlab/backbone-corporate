-- Hand-authored (user-owned). Not regenerated.
--
-- Strip every company-fence artifact from the FX rate table (ADR-0029): the module is
-- tenant-agnostic; org scoping is installed by the COMPOSING service's tenancy decorator,
-- never by the module. Every corporate entity is global reference data; currency_exchanges
-- loses its former company-scoped rate lane (a per-company rate overriding the global one)
-- together with the company_id column itself. Dropped here: the company-leading index, the
-- currency_exchanges_company_isolation RLS policy, and the column. RLS enable/force flags
-- are deliberately NOT touched: the decorator owns those now.
--
-- Ordering guard (the decorator must run FIRST on any database with data): the module
-- never moves tenancy data. The table is safe to strip when EITHER
--   a) it carries org_unit_id with no NULLs — the decorator backfilled it from company_id —
--      or b) it is empty (a fresh database: the earlier chain files created it empty).
-- Otherwise the strip RAISEs, naming the decorator step, rather than dropping a column
-- that still holds the only tenancy key. The file is re-runnable (every drop is IF EXISTS
-- and the tracker has no checksums), so a failed run retries cleanly after the decorator
-- lands.
--
-- The no-overlap EXCLUDE backstop is REBUILT here, before the column drop: its index
-- spans COALESCE(company_id, sentinel), so the column cannot be dropped while the old
-- constraint stands. Post-strip the maturity invariant is table-wide — one unambiguous
-- rate window per directed pair.
--
-- Data conflict guard: two windows that only coexisted because they sat in different
-- former company scopes (a negotiated company rate plus the global fallback) become a
-- genuine ambiguity once scopes vanish — one unambiguous table-wide rate per pair means
-- all but one must go. The strip RAISEs naming a conflicting pair rather than silently
-- picking a winner (which window survives changes every conversion result); soft-delete
-- the losing windows and re-run.

CREATE EXTENSION IF NOT EXISTS btree_gist;

DO $$
DECLARE
    has_org boolean;
    org_nulls bigint;
    total bigint;
    offender record;
BEGIN
    IF to_regclass('corporate.currency_exchanges') IS NULL THEN
        RETURN; -- chain not fully applied on this database; nothing to strip
    END IF;

    EXECUTE 'SELECT count(*) FROM corporate.currency_exchanges' INTO total;

    SELECT EXISTS (
               SELECT 1 FROM information_schema.columns
               WHERE table_schema = 'corporate'
                 AND table_name = 'currency_exchanges'
                 AND column_name = 'org_unit_id'
           )
    INTO has_org;

    IF has_org THEN
        EXECUTE 'SELECT count(*) FROM corporate.currency_exchanges WHERE org_unit_id IS NULL'
        INTO org_nulls;
    ELSE
        org_nulls := total; -- no org column: every row's only tenancy key is company_id
    END IF;

    IF NOT (total = 0 OR (has_org AND org_nulls = 0)) THEN
        RAISE EXCEPTION 'refusing to strip company_id — corporate.currency_exchanges is not yet covered by the tenancy decorator (% rows, % rows not covered by org_unit_id). Apply the composing service''s tenancy decorator (it backfills org_unit_id from company_id) and re-run; it is the only step that moves tenancy data.', total, org_nulls;
    END IF;

    SELECT a.from_currency, a.to_currency
      INTO offender
      FROM corporate.currency_exchanges a
      JOIN corporate.currency_exchanges b
        ON  a.from_currency = b.from_currency
        AND a.to_currency   = b.to_currency
        AND a.id            < b.id
        AND a.effective_from <= COALESCE(b.effective_to, DATE '9999-12-31')
        AND b.effective_from <= COALESCE(a.effective_to, DATE '9999-12-31')
     WHERE (a.metadata->>'deleted_at') IS NULL
       AND (b.metadata->>'deleted_at') IS NULL
     LIMIT 1;

    IF offender.from_currency IS NOT NULL THEN
        RAISE EXCEPTION 'refusing to strip company_id — currency pair %->% still carries overlapping windows from different former company scopes; soft-delete all but one window for the pair and re-run (the rebuilt table-wide no-overlap rule admits a single unambiguous rate)', offender.from_currency, offender.to_currency;
    END IF;
END $$;

-- ── currency_exchanges ────────────────────────────────────────────────────────
-- Rebuild the FX maturity backstop WITHOUT the company element. After the strip the
-- invariant is table-wide: for one directed pair the effective windows must not overlap,
-- or a historical conversion would match two rows and pick one nondeterministically.
ALTER TABLE corporate.currency_exchanges
  DROP CONSTRAINT IF EXISTS currency_exchanges_no_overlap;
ALTER TABLE corporate.currency_exchanges
  ADD CONSTRAINT currency_exchanges_no_overlap
  EXCLUDE USING gist (
    from_currency WITH =,
    to_currency WITH =,
    daterange(effective_from, effective_to, '[]') WITH &&
  )
  WHERE ((metadata->>'deleted_at') IS NULL);

DROP INDEX IF EXISTS corporate.idx_currency_exchanges_company_id;
DROP POLICY IF EXISTS currency_exchanges_company_isolation ON corporate.currency_exchanges;
ALTER TABLE corporate.currency_exchanges DROP COLUMN IF EXISTS company_id;
