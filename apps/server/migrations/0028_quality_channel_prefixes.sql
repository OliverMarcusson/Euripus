CREATE TABLE IF NOT EXISTS admin_quality_channel_prefixes (
  prefix TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT admin_quality_channel_prefixes_format CHECK (prefix ~ '^[A-Z]{2,3}\|$')
);
