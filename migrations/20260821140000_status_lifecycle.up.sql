-- Migration: replace the two corporate reference-master lifecycle booleans with status enums
-- incoterms and terms_and_conditions each carried `is_active BOOLEAN NOT NULL DEFAULT TRUE`;
-- the tree-wide convention is one `status` enum field per lifecycle (see
-- docs/refactoring-schema in the serpa workspace), matching the currencies flip this
-- module already carries. Each boolean migrates only rows deviating from its own column
-- default. The enum types are created unqualified so they land beside the module's other
-- enum types (public), where the generated sqlx type_name resolves.

DO $$ BEGIN
    CREATE TYPE incoterm_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN
    CREATE TYPE terms_and_conditions_status AS ENUM ('active', 'inactive');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

ALTER TABLE corporate.incoterms ADD COLUMN status incoterm_status NOT NULL DEFAULT 'active';
UPDATE corporate.incoterms SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE corporate.incoterms DROP COLUMN is_active;

ALTER TABLE corporate.terms_and_conditions ADD COLUMN status terms_and_conditions_status NOT NULL DEFAULT 'active';
UPDATE corporate.terms_and_conditions SET status = 'inactive' WHERE NOT is_active;
ALTER TABLE corporate.terms_and_conditions DROP COLUMN is_active;
