WITH ranked_active_sync_jobs AS (
  SELECT
    id,
    ROW_NUMBER() OVER (
      PARTITION BY profile_id
      ORDER BY created_at DESC, id DESC
    ) AS active_rank
  FROM sync_jobs
  WHERE status IN ('queued', 'running')
)
UPDATE sync_jobs
SET
  status = 'failed',
  finished_at = COALESCE(finished_at, NOW()),
  current_phase = 'failed',
  phase_message = 'Interrupted by active sync uniqueness migration',
  error_message = 'Interrupted by active sync uniqueness migration'
WHERE id IN (
  SELECT id
  FROM ranked_active_sync_jobs
  WHERE active_rank > 1
);

CREATE UNIQUE INDEX IF NOT EXISTS sync_jobs_one_active_per_profile_idx
  ON sync_jobs(profile_id)
  WHERE status IN ('queued', 'running');
