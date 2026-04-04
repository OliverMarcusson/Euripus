CREATE INDEX IF NOT EXISTS programs_user_profile_idx
  ON programs(user_id, profile_id);

CREATE INDEX IF NOT EXISTS programs_search_tsv_idx
  ON programs
  USING GIN (
    to_tsvector(
      'simple',
      coalesce(title, '') || ' ' || coalesce(channel_name, '') || ' ' || coalesce(description, '')
    )
  );

CREATE INDEX IF NOT EXISTS programs_search_trgm_idx
  ON programs
  USING GIN (
    (
      coalesce(title, '') || ' ' || coalesce(channel_name, '') || ' ' || coalesce(description, '')
    ) gin_trgm_ops
  );
