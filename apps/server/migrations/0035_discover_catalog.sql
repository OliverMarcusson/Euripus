-- Discover joins TMDB charts against the per-user on-demand catalog. Both sides run
-- through one normalizer so provider titles like "4K | EN - The Matrix" collapse onto
-- the same key as TMDB's "The Matrix". Defining it once as an IMMUTABLE function keeps
-- the two sides from drifting; a Rust normalizer for TMDB and a SQL one for the catalog
-- would silently diverge.
CREATE OR REPLACE FUNCTION discover_match_key(value TEXT)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
PARALLEL SAFE
AS $$
  SELECT regexp_replace(
    translate(
      -- ß has to expand to two characters, which translate() cannot do.
      replace(
        lower(
          -- Strip the quality and platform prefixes Xtream providers stack onto titles.
          -- Mirrors the prefix set in 0026_filter_on_demand_catalog.sql.
          regexp_replace(
            value,
            '^[[:space:]]*((4K|UHD|FHD|HD)[[:space:]_:|-]+)*((EN|SE|NF|AMZ|A\+|D\+|PRMT|VP|MRVL|DSC\+|SKY|MAX|P\+|PCOK|SHWT)[[:space:]_:|-]+)*',
            '',
            'i'
          )
        ),
        'ß',
        'ss'
      ),
      -- unaccent() is only STABLE, so it cannot appear in a generated column. This folding
      -- is not linguistically perfect (ö collapses to o, which suits Swedish but not
      -- German), but both sides of the match run through this same function, so agreement
      -- matters more than correctness here.
      'àáâãäåæèéêëìíîïñòóôõöøùúûüýÿçšžđł',
      'aaaaaaaeeeeiiiinoooooouuuuyycszdl'
    ),
    '[^a-z0-9]+',
    '',
    'g'
  );
$$;

-- STORED so it backfills existing rows immediately and stays correct on every sync
-- without a separate maintenance pass. Note that the column pins the function: changing
-- the normalizer later needs a migration that drops and re-adds this column.
ALTER TABLE on_demand_titles
  ADD COLUMN match_key TEXT GENERATED ALWAYS AS (discover_match_key(name)) STORED;

CREATE INDEX on_demand_titles_match_key_idx
  ON on_demand_titles(user_id, media_type, match_key);

-- TMDB metadata is global, not per user: one row serves every account.
CREATE TABLE tmdb_titles (
  media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'series')),
  tmdb_id BIGINT NOT NULL,
  title TEXT NOT NULL,
  original_title TEXT NOT NULL,
  original_language TEXT NULL,
  origin_countries TEXT[] NOT NULL DEFAULT '{}',
  overview TEXT NULL,
  poster_path TEXT NULL,
  backdrop_path TEXT NULL,
  release_date TEXT NULL,
  release_year INTEGER NULL,
  vote_average DOUBLE PRECISION NULL,
  vote_count INTEGER NULL,
  popularity DOUBLE PRECISION NULL,
  -- Normalized primary, original and alternative titles. Alternative titles cost one
  -- TMDB request per title, so they are backfilled in the background rather than during
  -- a chart refresh; until then this holds just the primary and original title.
  match_keys TEXT[] NOT NULL DEFAULT '{}',
  alternative_titles_fetched_at TIMESTAMPTZ NULL,
  refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (media_type, tmdb_id)
);

CREATE INDEX tmdb_titles_alternative_backfill_idx
  ON tmdb_titles(alternative_titles_fetched_at NULLS FIRST, refreshed_at);

-- TMDB has no per-country popularity, so a chart is identified by how the country was
-- applied as well as by which country: 'available_in' uses watch_region licensing,
-- 'from' uses country of origin. 'global' carries an empty country code because a
-- nullable column cannot participate in the primary key.
CREATE TABLE tmdb_chart_entries (
  chart TEXT NOT NULL CHECK (chart IN ('trending', 'popular', 'top_rated')),
  media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'series')),
  country_mode TEXT NOT NULL CHECK (country_mode IN ('global', 'available_in', 'from')),
  country_code TEXT NOT NULL,
  rank INTEGER NOT NULL CHECK (rank > 0),
  tmdb_id BIGINT NOT NULL,
  refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (chart, media_type, country_mode, country_code, rank),
  CONSTRAINT tmdb_chart_entries_country_scope
    CHECK ((country_mode = 'global') = (country_code = '')),
  CONSTRAINT tmdb_chart_entries_title_fk
    FOREIGN KEY (media_type, tmdb_id) REFERENCES tmdb_titles(media_type, tmdb_id) ON DELETE CASCADE
);

CREATE INDEX tmdb_chart_entries_title_idx
  ON tmdb_chart_entries(media_type, tmdb_id);
