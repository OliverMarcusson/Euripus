ALTER TABLE channels
ADD COLUMN IF NOT EXISTS search_country_code TEXT NULL,
ADD COLUMN IF NOT EXISTS search_provider_name TEXT NULL,
ADD COLUMN IF NOT EXISTS search_is_ppv BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS search_is_vip BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE programs
ADD COLUMN IF NOT EXISTS search_country_code TEXT NULL,
ADD COLUMN IF NOT EXISTS search_provider_name TEXT NULL,
ADD COLUMN IF NOT EXISTS search_is_ppv BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS search_is_vip BOOLEAN NOT NULL DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS channels_search_country_idx
ON channels(user_id, search_country_code);

CREATE INDEX IF NOT EXISTS channels_search_provider_idx
ON channels(user_id, search_provider_name);

CREATE INDEX IF NOT EXISTS channels_search_flags_idx
ON channels(user_id, search_is_ppv, search_is_vip);

CREATE INDEX IF NOT EXISTS programs_search_country_idx
ON programs(user_id, search_country_code);

CREATE INDEX IF NOT EXISTS programs_search_provider_idx
ON programs(user_id, search_provider_name);

CREATE INDEX IF NOT EXISTS programs_search_flags_idx
ON programs(user_id, search_is_ppv, search_is_vip);

CREATE TABLE IF NOT EXISTS admin_search_pattern_groups (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  kind TEXT NOT NULL,
  value TEXT NOT NULL,
  normalized_value TEXT NOT NULL,
  match_target TEXT NOT NULL,
  match_mode TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS admin_search_pattern_groups_kind_idx
ON admin_search_pattern_groups(kind, enabled, priority DESC);

CREATE TABLE IF NOT EXISTS admin_search_patterns (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  group_id UUID NOT NULL REFERENCES admin_search_pattern_groups(id) ON DELETE CASCADE,
  pattern TEXT NOT NULL,
  normalized_pattern TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS admin_search_patterns_group_idx
ON admin_search_patterns(group_id);
