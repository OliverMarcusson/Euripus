CREATE TABLE IF NOT EXISTS admin_quality_channel_settings (
  singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
  include_categories_without_country_prefix BOOLEAN NOT NULL DEFAULT FALSE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO admin_quality_channel_settings (singleton)
VALUES (TRUE)
ON CONFLICT (singleton) DO NOTHING;

ALTER TABLE admin_quality_channel_prefixes
  DROP CONSTRAINT IF EXISTS admin_quality_channel_prefixes_format;

ALTER TABLE admin_quality_channel_prefixes
  ADD CONSTRAINT admin_quality_channel_prefixes_format
  CHECK (prefix ~ '^[A-Z0-9]{2,3}\|$');
