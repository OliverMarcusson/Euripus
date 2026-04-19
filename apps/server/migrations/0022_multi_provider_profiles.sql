ALTER TABLE provider_profiles
  DROP CONSTRAINT IF EXISTS provider_profiles_user_id_key;

CREATE INDEX IF NOT EXISTS provider_profiles_user_id_idx
  ON provider_profiles (user_id);
