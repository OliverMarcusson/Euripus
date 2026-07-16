CREATE TABLE on_demand_categories (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  profile_id UUID NOT NULL REFERENCES provider_profiles(id) ON DELETE CASCADE,
  media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'series')),
  remote_category_id TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX on_demand_categories_remote_idx
  ON on_demand_categories(user_id, profile_id, media_type, remote_category_id);

CREATE TABLE on_demand_titles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  profile_id UUID NOT NULL REFERENCES provider_profiles(id) ON DELETE CASCADE,
  category_id UUID NULL REFERENCES on_demand_categories(id) ON DELETE SET NULL,
  media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'series')),
  remote_id TEXT NOT NULL,
  name TEXT NOT NULL,
  poster_url TEXT NULL,
  backdrop_url TEXT NULL,
  plot TEXT NULL,
  genre TEXT NULL,
  cast_names TEXT NULL,
  director TEXT NULL,
  release_date TEXT NULL,
  rating DOUBLE PRECISION NULL,
  duration_minutes INTEGER NULL,
  container_extension TEXT NULL,
  provider_updated_at BIGINT NULL,
  episodes_fetched_at TIMESTAMPTZ NULL,
  details_fetched_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX on_demand_titles_remote_idx
  ON on_demand_titles(user_id, profile_id, media_type, remote_id);
CREATE INDEX on_demand_titles_browse_idx
  ON on_demand_titles(user_id, media_type, category_id, name);
CREATE INDEX on_demand_titles_name_trgm_idx
  ON on_demand_titles USING GIN (name gin_trgm_ops);

CREATE TABLE on_demand_episodes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  profile_id UUID NOT NULL REFERENCES provider_profiles(id) ON DELETE CASCADE,
  series_id UUID NOT NULL REFERENCES on_demand_titles(id) ON DELETE CASCADE,
  remote_id TEXT NOT NULL,
  season_number INTEGER NOT NULL,
  episode_number INTEGER NOT NULL,
  name TEXT NOT NULL,
  plot TEXT NULL,
  duration_minutes INTEGER NULL,
  poster_url TEXT NULL,
  container_extension TEXT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX on_demand_episodes_remote_idx
  ON on_demand_episodes(user_id, profile_id, remote_id);
CREATE INDEX on_demand_episodes_series_idx
  ON on_demand_episodes(series_id, season_number, episode_number);
