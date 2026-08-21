-- Down: restore the two is_active booleans exactly as they were.
-- Only 'inactive' rows are written back as FALSE; rows at the column default
-- map to the boolean default TRUE without an UPDATE.

ALTER TABLE corporate.incoterms ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE corporate.incoterms SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE corporate.incoterms DROP COLUMN status;

ALTER TABLE corporate.terms_and_conditions ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE corporate.terms_and_conditions SET is_active = FALSE WHERE status = 'inactive';
ALTER TABLE corporate.terms_and_conditions DROP COLUMN status;

DROP TYPE IF EXISTS incoterm_status;
DROP TYPE IF EXISTS terms_and_conditions_status;
