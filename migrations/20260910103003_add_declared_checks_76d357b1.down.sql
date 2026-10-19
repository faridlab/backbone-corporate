-- Down: drop the declared rule constraints for corporate module
ALTER TABLE IF EXISTS corporate.territories DROP CONSTRAINT IF EXISTS ck_territories_name_max_length__max_length;
ALTER TABLE IF EXISTS corporate.territories DROP CONSTRAINT IF EXISTS ck_territories_name_required__required;
ALTER TABLE IF EXISTS corporate.territories DROP CONSTRAINT IF EXISTS ck_territories_code_max_length__max_length;
ALTER TABLE IF EXISTS corporate.territories DROP CONSTRAINT IF EXISTS ck_territories_code_required__required;
DO $$
BEGIN
    IF to_regclass('corporate.uq_territories_code__unique') IS NOT NULL AND to_regclass('corporate.idx_territories_code') IS NULL THEN
        ALTER INDEX corporate.uq_territories_code__unique RENAME TO idx_territories_code;
    END IF;
END $$;

ALTER TABLE IF EXISTS corporate.terms_and_conditions DROP CONSTRAINT IF EXISTS ck_terms_and_conditions_body_max_length__max_length;
ALTER TABLE IF EXISTS corporate.terms_and_conditions DROP CONSTRAINT IF EXISTS ck_terms_and_conditions_body_required__required;
ALTER TABLE IF EXISTS corporate.terms_and_conditions DROP CONSTRAINT IF EXISTS ck_terms_and_conditions_title_max_length__max_length;
ALTER TABLE IF EXISTS corporate.terms_and_conditions DROP CONSTRAINT IF EXISTS ck_terms_and_conditions_title_required__required;
ALTER TABLE IF EXISTS corporate.terms_and_conditions DROP CONSTRAINT IF EXISTS ck_terms_and_conditions_code_max_length__max_length;
ALTER TABLE IF EXISTS corporate.terms_and_conditions DROP CONSTRAINT IF EXISTS ck_terms_and_conditions_code_required__required;
DO $$
BEGIN
    IF to_regclass('corporate.uq_terms_and_conditions_code__unique') IS NOT NULL AND to_regclass('corporate.idx_terms_and_conditions_code') IS NULL THEN
        ALTER INDEX corporate.uq_terms_and_conditions_code__unique RENAME TO idx_terms_and_conditions_code;
    END IF;
END $$;

ALTER TABLE IF EXISTS corporate.incoterms DROP CONSTRAINT IF EXISTS ck_incoterms_name_max_length__max_length;
ALTER TABLE IF EXISTS corporate.incoterms DROP CONSTRAINT IF EXISTS ck_incoterms_name_required__required;
ALTER TABLE IF EXISTS corporate.incoterms DROP CONSTRAINT IF EXISTS ck_incoterms_code_max_length__max_length;
ALTER TABLE IF EXISTS corporate.incoterms DROP CONSTRAINT IF EXISTS ck_incoterms_code_required__required;
DO $$
BEGIN
    IF to_regclass('corporate.uq_incoterms_code__unique') IS NOT NULL AND to_regclass('corporate.idx_incoterms_code') IS NULL THEN
        ALTER INDEX corporate.uq_incoterms_code__unique RENAME TO idx_incoterms_code;
    END IF;
END $$;

ALTER TABLE IF EXISTS corporate.currency_exchanges DROP CONSTRAINT IF EXISTS ck_currency_exchanges_source_max_length__max_length;
ALTER TABLE IF EXISTS corporate.currency_exchanges DROP CONSTRAINT IF EXISTS ck_currency_exchanges_rate_non_negative__non_negative;
ALTER TABLE IF EXISTS corporate.currency_exchanges DROP CONSTRAINT IF EXISTS ck_currency_exchanges_to_currency_max_length__max_length;
ALTER TABLE IF EXISTS corporate.currency_exchanges DROP CONSTRAINT IF EXISTS ck_currency_exchanges_to_currency_required__required;
ALTER TABLE IF EXISTS corporate.currency_exchanges DROP CONSTRAINT IF EXISTS ck_currency_exchanges_from_currency_max_length__max_length;
ALTER TABLE IF EXISTS corporate.currency_exchanges DROP CONSTRAINT IF EXISTS ck_currency_exchanges_from_currency_required__required;
ALTER TABLE IF EXISTS corporate.currencies DROP CONSTRAINT IF EXISTS ck_currencies_symbol_max_length__max_length;
ALTER TABLE IF EXISTS corporate.currencies DROP CONSTRAINT IF EXISTS ck_currencies_name_max_length__max_length;
ALTER TABLE IF EXISTS corporate.currencies DROP CONSTRAINT IF EXISTS ck_currencies_name_required__required;
ALTER TABLE IF EXISTS corporate.currencies DROP CONSTRAINT IF EXISTS ck_currencies_iso_code_max_length__max_length;
ALTER TABLE IF EXISTS corporate.currencies DROP CONSTRAINT IF EXISTS ck_currencies_iso_code_required__required;
DO $$
BEGIN
    IF to_regclass('corporate.uq_currencies_iso_code__unique') IS NOT NULL AND to_regclass('corporate.idx_currencies_iso_code') IS NULL THEN
        ALTER INDEX corporate.uq_currencies_iso_code__unique RENAME TO idx_currencies_iso_code;
    END IF;
END $$;

