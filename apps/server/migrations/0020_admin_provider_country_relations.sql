CREATE TABLE IF NOT EXISTS admin_search_provider_countries (
  group_id UUID NOT NULL REFERENCES admin_search_pattern_groups(id) ON DELETE CASCADE,
  country_code TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (group_id, country_code)
);

CREATE INDEX IF NOT EXISTS admin_search_provider_countries_country_idx
ON admin_search_provider_countries(country_code);
