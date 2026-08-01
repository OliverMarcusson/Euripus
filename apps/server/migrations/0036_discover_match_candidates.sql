-- The single canonical key from 0035 was wrong in both directions.
--
-- Too strict: it only stripped provider tags in a fixed quality-then-platform order and
-- ignored the season and year suffixes providers append, so "EN | The Rookie S05",
-- "EN | House of the Dragon (2022)" and "EN | Rick & Morty" all failed to match TMDB.
--
-- Too loose: tags were stripped when followed by plain whitespace, so "Sky High" reduced
-- to "high" and "Max Payne" to "payne", which match unrelated titles.
--
-- One key cannot satisfy both. Each title now expands to a small set of candidate keys and
-- a match is any overlap, so the raw form always survives while the stripped forms add
-- reach.

CREATE EXTENSION IF NOT EXISTS btree_gin;

-- Case, accent and punctuation folding. "&" becomes "and" so that a provider's
-- "Rick & Morty" agrees with TMDB's "Rick and Morty".
CREATE OR REPLACE FUNCTION discover_fold_title(value TEXT)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
PARALLEL SAFE
AS $$
  SELECT regexp_replace(
    translate(
      -- ß has to expand to two characters, which translate() cannot do.
      replace(replace(lower(value), 'ß', 'ss'), '&', ' and '),
      -- unaccent() is only STABLE, so it cannot appear in a generated column. The folding
      -- is not linguistically perfect (ö collapses to o, which suits Swedish but not
      -- German), but both sides run through this same function, so agreement matters more.
      'àáâãäåæèéêëìíîïñòóôõöøùúûüýÿçšžđł',
      'aaaaaaaeeeeiiiinoooooouuuuyycszdl'
    ),
    '[^a-z0-9]+',
    '',
    'g'
  );
$$;

-- Strips leading quality and platform tags in any order and any number.
--
-- A delimiter is required after the tag. Allowing plain whitespace is what made 0035 eat
-- legitimate first words, since SKY, MAX, VP and SE are all also ordinary title words.
CREATE OR REPLACE FUNCTION discover_strip_title_tags(value TEXT)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
PARALLEL SAFE
AS $$
  SELECT regexp_replace(
    value,
    '^([[:space:]]*(4K|UHD|FHD|HD|EN|SE|NF|AMZ|A\+|D\+|PRMT|VP|MRVL|DSC\+|SKY|MAX|P\+|PCOK|SHWT)[[:space:]]*[|:_-]+)+[[:space:]]*',
    '',
    'i'
  );
$$;

-- Strips trailing season and year markers. Providers split a series across several catalog
-- entries ("... S01", "... Season 2") that all correspond to one TMDB title.
CREATE OR REPLACE FUNCTION discover_strip_title_suffix(value TEXT)
RETURNS TEXT
LANGUAGE SQL
IMMUTABLE
PARALLEL SAFE
AS $$
  SELECT regexp_replace(
    value,
    '([[:space:]]*[-–|:]?[[:space:]]*(\((19|20)[0-9]{2}\)|\[(19|20)[0-9]{2}\]|S[0-9]{1,2}|SEASON[[:space:]]*[0-9]{1,2}))+[[:space:]]*$',
    '',
    'i'
  );
$$;

-- The candidate set for one name. The unstripped form is always included, so aggressive
-- stripping can only ever add reach, never remove a correct match.
CREATE OR REPLACE FUNCTION discover_match_keys(value TEXT)
RETURNS TEXT[]
LANGUAGE SQL
IMMUTABLE
PARALLEL SAFE
AS $$
  SELECT ARRAY(
    SELECT DISTINCT k FROM unnest(ARRAY[
      discover_fold_title(value),
      discover_fold_title(discover_strip_title_tags(value)),
      discover_fold_title(discover_strip_title_suffix(value)),
      discover_fold_title(discover_strip_title_suffix(discover_strip_title_tags(value)))
    ]) AS k
    WHERE k <> ''
  );
$$;

-- Combines every name TMDB knows a title by into one candidate set.
CREATE OR REPLACE FUNCTION discover_title_match_keys(
  title TEXT,
  original_title TEXT,
  alternative_titles TEXT[]
)
RETURNS TEXT[]
LANGUAGE SQL
IMMUTABLE
PARALLEL SAFE
AS $$
  SELECT ARRAY(
    SELECT DISTINCT k
    FROM unnest(
      discover_match_keys(title)
      || discover_match_keys(original_title)
      || COALESCE((
        SELECT array_agg(alternative_key)
        FROM unnest(alternative_titles) AS alternative_title,
             unnest(discover_match_keys(alternative_title)) AS alternative_key
      ), '{}')
    ) AS k
    WHERE k <> ''
  );
$$;

DROP INDEX on_demand_titles_match_key_idx;
ALTER TABLE on_demand_titles DROP COLUMN match_key;
DROP FUNCTION discover_match_key(TEXT);

ALTER TABLE on_demand_titles
  ADD COLUMN match_keys TEXT[] GENERATED ALWAYS AS (discover_match_keys(name)) STORED;

-- btree_gin lets the array overlap and the user and media_type equality checks share one
-- index, which is the exact shape of the Discover availability join.
CREATE INDEX on_demand_titles_match_keys_idx
  ON on_demand_titles USING GIN (user_id, media_type, match_keys);

-- 0035 stored only normalized alternative-title keys, so changing the normalizer could not
-- recompute them without re-querying TMDB. Keeping the raw names makes any future change a
-- local UPDATE.
ALTER TABLE tmdb_titles ADD COLUMN alternative_titles TEXT[] NOT NULL DEFAULT '{}';

UPDATE tmdb_titles
SET match_keys = discover_title_match_keys(title, original_title, alternative_titles);

-- Existing rows have no raw alternative titles to rebuild from, so re-run the backfill.
UPDATE tmdb_titles SET alternative_titles_fetched_at = NULL;
