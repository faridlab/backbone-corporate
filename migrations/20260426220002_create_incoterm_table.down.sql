-- Down: drop corporate.incoterms table
DROP TABLE IF EXISTS corporate.incoterms CASCADE;
DROP FUNCTION IF EXISTS corporate.incoterms_audit_timestamp() CASCADE;
