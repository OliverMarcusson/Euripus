CREATE INDEX IF NOT EXISTS channels_user_profile_updated_idx
  ON channels(user_id, profile_id, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS channels_user_category_name_idx
  ON channels(user_id, category_id, name ASC);

CREATE INDEX IF NOT EXISTS programs_user_channel_time_idx
  ON programs(user_id, channel_id, start_at ASC, end_at ASC);
