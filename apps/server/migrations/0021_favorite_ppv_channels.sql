CREATE TABLE IF NOT EXISTS favorite_ppv_channels (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  channel_id UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  sort_order INTEGER NOT NULL,
  PRIMARY KEY (user_id, channel_id)
);

CREATE INDEX IF NOT EXISTS favorite_ppv_channels_user_sort_idx
  ON favorite_ppv_channels(user_id, sort_order, created_at DESC);
