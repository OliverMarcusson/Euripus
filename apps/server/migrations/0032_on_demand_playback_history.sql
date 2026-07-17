CREATE TABLE on_demand_playback_history (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title_id UUID NOT NULL REFERENCES on_demand_titles(id) ON DELETE CASCADE,
  episode_id UUID NULL REFERENCES on_demand_episodes(id) ON DELETE CASCADE,
  position_seconds DOUBLE PRECISION NOT NULL DEFAULT 0 CHECK (position_seconds >= 0),
  duration_seconds DOUBLE PRECISION NULL CHECK (duration_seconds IS NULL OR duration_seconds >= 0),
  last_played_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (user_id, title_id)
);

CREATE INDEX on_demand_playback_history_recent_idx
  ON on_demand_playback_history(user_id, last_played_at DESC);
