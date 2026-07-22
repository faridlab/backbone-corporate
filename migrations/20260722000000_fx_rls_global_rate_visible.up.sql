-- RLS policy refresh for corporate.currency_exchanges (ADR-0008 follow-up).
--
-- Context: the original fence (20260426220005) used a single predicate for both USING and WITH CHECK:
--     company_id = NULLIF(current_setting('app.company_id', true), '')::uuid
-- That admitted ONLY the caller's own rows — global (NULL-company) fallback rates were invisible to a
-- scoped connection, so `FxService::convert` returned NoRate even when a usable global rate existed, and
-- every multi-currency consumer (billing/payment/accounting) broke under the non-super app role.
--
-- Fix: split the policy so the USING (read) clause admits global rows to every tenant, while the
-- WITH CHECK (write) clause keeps writes own-only. A global rate can still be created — but only via the
-- admin/bypass path (migrations, seeders, or a platform caller on a role that bypasses RLS), never by a
-- tenant forging `company_id = NULL` on a scoped connection. FORCE ROW LEVEL SECURITY stays on so the
-- table owner does not silently bypass the WITH CHECK either.
--
-- Coarse grain (a tenant reads ALL global rates, not a curated subset) is intentional: a reference FX rate
-- is a shared master, the same visibility shape as `corporate.currencies`. A per-tenant global-rate grant
-- is a non-goal here; if that ever lands, it belongs in a separate policy table, not by narrowing this one.

ALTER TABLE corporate.currency_exchanges FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS currency_exchanges_company_isolation ON corporate.currency_exchanges;
CREATE POLICY currency_exchanges_company_isolation ON corporate.currency_exchanges
    FOR ALL
    USING      (company_id IS NULL
                OR company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
