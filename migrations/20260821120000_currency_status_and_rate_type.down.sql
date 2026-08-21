-- Down: restore the is_active flag and drop rate provenance.

ALTER TABLE corporate.currency_exchanges DROP COLUMN IF EXISTS source;
ALTER TABLE corporate.currency_exchanges DROP COLUMN IF EXISTS rate_type;

DROP INDEX IF EXISTS idx_currencies_status;

ALTER TABLE corporate.currencies ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT TRUE;
UPDATE corporate.currencies SET is_active = (status = 'active');
ALTER TABLE corporate.currencies DROP COLUMN status;

CREATE INDEX IF NOT EXISTS idx_currencies_is_active ON corporate.currencies (is_active);
