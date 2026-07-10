-- Down: drop corporate.terms_and_conditions table
DROP TABLE IF EXISTS corporate.terms_and_conditions CASCADE;
DROP FUNCTION IF EXISTS corporate.terms_and_conditions_audit_timestamp() CASCADE;
