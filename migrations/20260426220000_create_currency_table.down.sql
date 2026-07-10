-- Down: drop corporate.currencies table
DROP TABLE IF EXISTS corporate.currencies CASCADE;
DROP FUNCTION IF EXISTS corporate.currencies_audit_timestamp() CASCADE;
