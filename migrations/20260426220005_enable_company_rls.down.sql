-- Down: remove the company RLS fence for corporate module

-- Reverse the company RLS fence for corporate.currency_exchanges
DROP POLICY IF EXISTS currency_exchanges_company_isolation ON corporate.currency_exchanges;
ALTER TABLE corporate.currency_exchanges NO FORCE ROW LEVEL SECURITY;
ALTER TABLE corporate.currency_exchanges DISABLE ROW LEVEL SECURITY;

