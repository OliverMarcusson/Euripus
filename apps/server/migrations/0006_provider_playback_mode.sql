ALTER TABLE provider_profiles
ADD COLUMN IF NOT EXISTS playback_mode TEXT NOT NULL DEFAULT 'direct';
