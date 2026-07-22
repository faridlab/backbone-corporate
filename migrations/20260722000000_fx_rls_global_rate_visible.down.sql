-- Reverse: restore the original single-predicate RLS policy on corporate.currency_exchanges.
-- (Global fallback rates become invisible to scoped connections again — the pre-fix behavior.)

DROP POLICY IF EXISTS currency_exchanges_company_isolation ON corporate.currency_exchanges;
CREATE POLICY currency_exchanges_company_isolation ON corporate.currency_exchanges
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid);
