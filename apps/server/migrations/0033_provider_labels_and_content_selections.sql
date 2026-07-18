ALTER TABLE provider_profiles
  ADD COLUMN IF NOT EXISTS label TEXT NULL;

ALTER TABLE users
  ADD COLUMN IF NOT EXISTS live_provider_id UUID NULL,
  ADD COLUMN IF NOT EXISTS on_demand_provider_id UUID NULL;

UPDATE users
SET live_provider_id = COALESCE(live_provider_id, active_provider_id),
    on_demand_provider_id = COALESCE(on_demand_provider_id, active_provider_id);

ALTER TABLE users
  DROP CONSTRAINT IF EXISTS users_live_provider_id_fkey,
  DROP CONSTRAINT IF EXISTS users_on_demand_provider_id_fkey;

ALTER TABLE users
  ADD CONSTRAINT users_live_provider_id_fkey
    FOREIGN KEY (live_provider_id) REFERENCES provider_profiles(id) ON DELETE SET NULL,
  ADD CONSTRAINT users_on_demand_provider_id_fkey
    FOREIGN KEY (on_demand_provider_id) REFERENCES provider_profiles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS users_live_provider_id_idx
  ON users(live_provider_id);

CREATE INDEX IF NOT EXISTS users_on_demand_provider_id_idx
  ON users(on_demand_provider_id);
