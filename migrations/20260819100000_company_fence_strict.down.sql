-- Revert the ADR-0014 strict fence re-statement for corporate module.
-- The split fence predates this migration (20260426220005 + 20260722000000), so the
-- honest reverse is to re-state the same live split policy, not to disarm the table:
-- a down that disabled RLS would leave exchange rates unfenced — a posture this module
-- never had. See the up-migration header for why the policy is split (global-rate reads,
-- own-only writes).

ALTER TABLE corporate.currency_exchanges FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS currency_exchanges_company_isolation ON corporate.currency_exchanges;
CREATE POLICY currency_exchanges_company_isolation ON corporate.currency_exchanges
    FOR ALL
    USING      (company_id IS NULL
                OR company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
