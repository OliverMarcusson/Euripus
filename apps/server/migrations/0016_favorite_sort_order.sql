ALTER TABLE favorites
  ADD COLUMN IF NOT EXISTS sort_order INTEGER;

WITH ranked_favorites AS (
  SELECT
    user_id,
    channel_id,
    ROW_NUMBER() OVER (
      PARTITION BY user_id
      ORDER BY created_at DESC, channel_id ASC
    ) - 1 AS next_sort_order
  FROM favorites
)
UPDATE favorites f
SET sort_order = ranked_favorites.next_sort_order
FROM ranked_favorites
WHERE f.user_id = ranked_favorites.user_id
  AND f.channel_id = ranked_favorites.channel_id
  AND f.sort_order IS NULL;

ALTER TABLE favorites
  ALTER COLUMN sort_order SET NOT NULL;

CREATE INDEX IF NOT EXISTS favorites_user_sort_idx
  ON favorites(user_id, sort_order, created_at DESC);

ALTER TABLE favorite_channel_categories
  ADD COLUMN IF NOT EXISTS sort_order INTEGER;

WITH ranked_categories AS (
  SELECT
    user_id,
    category_id,
    ROW_NUMBER() OVER (
      PARTITION BY user_id
      ORDER BY created_at DESC, category_id ASC
    ) - 1 AS next_sort_order
  FROM favorite_channel_categories
)
UPDATE favorite_channel_categories fcc
SET sort_order = ranked_categories.next_sort_order
FROM ranked_categories
WHERE fcc.user_id = ranked_categories.user_id
  AND fcc.category_id = ranked_categories.category_id
  AND fcc.sort_order IS NULL;

ALTER TABLE favorite_channel_categories
  ALTER COLUMN sort_order SET NOT NULL;

CREATE INDEX IF NOT EXISTS favorite_channel_categories_user_sort_idx
  ON favorite_channel_categories(user_id, sort_order, created_at DESC);
