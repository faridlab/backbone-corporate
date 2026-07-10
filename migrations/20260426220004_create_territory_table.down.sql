-- Down: drop corporate.territories table
DROP TABLE IF EXISTS corporate.territories CASCADE;
DROP FUNCTION IF EXISTS corporate.territories_audit_timestamp() CASCADE;
