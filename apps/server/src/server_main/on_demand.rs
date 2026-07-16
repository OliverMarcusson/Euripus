use super::*;

const DEFAULT_LIMIT: i64 = 48;
const MAX_LIMIT: i64 = 100;
const EPISODE_CACHE_HOURS: i64 = 24;

#[derive(Debug, Serialize, FromRow, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct OnDemandTitleResponse {
    pub(super) id: Uuid,
    #[serde(skip_serializing)]
    pub(super) profile_id: Uuid,
    pub(super) media_type: String,
    pub(super) name: String,
    pub(super) category_id: Option<Uuid>,
    pub(super) category_name: Option<String>,
    pub(super) poster_url: Option<String>,
    pub(super) backdrop_url: Option<String>,
    pub(super) plot: Option<String>,
    pub(super) genre: Option<String>,
    pub(super) cast_names: Option<String>,
    pub(super) director: Option<String>,
    pub(super) release_date: Option<String>,
    pub(super) rating: Option<f64>,
    pub(super) duration_minutes: Option<i32>,
    pub(super) container_extension: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct OnDemandCategoryResponse {
    id: Uuid,
    media_type: String,
    name: String,
    title_count: i64,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub(super) struct OnDemandEpisodeResponse {
    pub(super) id: Uuid,
    #[serde(skip_serializing)]
    pub(super) profile_id: Uuid,
    pub(super) series_id: Uuid,
    pub(super) season_number: i32,
    pub(super) episode_number: i32,
    pub(super) name: String,
    pub(super) plot: Option<String>,
    pub(super) duration_minutes: Option<i32>,
    pub(super) poster_url: Option<String>,
    pub(super) container_extension: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OnDemandPageResponse {
    items: Vec<OnDemandTitleResponse>,
    total_count: i64,
    next_offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogQuery {
    #[serde(rename = "type")]
    media_type: Option<String>,
    category_id: Option<Uuid>,
    query: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/on-demand/categories", get(list_categories))
        .route("/on-demand/titles", get(list_titles))
        .route("/on-demand/titles/{id}", get(get_title))
        .route("/on-demand/series/{id}/episodes", get(list_series_episodes))
}

fn normalize_media_type(raw: Option<&str>) -> Result<&str, AppError> {
    match raw.unwrap_or("movie") {
        "movie" => Ok("movie"),
        "series" => Ok("series"),
        _ => Err(AppError::BadRequest(
            "On-demand type must be 'movie' or 'series'.".to_string(),
        )),
    }
}

async fn list_categories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CatalogQuery>,
) -> ApiResult<Vec<OnDemandCategoryResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let media_type = normalize_media_type(query.media_type.as_deref())?;
    let rows = sqlx::query_as::<_, OnDemandCategoryResponse>(
        r#"
        SELECT c.id, c.media_type, c.name, COUNT(t.id) AS title_count
        FROM on_demand_categories c
        LEFT JOIN on_demand_titles t ON t.category_id = c.id
        WHERE c.user_id = $1 AND c.media_type = $2
        GROUP BY c.id ORDER BY lower(c.name), c.id
    "#,
    )
    .bind(auth.user_id)
    .bind(media_type)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn list_titles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CatalogQuery>,
) -> ApiResult<OnDemandPageResponse> {
    let auth = require_auth(&state, &headers).await?;
    let media_type = normalize_media_type(query.media_type.as_deref())?;
    let offset = query.offset.unwrap_or(0).max(0);
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let term = query
        .query
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let total_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM on_demand_titles t
        WHERE t.user_id = $1 AND t.media_type = $2
          AND ($3::uuid IS NULL OR t.category_id = $3)
          AND ($4::text IS NULL OR t.name ILIKE ('%' || $4 || '%'))
    "#,
    )
    .bind(auth.user_id)
    .bind(media_type)
    .bind(query.category_id)
    .bind(term)
    .fetch_one(&state.pool)
    .await?;
    let mut items = sqlx::query_as::<_, OnDemandTitleResponse>(&format!(
        r#"
        SELECT t.id, t.profile_id, t.media_type, t.name, t.category_id, c.name AS category_name,
          t.poster_url, t.backdrop_url, t.plot, t.genre, t.cast_names, t.director,
          t.release_date, t.rating, t.duration_minutes, t.container_extension
        FROM on_demand_titles t LEFT JOIN on_demand_categories c ON c.id = t.category_id
        WHERE t.user_id = $1 AND t.media_type = $2
          AND ($3::uuid IS NULL OR t.category_id = $3)
          AND ($4::text IS NULL OR t.name ILIKE ('%' || $4 || '%'))
        ORDER BY lower(t.name), t.id OFFSET $5 LIMIT $6
    "#
    ))
    .bind(auth.user_id)
    .bind(media_type)
    .bind(query.category_id)
    .bind(term)
    .bind(offset)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    rewrite_title_images(&state, &headers, auth.user_id, &mut items)?;
    Ok(Json(OnDemandPageResponse {
        items,
        total_count,
        next_offset: (offset + limit < total_count).then_some(offset + limit),
    }))
}

async fn get_title(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<OnDemandTitleResponse> {
    let auth = require_auth(&state, &headers).await?;
    refresh_movie_details_if_stale(&state, auth.user_id, id).await?;
    let mut item = load_title(&state.pool, auth.user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("On-demand title not found".to_string()))?;
    rewrite_title_images(
        &state,
        &headers,
        auth.user_id,
        std::slice::from_mut(&mut item),
    )?;
    Ok(Json(item))
}

#[derive(Debug, FromRow)]
struct MovieDetailRecord {
    profile_id: Uuid,
    remote_id: String,
    details_fetched_at: Option<DateTime<Utc>>,
    base_url: String,
    username: String,
    password_encrypted: String,
    output_format: String,
}

async fn refresh_movie_details_if_stale(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
) -> Result<(), AppError> {
    let record = sqlx::query_as::<_, MovieDetailRecord>(
        r#"
        SELECT t.profile_id, t.remote_id, t.details_fetched_at,
          p.base_url, p.username, p.password_encrypted, p.output_format
        FROM on_demand_titles t JOIN provider_profiles p ON p.id = t.profile_id
        WHERE t.user_id = $1 AND t.id = $2 AND t.media_type = 'movie'
    "#,
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(record) = record else {
        return Ok(());
    };
    if record
        .details_fetched_at
        .is_some_and(|at| at >= Utc::now() - ChronoDuration::hours(EPISODE_CACHE_HOURS))
    {
        return Ok(());
    }
    let credentials = XtreamCredentials {
        base_url: record.base_url,
        username: record.username,
        password: decrypt_secret(&state.config.encryption_key, &record.password_encrypted)?,
        output_format: record.output_format,
    };
    match xtreme::fetch_vod_info(&state.provider_http_client, &credentials, &record.remote_id).await
    {
        Ok(details) => {
            sqlx::query(r#"UPDATE on_demand_titles SET
              poster_url=COALESCE($2,poster_url), backdrop_url=COALESCE($3,backdrop_url), plot=COALESCE($4,plot),
              genre=COALESCE($5,genre), cast_names=COALESCE($6,cast_names), director=COALESCE($7,director),
              release_date=COALESCE($8,release_date), rating=COALESCE($9,rating), duration_minutes=COALESCE($10,duration_minutes),
              details_fetched_at=NOW(), updated_at=NOW() WHERE id=$1"#)
              .bind(id).bind(details.poster_url).bind(details.backdrop_url).bind(details.plot).bind(details.genre)
              .bind(details.cast_names).bind(details.director).bind(details.release_date).bind(details.rating)
              .bind(details.duration_minutes).execute(&state.pool).await?;
        }
        Err(error) => {
            warn!(title_id = %id, error = ?error, "unable to refresh Xtream movie details; using catalog metadata")
        }
    }
    Ok(())
}

async fn load_title(
    pool: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> Result<Option<OnDemandTitleResponse>, AppError> {
    Ok(sqlx::query_as::<_, OnDemandTitleResponse>(
        r#"
        SELECT t.id, t.profile_id, t.media_type, t.name, t.category_id, c.name AS category_name,
          t.poster_url, t.backdrop_url, t.plot, t.genre, t.cast_names, t.director,
          t.release_date, t.rating, t.duration_minutes, t.container_extension
        FROM on_demand_titles t LEFT JOIN on_demand_categories c ON c.id = t.category_id
        WHERE t.user_id = $1 AND t.id = $2
    "#,
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

async fn list_series_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Vec<OnDemandEpisodeResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let series = sqlx::query_as::<_, SeriesCacheRecord>(
        r#"
        SELECT t.id, t.profile_id, t.remote_id, t.episodes_fetched_at,
          p.base_url, p.username, p.password_encrypted, p.output_format
        FROM on_demand_titles t JOIN provider_profiles p ON p.id = t.profile_id
        WHERE t.user_id = $1 AND t.id = $2 AND t.media_type = 'series'
    "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Series not found".to_string()))?;
    let stale = series
        .episodes_fetched_at
        .is_none_or(|at| at < Utc::now() - ChronoDuration::hours(EPISODE_CACHE_HOURS));
    if stale {
        let credentials = XtreamCredentials {
            base_url: series.base_url,
            username: series.username,
            password: decrypt_secret(&state.config.encryption_key, &series.password_encrypted)?,
            output_format: series.output_format,
        };
        match xtreme::fetch_series_episodes(
            &state.provider_http_client,
            &credentials,
            &series.remote_id,
        )
        .await
        {
            Ok(episodes) => {
                persist_episodes(
                    &state.pool,
                    auth.user_id,
                    series.profile_id,
                    series.id,
                    &episodes,
                )
                .await?
            }
            Err(error) => {
                let count = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM on_demand_episodes WHERE series_id = $1",
                )
                .bind(id)
                .fetch_one(&state.pool)
                .await?;
                if count == 0 {
                    return Err(AppError::Internal(error));
                }
                warn!(series_id = %id, error = ?error, "using stale series episode cache");
            }
        }
    }
    let mut rows = sqlx::query_as::<_, OnDemandEpisodeResponse>(
        r#"
        SELECT id, profile_id, series_id, season_number, episode_number, name, plot,
          duration_minutes, poster_url, container_extension
        FROM on_demand_episodes WHERE user_id = $1 AND series_id = $2
        ORDER BY season_number, episode_number, id
    "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let base = request_base_url(&state.config, &headers)?;
    for row in &mut rows {
        row.poster_url = rewrite_channel_logo_url(
            &state,
            &base,
            auth.user_id,
            row.profile_id,
            row.poster_url.take(),
        )?;
    }
    Ok(Json(rows))
}

#[derive(Debug, FromRow)]
struct SeriesCacheRecord {
    id: Uuid,
    profile_id: Uuid,
    remote_id: String,
    episodes_fetched_at: Option<DateTime<Utc>>,
    base_url: String,
    username: String,
    password_encrypted: String,
    output_format: String,
}

async fn persist_episodes(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    series_id: Uuid,
    episodes: &[xtreme::XtreamEpisode],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM on_demand_episodes WHERE series_id = $1")
        .bind(series_id)
        .execute(&mut *tx)
        .await?;
    for item in episodes.iter().filter(|item| !item.remote_id.is_empty()) {
        sqlx::query(r#"INSERT INTO on_demand_episodes
          (user_id, profile_id, series_id, remote_id, season_number, episode_number, name, plot, duration_minutes, poster_url, container_extension)
          VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#)
          .bind(user_id).bind(profile_id).bind(series_id).bind(&item.remote_id).bind(item.season_number)
          .bind(item.episode_number).bind(&item.name).bind(&item.plot).bind(item.duration_minutes)
          .bind(&item.poster_url).bind(&item.container_extension).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE on_demand_titles SET episodes_fetched_at = NOW() WHERE id = $1")
        .bind(series_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

fn rewrite_title_images(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    items: &mut [OnDemandTitleResponse],
) -> Result<(), AppError> {
    let base = request_base_url(&state.config, headers)?;
    for item in items {
        item.poster_url = rewrite_channel_logo_url(
            state,
            &base,
            user_id,
            item.profile_id,
            item.poster_url.take(),
        )?;
        item.backdrop_url = rewrite_channel_logo_url(
            state,
            &base,
            user_id,
            item.profile_id,
            item.backdrop_url.take(),
        )?;
    }
    Ok(())
}

fn consume_catalog_title_tag<'a>(value: &'a str, tag: &str) -> Option<&'a str> {
    let prefix = value.get(..tag.len())?;
    if !prefix.eq_ignore_ascii_case(tag) {
        return None;
    }

    let remainder = &value[tag.len()..];
    if remainder.is_empty() {
        return Some(remainder);
    }
    let first = remainder.chars().next()?;
    if !first.is_ascii_whitespace() && !matches!(first, '-' | '_' | ':' | '|') {
        return None;
    }
    Some(remainder.trim_start_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, '-' | '_' | ':' | '|')
    }))
}

fn is_preferred_catalog_title(name: &str) -> bool {
    let mut remainder = name.trim_start();
    loop {
        let quality_remainder = ["4K", "UHD", "FHD", "HD"]
            .iter()
            .find_map(|tag| consume_catalog_title_tag(remainder, tag));
        match quality_remainder {
            Some(value) if !value.is_empty() => remainder = value,
            _ => break,
        }
    }

    // Xtream title prefixes are overloaded: EN/SE identify language while the other
    // supported tags identify the source platform. Platform catalogs are retained even
    // when the provider does not include a separate language tag.
    [
        "EN", "SE", "NF", "AMZ", "A+", "D+", "PRMT", "VP", "MRVL", "DSC+", "SKY", "MAX", "P+",
        "PCOK", "SHWT",
    ]
    .iter()
    .any(|tag| consume_catalog_title_tag(remainder, tag).is_some())
}

fn should_persist_title(item: &xtreme::XtreamOnDemandTitle) -> bool {
    !item.remote_id.is_empty() && is_preferred_catalog_title(&item.name)
}

pub(super) async fn persist_catalog(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    media_type: &str,
    categories: &[XtreamCategory],
    titles: &[xtreme::XtreamOnDemandTitle],
) -> Result<()> {
    let mut tx = pool.begin().await?;
    for category in categories
        .iter()
        .filter(|category| !category.remote_category_id.is_empty())
    {
        sqlx::query(r#"INSERT INTO on_demand_categories (user_id,profile_id,media_type,remote_category_id,name)
          VALUES ($1,$2,$3,$4,$5) ON CONFLICT (user_id,profile_id,media_type,remote_category_id)
          DO UPDATE SET name=EXCLUDED.name, updated_at=NOW()"#)
          .bind(user_id).bind(profile_id).bind(media_type).bind(&category.remote_category_id).bind(&category.name).execute(&mut *tx).await?;
    }
    for item in titles.iter().filter(|item| should_persist_title(item)) {
        sqlx::query(r#"INSERT INTO on_demand_titles
          (user_id,profile_id,category_id,media_type,remote_id,name,poster_url,backdrop_url,plot,genre,cast_names,director,release_date,rating,duration_minutes,container_extension,provider_updated_at)
          SELECT $1,$2,c.id,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16 FROM (SELECT 1) x
          LEFT JOIN on_demand_categories c ON c.user_id=$1 AND c.profile_id=$2 AND c.media_type=$3 AND c.remote_category_id=$17
          ON CONFLICT (user_id,profile_id,media_type,remote_id) DO UPDATE SET category_id=EXCLUDED.category_id,name=EXCLUDED.name,
          poster_url=EXCLUDED.poster_url,backdrop_url=EXCLUDED.backdrop_url,plot=EXCLUDED.plot,genre=EXCLUDED.genre,cast_names=EXCLUDED.cast_names,
          director=EXCLUDED.director,release_date=EXCLUDED.release_date,rating=EXCLUDED.rating,duration_minutes=EXCLUDED.duration_minutes,
          container_extension=EXCLUDED.container_extension,provider_updated_at=EXCLUDED.provider_updated_at,updated_at=NOW()"#)
          .bind(user_id).bind(profile_id).bind(media_type).bind(&item.remote_id).bind(&item.name).bind(&item.poster_url).bind(&item.backdrop_url)
          .bind(&item.plot).bind(&item.genre).bind(&item.cast_names).bind(&item.director).bind(&item.release_date).bind(item.rating)
          .bind(item.duration_minutes).bind(&item.container_extension).bind(item.provider_updated_at).bind(&item.category_id).execute(&mut *tx).await?;
    }
    let ids = titles
        .iter()
        .filter(|item| should_persist_title(item))
        .map(|item| item.remote_id.clone())
        .collect::<Vec<_>>();
    sqlx::query("DELETE FROM on_demand_titles WHERE user_id=$1 AND profile_id=$2 AND media_type=$3 AND NOT (remote_id = ANY($4::text[]))")
      .bind(user_id).bind(profile_id).bind(media_type).bind(&ids).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM on_demand_categories WHERE user_id=$1 AND profile_id=$2 AND media_type=$3 AND NOT EXISTS (SELECT 1 FROM on_demand_titles t WHERE t.category_id=on_demand_categories.id)")
      .bind(user_id).bind(profile_id).bind(media_type).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_preferred_catalog_title;

    #[test]
    fn accepts_language_and_platform_title_prefixes() {
        for title in [
            "EN - Slow Horses",
            "SE - Bron",
            "4K-EN - Foundation",
            "4K - SE: Tunna blå linjen",
            "uhd_en | Silo",
            "FHD:EN- The Bear",
            "NF - The Crown",
            "4K-AMZ - Reacher",
            "A+ - Severance",
            "4K-D+ - Andor",
            "MAX - The Last of Us",
            "P+ - Star Trek",
            "NF-DO - Our Planet",
        ] {
            assert!(is_preferred_catalog_title(title), "rejected {title:?}");
        }
    }

    #[test]
    fn rejects_other_or_embedded_language_markers() {
        for title in [
            "ES - La casa de papel",
            "AR-EN-S - Example",
            "IN-EN - Example",
            "ENGLISH SERIES",
            "4K-TOP - Example",
            "SC - Nordic collection",
            "The Last Enemy",
            "",
        ] {
            assert!(!is_preferred_catalog_title(title), "accepted {title:?}");
        }
    }
}
