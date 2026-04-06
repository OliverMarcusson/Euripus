CREATE TABLE IF NOT EXISTS receiver_devices (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  owner_user_id UUID NULL REFERENCES users(id) ON DELETE CASCADE,
  device_key TEXT NOT NULL,
  device_name TEXT NOT NULL,
  platform TEXT NOT NULL,
  form_factor_hint TEXT NULL,
  app_kind TEXT NOT NULL,
  remembered BOOLEAN NOT NULL DEFAULT FALSE,
  receiver_credential_hash TEXT NULL,
  paired_at TIMESTAMPTZ NULL,
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  current_playback_title TEXT NULL,
  current_playback_kind TEXT NULL,
  current_playback_live BOOLEAN NULL,
  current_playback_catchup BOOLEAN NULL,
  current_playback_updated_at TIMESTAMPTZ NULL,
  current_playback_paused BOOLEAN NULL,
  current_playback_position_seconds DOUBLE PRECISION NULL,
  current_playback_duration_seconds DOUBLE PRECISION NULL,
  revoked_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS receiver_devices_device_key_idx
  ON receiver_devices(device_key);

CREATE INDEX IF NOT EXISTS receiver_devices_owner_seen_idx
  ON receiver_devices(owner_user_id, updated_at DESC)
  WHERE owner_user_id IS NOT NULL AND revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS receiver_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  receiver_device_id UUID NOT NULL REFERENCES receiver_devices(id) ON DELETE CASCADE,
  session_token_hash TEXT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  closed_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS receiver_sessions_token_hash_idx
  ON receiver_sessions(session_token_hash);

CREATE INDEX IF NOT EXISTS receiver_sessions_device_idx
  ON receiver_sessions(receiver_device_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS receiver_pairing_codes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  receiver_device_id UUID NOT NULL REFERENCES receiver_devices(id) ON DELETE CASCADE,
  code TEXT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  claimed_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS receiver_pairing_codes_active_code_idx
  ON receiver_pairing_codes(code)
  WHERE claimed_at IS NULL;

CREATE INDEX IF NOT EXISTS receiver_pairing_codes_device_idx
  ON receiver_pairing_codes(receiver_device_id, created_at DESC);

CREATE TABLE IF NOT EXISTS receiver_controller_sessions (
  controller_session_id UUID PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  receiver_device_id UUID NOT NULL REFERENCES receiver_devices(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS receiver_controller_sessions_user_idx
  ON receiver_controller_sessions(user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS receiver_commands (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  controller_session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  receiver_device_id UUID NOT NULL REFERENCES receiver_devices(id) ON DELETE CASCADE,
  command_type TEXT NOT NULL,
  source_title TEXT NOT NULL,
  status TEXT NOT NULL,
  payload JSONB NOT NULL,
  error_message TEXT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  delivered_at TIMESTAMPTZ NULL,
  acknowledged_at TIMESTAMPTZ NULL,
  failed_at TIMESTAMPTZ NULL
);

CREATE INDEX IF NOT EXISTS receiver_commands_device_idx
  ON receiver_commands(receiver_device_id, created_at DESC);

CREATE INDEX IF NOT EXISTS receiver_commands_controller_idx
  ON receiver_commands(controller_session_id, created_at DESC);
