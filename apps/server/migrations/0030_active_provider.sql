ALTER TABLE users
  ADD COLUMN IF NOT EXISTS active_provider_id UUID NULL;

UPDATE users u
SET active_provider_id = (
  SELECT p.id
  FROM provider_profiles p
  WHERE p.user_id = u.id
  ORDER BY p.created_at ASC, p.id ASC
  LIMIT 1
)
WHERE u.active_provider_id IS NULL;

ALTER TABLE users
  DROP CONSTRAINT IF EXISTS users_active_provider_id_fkey;

ALTER TABLE users
  ADD CONSTRAINT users_active_provider_id_fkey
  FOREIGN KEY (active_provider_id) REFERENCES provider_profiles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS users_active_provider_id_idx
  ON users(active_provider_id);
