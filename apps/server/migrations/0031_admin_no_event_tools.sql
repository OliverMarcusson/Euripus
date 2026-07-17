ALTER TABLE users
  ADD COLUMN IF NOT EXISTS is_admin BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE channels
  ADD COLUMN IF NOT EXISTS hls_stream_origin TEXT NULL,
  ADD COLUMN IF NOT EXISTS hls_stream_path TEXT NULL;

CREATE INDEX IF NOT EXISTS channels_hls_stream_identity_idx
  ON channels(hls_stream_origin, hls_stream_path)
  WHERE hls_stream_path IS NOT NULL;

CREATE TABLE IF NOT EXISTS admin_no_event_hls_streams (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  hls_stream_origin TEXT NOT NULL,
  hls_stream_path TEXT NOT NULL,
  observed_channel_id UUID NULL REFERENCES channels(id) ON DELETE SET NULL,
  observed_channel_name TEXT NOT NULL,
  created_by UUID NULL REFERENCES users(id) ON DELETE SET NULL,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (hls_stream_origin, hls_stream_path)
);

CREATE TABLE IF NOT EXISTS admin_no_event_regex_rules (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  sample TEXT NOT NULL,
  pattern TEXT NOT NULL,
  explanation TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  created_by UUID NULL REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT admin_no_event_regex_rules_status
    CHECK (status IN ('pending', 'confirmed', 'rejected', 'disabled'))
);

CREATE INDEX IF NOT EXISTS admin_no_event_regex_rules_active_idx
  ON admin_no_event_regex_rules(status)
  WHERE status = 'confirmed';
