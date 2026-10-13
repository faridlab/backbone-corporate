-- Down: drop the declared foreign keys for corporate module
ALTER TABLE IF EXISTS corporate.territories DROP CONSTRAINT IF EXISTS fk_territories_parent_id;
