CREATE TABLE IF NOT EXISTS playback_devices (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  device_key TEXT NOT NULL,
  device_name TEXT NOT NULL,
  platform TEXT NOT NULL,
  form_factor_hint TEXT NULL,
  remote_target_enabled BOOLEAN NOT NULL DEFAULT FALSE,
  current_playback_title TEXT NULL,
  current_playback_kind TEXT NULL,
  current_playback_live BOOLEAN NULL,
  current_playback_catchup BOOLEAN NULL,
  current_playback_updated_at TIMESTAMPTZ NULL,
  current_controller_session_id UUID NULL REFERENCES sessions(id) ON DELETE SET NULL,
  last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS playback_devices_user_key_idx
  ON playback_devices(user_id, device_key);

CREATE INDEX IF NOT EXISTS playback_devices_user_seen_idx
  ON playback_devices(user_id, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS playback_devices_session_idx
  ON playback_devices(session_id);

CREATE TABLE IF NOT EXISTS remote_playback_sessions (
  controller_session_id UUID PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  target_device_id UUID NOT NULL REFERENCES playback_devices(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS remote_playback_sessions_user_idx
  ON remote_playback_sessions(user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS remote_playback_sessions_target_idx
  ON remote_playback_sessions(target_device_id);

CREATE TABLE IF NOT EXISTS remote_playback_commands (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  controller_session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  target_device_id UUID NOT NULL REFERENCES playback_devices(id) ON DELETE CASCADE,
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

CREATE INDEX IF NOT EXISTS remote_playback_commands_target_idx
  ON remote_playback_commands(target_device_id, created_at DESC);

CREATE INDEX IF NOT EXISTS remote_playback_commands_controller_idx
  ON remote_playback_commands(controller_session_id, created_at DESC);
