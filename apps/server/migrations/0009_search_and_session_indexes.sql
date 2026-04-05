DROP INDEX IF EXISTS programs_search_tsv_idx;
DROP INDEX IF EXISTS programs_search_trgm_idx;

CREATE INDEX IF NOT EXISTS sessions_refresh_token_hash_idx
  ON sessions (refresh_token_hash);

CREATE INDEX IF NOT EXISTS provider_profiles_status_idx
  ON provider_profiles (status);
