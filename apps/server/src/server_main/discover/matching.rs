use super::*;

/// A chart row joined against whatever the user's providers actually carry.
///
/// `on_demand_title_id` is `NULL` when nothing matched: the client renders those greyed
/// out rather than dropping them, so chart positions stay honest.
#[derive(Debug, FromRow, Clone)]
pub(super) struct DiscoverTitleRow {
    pub(super) rank: i32,
    pub(super) tmdb_id: i64,
    pub(super) media_type: String,
    pub(super) title: String,
    pub(super) original_title: String,
    pub(super) origin_countries: Vec<String>,
    pub(super) overview: Option<String>,
    pub(super) poster_path: Option<String>,
    pub(super) backdrop_path: Option<String>,
    pub(super) release_date: Option<String>,
    pub(super) release_year: Option<i32>,
    pub(super) vote_average: Option<f64>,
    pub(super) vote_count: Option<i32>,
    pub(super) on_demand_title_id: Option<Uuid>,
    pub(super) provider_label: Option<String>,
}

/// Resolves the chart against a user's catalog.
///
/// The lateral join prefers a release-year match before falling back to any title sharing
/// a match key, because provider catalogs are full of remakes that normalize identically
/// ("The Thing", "Dune"). Without the year preference a 2021 chart entry would happily
/// bind to a 1984 rip.
pub(super) async fn load_chart_titles(
    pool: &PgPool,
    user_id: Uuid,
    chart: &str,
    media_type: &str,
    country_mode: &str,
    country_code: &str,
    offset: i64,
    limit: i64,
) -> Result<Vec<DiscoverTitleRow>, AppError> {
    let rows = sqlx::query_as::<_, DiscoverTitleRow>(
        r#"
        SELECT
          e.rank,
          t.tmdb_id,
          t.media_type,
          t.title,
          t.original_title,
          t.origin_countries,
          t.overview,
          t.poster_path,
          t.backdrop_path,
          t.release_date,
          t.release_year,
          t.vote_average,
          t.vote_count,
          matched.id AS on_demand_title_id,
          matched.provider_label
        FROM tmdb_chart_entries e
        JOIN tmdb_titles t
          ON t.media_type = e.media_type AND t.tmdb_id = e.tmdb_id
        LEFT JOIN LATERAL (
          SELECT o.id, COALESCE(p.label, p.base_url) AS provider_label
          FROM on_demand_titles o
          JOIN provider_profiles p ON p.id = o.profile_id
          WHERE o.user_id = $1
            AND o.media_type = e.media_type
            AND o.match_key = ANY(t.match_keys)
          ORDER BY
            CASE
              WHEN t.release_year IS NULL THEN 1
              WHEN substring(o.release_date from '\d{4}')::int = t.release_year THEN 0
              ELSE 1
            END,
            o.id
          LIMIT 1
        ) matched ON TRUE
        WHERE e.chart = $2
          AND e.media_type = $3
          AND e.country_mode = $4
          AND e.country_code = $5
        ORDER BY e.rank
        OFFSET $6 LIMIT $7
        "#,
    )
    .bind(user_id)
    .bind(chart)
    .bind(media_type)
    .bind(country_mode)
    .bind(country_code)
    .bind(offset)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub(super) async fn count_chart_titles(
    pool: &PgPool,
    chart: &str,
    media_type: &str,
    country_mode: &str,
    country_code: &str,
) -> Result<i64, AppError> {
    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM tmdb_chart_entries
        WHERE chart = $1 AND media_type = $2 AND country_mode = $3 AND country_code = $4
        "#,
    )
    .bind(chart)
    .bind(media_type)
    .bind(country_mode)
    .bind(country_code)
    .fetch_one(pool)
    .await?;
    Ok(total)
}

/// TMDB serves images from a public CDN, so unlike provider posters these are handed to
/// the browser directly and never go through the relay.
pub(super) fn tmdb_image_url(path: Option<&str>, size: &str) -> Option<String> {
    let path = path?;
    if path.is_empty() {
        return None;
    }
    Some(format!("{TMDB_IMAGE_BASE_URL}/{size}{path}"))
}

/// Extracts the four-digit year from TMDB's `YYYY-MM-DD` release dates. TV rows use
/// `first_air_date`, which has the same shape.
pub(super) fn release_year(release_date: Option<&str>) -> Option<i32> {
    release_date?.get(..4)?.parse().ok()
}
