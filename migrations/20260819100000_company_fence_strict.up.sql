-- Company fence posture for corporate module (ADR-0014: strict)
-- The only company-scoped entity here is currency_exchanges; the rest of the module
-- (currencies, incoterms, territories, terms_and_conditions, companies) are global
-- reference masters with no company_id — implicitly outside the declaration, keeping
-- their existing unfenced posture.
--
-- currency_exchanges carries a deliberately SPLIT policy rather than the plain strict
-- predicate the declaration would naively derive. Context: the original fence
-- (20260426220005) used one predicate for both USING and WITH CHECK, which admitted ONLY
-- the caller's own rows — global (NULL-company) fallback rates were invisible to a scoped
-- connection, so `FxService::convert` returned NoRate even when a usable global rate
-- existed, and every multi-currency consumer (billing/payment/accounting) broke under the
-- non-super app role. The fix (20260722000000) split the policy: the USING (read) clause
-- admits global rows to every tenant, while the WITH CHECK (write) clause keeps writes
-- own-only — a global rate can still be created, but only via the admin/bypass path
-- (migrations, seeders, or a platform caller on a role that bypasses RLS), never by a
-- tenant forging `company_id = NULL` on a scoped connection. FORCE ROW LEVEL SECURITY
-- stays on so the table owner does not silently bypass the WITH CHECK either. Coarse
-- grain (a tenant reads ALL global rates, not a curated subset) is intentional: a
-- reference FX rate is a shared master, the same visibility shape as
-- `corporate.currencies`.
--
-- This migration declares the module posture (ADR-0014) and re-states that live split
-- policy verbatim — an idempotent refresh, not a semantic change. A future
-- `metaphor schema generate` re-deriving a plain-strict stanza here would be a REGRESSION:
-- if the generator ever emits for this table, restore the split shape.
-- Requires the app to connect as a non-superuser role; migrations/seeders run as
-- the owner and bypass.

ALTER TABLE corporate.currency_exchanges ENABLE ROW LEVEL SECURITY;
ALTER TABLE corporate.currency_exchanges FORCE  ROW LEVEL SECURITY;
DROP POLICY IF EXISTS currency_exchanges_company_isolation ON corporate.currency_exchanges;
CREATE POLICY currency_exchanges_company_isolation ON corporate.currency_exchanges
    FOR ALL
    USING      (company_id IS NULL
                OR company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
