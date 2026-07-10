-- Down: drop corporate.currency_exchanges table
DROP TABLE IF EXISTS corporate.currency_exchanges CASCADE;
DROP FUNCTION IF EXISTS corporate.currency_exchanges_audit_timestamp() CASCADE;
