ALTER TABLE receiver_commands
  ADD COLUMN IF NOT EXISTS executing_at TIMESTAMPTZ NULL,
  ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ NULL;

ALTER TABLE receiver_devices
  ADD COLUMN IF NOT EXISTS current_playback_buffering BOOLEAN NULL,
  ADD COLUMN IF NOT EXISTS current_playback_error_message TEXT NULL;
