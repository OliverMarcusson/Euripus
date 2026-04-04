CREATE TABLE IF NOT EXISTS epg_sources (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  profile_id UUID NOT NULL REFERENCES provider_profiles(id) ON DELETE CASCADE,
  url TEXT NOT NULL,
  priority INTEGER NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  source_kind TEXT NOT NULL DEFAULT 'external',
  last_sync_at TIMESTAMPTZ NULL,
  last_sync_error TEXT NULL,
  last_program_count INTEGER NULL,
  last_matched_count INTEGER NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS epg_sources_profile_priority_idx
  ON epg_sources(profile_id, priority, created_at);
