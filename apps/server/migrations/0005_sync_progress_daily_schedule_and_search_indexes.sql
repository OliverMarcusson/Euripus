ALTER TABLE provider_profiles
  ADD COLUMN IF NOT EXISTS last_scheduled_sync_on DATE NULL;

ALTER TABLE sync_jobs
  ADD COLUMN IF NOT EXISTS trigger TEXT NOT NULL DEFAULT 'manual',
  ADD COLUMN IF NOT EXISTS current_phase TEXT NULL,
  ADD COLUMN IF NOT EXISTS completed_phases INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS total_phases INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS phase_message TEXT NULL;

CREATE INDEX IF NOT EXISTS sync_jobs_profile_status_idx
  ON sync_jobs(profile_id, status);

CREATE INDEX IF NOT EXISTS search_documents_user_type_starts_idx
  ON search_documents(user_id, entity_type, starts_at NULLS LAST);

CREATE INDEX IF NOT EXISTS search_documents_user_type_title_idx
  ON search_documents(user_id, entity_type, lower(title));
