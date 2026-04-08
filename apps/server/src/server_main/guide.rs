use super::*;

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/channels", get(list_channels))
        .route("/channels/{id}", get(get_channel))
        .route("/guide", get(get_guide))
        .route(
            "/guide/preferences",
            get(get_guide_preferences).put(save_guide_preferences),
        )
        .route("/guide/category/{category_id}", get(get_guide_category))
        .route("/guide/channel/{id}", get(get_channel_guide))
        .route("/favorites", get(list_favorites))
        .route(
            "/favorites/{channel_id}",
            post(add_favorite).delete(remove_favorite),
        )
        .route(
            "/favorites/categories/{category_id}",
            post(add_category_favorite).delete(remove_category_favorite),
        )
        .route("/favorites/order", put(save_favorite_order))
        .route("/recents", get(list_recents))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuideResponse {
    pub(super) categories: Vec<GuideCategorySummaryResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuidePreferencesResponse {
    pub(super) included_category_ids: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuideCategorySummaryResponse {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) channel_count: i64,
    pub(super) live_now_count: i64,
    pub(super) is_favorite: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuideChannelEntryResponse {
    pub(super) channel: ChannelResponse,
    pub(super) program: Option<ProgramResponse>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum FavoriteEntryResponse {
    Category {
        category: GuideCategorySummaryResponse,
        order: i32,
    },
    Channel {
        channel: ChannelResponse,
        program: Option<ProgramResponse>,
        order: i32,
    },
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuideCategoryResponse {
    pub(super) category: GuideCategorySummaryResponse,
    pub(super) entries: Vec<GuideChannelEntryResponse>,
    pub(super) total_count: i64,
    pub(super) next_offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RecentChannelResponse {
    pub(super) channel: ChannelResponse,
    pub(super) last_played_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveGuidePreferencesPayload {
    pub(super) included_category_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuideCategoryQuery {
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveFavoriteOrderPayload {
    pub(super) category_ids: Vec<Uuid>,
    pub(super) channel_ids: Vec<Uuid>,
}

async fn list_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<ChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let mut channels = fetch_channels(&state.pool, auth.user_id).await?;
    rewrite_channel_logo_urls(&state, &headers, auth.user_id, &mut channels)?;
    Ok(Json(channels))
}

async fn get_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<ChannelResponse> {
    let auth = require_auth(&state, &headers).await?;
    let mut channel = sqlx::query_as::<_, ChannelResponse>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          EXISTS(
            SELECT 1
            FROM programs p
            WHERE p.user_id = c.user_id
              AND p.channel_id = c.id
              AND p.end_at > NOW() - ($3 * INTERVAL '1 hour')
              AND p.start_at < NOW() + ($4 * INTERVAL '1 day')
          ) AS has_epg,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1 AND c.id = $2
        "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Channel not found".to_string()))?;
    channel.logo_url = rewrite_channel_logo_url(
        &state,
        &request_base_url(&state.config, &headers)?,
        auth.user_id,
        channel.profile_id,
        channel.logo_url,
    )?;

    Ok(Json(channel))
}

async fn get_guide(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<GuideResponse> {
    let auth = require_auth(&state, &headers).await?;
    let categories = fetch_guide_categories(&state.pool, auth.user_id).await?;
    Ok(Json(GuideResponse { categories }))
}

async fn get_guide_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<GuidePreferencesResponse> {
    let auth = require_auth(&state, &headers).await?;
    let included_category_ids = load_guide_preferences(&state.pool, auth.user_id).await?;

    Ok(Json(GuidePreferencesResponse {
        included_category_ids,
    }))
}

async fn save_guide_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveGuidePreferencesPayload>,
) -> ApiResult<GuidePreferencesResponse> {
    let auth = require_auth(&state, &headers).await?;
    let included_category_ids = normalize_category_ids(payload.included_category_ids);

    sqlx::query(
        r#"
        INSERT INTO user_guide_preferences (user_id, included_category_ids, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (user_id)
        DO UPDATE SET
          included_category_ids = EXCLUDED.included_category_ids,
          updated_at = NOW()
        "#,
    )
    .bind(auth.user_id)
    .bind(&included_category_ids)
    .execute(&state.pool)
    .await?;

    Ok(Json(GuidePreferencesResponse {
        included_category_ids,
    }))
}

async fn get_guide_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
    Query(query): Query<GuideCategoryQuery>,
) -> ApiResult<GuideCategoryResponse> {
    let auth = require_auth(&state, &headers).await?;
    let (offset, limit) = parse_guide_category_pagination(query)?;
    let category = fetch_guide_category_summary(&state.pool, auth.user_id, &category_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Guide category not found".to_string()))?;
    let total_count =
        fetch_guide_category_total_count(&state.pool, auth.user_id, &category_id).await?;
    let rows =
        fetch_guide_category_rows(&state.pool, auth.user_id, &category_id, offset, limit).await?;
    let request_base_url = request_base_url(&state.config, &headers)?;
    let entries = rows
        .into_iter()
        .map(|row| map_guide_category_entry(&state, &request_base_url, auth.user_id, row))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(GuideCategoryResponse {
        category,
        next_offset: next_guide_offset(offset, limit, total_count),
        total_count,
        entries,
    }))
}

async fn get_channel_guide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Vec<ProgramResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let programs = sqlx::query_as::<_, ProgramResponse>(
        r#"
        SELECT
          id,
          channel_id,
          channel_name,
          title,
          description,
          start_at,
          end_at,
          can_catchup
        FROM programs
        WHERE user_id = $1 AND channel_id = $2
        ORDER BY start_at ASC
        LIMIT 250
        "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(programs))
}

async fn list_favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<FavoriteEntryResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let request_base_url = request_base_url(&state.config, &headers)?;
    let category_favorites = sqlx::query_as::<_, FavoriteCategoryRow>(
        r#"
        SELECT
          cc.id::text AS id,
          cc.name AS name,
          COUNT(DISTINCT c.id) AS channel_count,
          COUNT(DISTINCT c.id) FILTER (
            WHERE p.start_at <= NOW() AND p.end_at > NOW()
          ) AS live_now_count,
          TRUE AS is_favorite,
          fcc.sort_order
        FROM favorite_channel_categories fcc
        JOIN channel_categories cc ON cc.id = fcc.category_id
        LEFT JOIN channels c
          ON c.user_id = fcc.user_id
         AND c.category_id = cc.id
        LEFT JOIN programs p
          ON p.user_id = c.user_id
         AND p.channel_id = c.id
         AND p.end_at > NOW() - ($2 * INTERVAL '1 hour')
         AND p.start_at < NOW() + ($3 * INTERVAL '1 day')
        WHERE fcc.user_id = $1
        GROUP BY cc.id, cc.name, fcc.sort_order
        ORDER BY fcc.sort_order ASC, cc.name ASC
        "#,
    )
    .bind(auth.user_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(&state.pool)
    .await?;
    let favorites = sqlx::query_as::<_, FavoriteChannelRow>(
        r#"
        SELECT
          c.id AS channel_id,
          c.profile_id,
          c.name AS channel_name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          TRUE AS is_favorite,
          p.id AS program_id,
          p.channel_id AS program_channel_id,
          p.channel_name AS program_channel_name,
          p.title AS program_title,
          p.description AS program_description,
          p.start_at AS program_start_at,
          p.end_at AS program_end_at,
          p.can_catchup AS program_can_catchup,
          f.sort_order
        FROM favorites f
        JOIN channels c ON c.id = f.channel_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        LEFT JOIN LATERAL (
          SELECT
            p.id,
            p.channel_id,
            p.channel_name,
            p.title,
            p.description,
            p.start_at,
            p.end_at,
            p.can_catchup,
            (p.start_at <= NOW() AND p.end_at > NOW()) AS is_live
          FROM programs p
          WHERE p.user_id = c.user_id
            AND p.channel_id = c.id
            AND p.end_at > NOW() - ($2 * INTERVAL '1 hour')
            AND p.start_at < NOW() + ($3 * INTERVAL '1 day')
          ORDER BY
            CASE
              WHEN p.start_at <= NOW() AND p.end_at > NOW() THEN 0
              WHEN p.start_at > NOW() THEN 1
              ELSE 2
            END ASC,
            CASE WHEN p.start_at > NOW() THEN p.start_at END ASC NULLS LAST,
            CASE WHEN p.start_at <= NOW() AND p.end_at > NOW() THEN p.start_at END DESC NULLS LAST,
            CASE WHEN p.end_at <= NOW() THEN p.end_at END DESC NULLS LAST,
            p.title ASC
          LIMIT 1
        ) p ON TRUE
        WHERE f.user_id = $1
        ORDER BY
          f.sort_order ASC,
          CASE
            WHEN p.start_at <= NOW() AND p.end_at > NOW() THEN 0
            WHEN p.start_at > NOW() THEN 1
            WHEN p.start_at IS NOT NULL THEN 2
            ELSE 3
          END ASC,
          CASE WHEN p.start_at > NOW() THEN p.start_at END ASC NULLS LAST,
          CASE WHEN p.end_at <= NOW() THEN p.end_at END DESC NULLS LAST,
          c.name ASC
        "#,
    )
    .bind(auth.user_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        category_favorites
            .into_iter()
            .map(|row| FavoriteEntryResponse::Category {
                category: map_guide_category_summary(GuideCategorySummaryRow {
                    id: row.id,
                    name: row.name,
                    channel_count: row.channel_count,
                    live_now_count: row.live_now_count,
                    is_favorite: row.is_favorite,
                }),
                order: row.sort_order,
            })
            .chain(
                favorites
                    .into_iter()
                    .map(|row| {
                        let order = row.sort_order;
                        map_guide_category_entry(
                            &state,
                            &request_base_url,
                            auth.user_id,
                            GuideCategoryEntryRow {
                                channel_id: row.channel_id,
                                profile_id: row.profile_id,
                                channel_name: row.channel_name,
                                logo_url: row.logo_url,
                                category_name: row.category_name,
                                remote_stream_id: row.remote_stream_id,
                                epg_channel_id: row.epg_channel_id,
                                has_catchup: row.has_catchup,
                                archive_duration_hours: row.archive_duration_hours,
                                stream_extension: row.stream_extension,
                                is_favorite: row.is_favorite,
                                program_id: row.program_id,
                                program_channel_id: row.program_channel_id,
                                program_channel_name: row.program_channel_name,
                                program_title: row.program_title,
                                program_description: row.program_description,
                                program_start_at: row.program_start_at,
                                program_end_at: row.program_end_at,
                                program_can_catchup: row.program_can_catchup,
                            },
                        )
                        .map(|entry| FavoriteEntryResponse::Channel {
                            channel: entry.channel,
                            program: entry.program,
                            order,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .collect(),
    ))
}

async fn save_favorite_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveFavoriteOrderPayload>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let category_ids = normalize_uuid_ids(payload.category_ids);
    let channel_ids = normalize_uuid_ids(payload.channel_ids);

    let mut transaction = state.pool.begin().await?;

    validate_favorite_category_ids(&mut transaction, auth.user_id, &category_ids).await?;
    validate_favorite_channel_ids(&mut transaction, auth.user_id, &channel_ids).await?;

    replace_favorite_category_order(&mut transaction, auth.user_id, &category_ids).await?;
    replace_favorite_channel_order(&mut transaction, auth.user_id, &channel_ids).await?;

    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let sort_order = next_favorite_channel_sort_order(&state.pool, auth.user_id).await?;
    sqlx::query(
        r#"
        INSERT INTO favorites (user_id, channel_id, sort_order)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, channel_id) DO NOTHING
        "#,
    )
    .bind(auth.user_id)
    .bind(channel_id)
    .bind(sort_order)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    sqlx::query("DELETE FROM favorites WHERE user_id = $1 AND channel_id = $2")
        .bind(auth.user_id)
        .bind(channel_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_category_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(category_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let sort_order = next_favorite_category_sort_order(&state.pool, auth.user_id).await?;
    sqlx::query(
        r#"
        INSERT INTO favorite_channel_categories (user_id, category_id, sort_order)
        SELECT $1, cc.id, $3
        FROM channel_categories cc
        WHERE cc.user_id = $1
          AND cc.id = $2
        ON CONFLICT (user_id, category_id) DO NOTHING
        "#,
    )
    .bind(auth.user_id)
    .bind(category_id)
    .bind(sort_order)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_category_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(category_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    sqlx::query("DELETE FROM favorite_channel_categories WHERE user_id = $1 AND category_id = $2")
        .bind(auth.user_id)
        .bind(category_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn next_favorite_channel_sort_order(pool: &PgPool, user_id: Uuid) -> Result<i32, AppError> {
    Ok(sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM favorites WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

async fn next_favorite_category_sort_order(pool: &PgPool, user_id: Uuid) -> Result<i32, AppError> {
    Ok(sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM favorite_channel_categories WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?)
}

fn normalize_uuid_ids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        if seen.insert(id) {
            normalized.push(id);
        }
    }
    normalized
}

async fn validate_favorite_channel_ids(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    let existing = sqlx::query_scalar::<_, Uuid>(
        "SELECT channel_id FROM favorites WHERE user_id = $1 ORDER BY sort_order ASC",
    )
    .bind(user_id)
    .fetch_all(transaction.as_mut())
    .await?;

    if existing.len() != ids.len()
        || existing.iter().copied().collect::<HashSet<_>>()
            != ids.iter().copied().collect::<HashSet<_>>()
    {
        return Err(AppError::BadRequest(
            "Favorite channel order must include every favorite channel exactly once.".to_string(),
        ));
    }

    Ok(())
}

async fn validate_favorite_category_ids(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    let existing = sqlx::query_scalar::<_, Uuid>(
        "SELECT category_id FROM favorite_channel_categories WHERE user_id = $1 ORDER BY sort_order ASC",
    )
    .bind(user_id)
    .fetch_all(transaction.as_mut())
    .await?;

    if existing.len() != ids.len()
        || existing.iter().copied().collect::<HashSet<_>>()
            != ids.iter().copied().collect::<HashSet<_>>()
    {
        return Err(AppError::BadRequest(
            "Favorite category order must include every favorite category exactly once."
                .to_string(),
        ));
    }

    Ok(())
}

async fn replace_favorite_channel_order(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    for (index, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE favorites SET sort_order = $3 WHERE user_id = $1 AND channel_id = $2")
            .bind(user_id)
            .bind(id)
            .bind(index as i32)
            .execute(transaction.as_mut())
            .await?;
    }

    Ok(())
}

async fn replace_favorite_category_order(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    for (index, id) in ids.iter().enumerate() {
        sqlx::query(
            "UPDATE favorite_channel_categories SET sort_order = $3 WHERE user_id = $1 AND category_id = $2",
        )
        .bind(user_id)
        .bind(id)
        .bind(index as i32)
        .execute(transaction.as_mut())
        .await?;
    }

    Ok(())
}

async fn list_recents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<RecentChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let rows = sqlx::query_as::<_, RecentChannelRow>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          EXISTS(
            SELECT 1
            FROM programs p
            WHERE p.user_id = c.user_id
              AND p.channel_id = c.id
              AND p.end_at > NOW() - ($2 * INTERVAL '1 hour')
              AND p.start_at < NOW() + ($3 * INTERVAL '1 day')
          ) AS has_epg,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite,
          r.last_played_at
        FROM recents r
        JOIN channels c ON c.id = r.channel_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE r.user_id = $1
        ORDER BY r.last_played_at DESC
        LIMIT 20
        "#,
    )
    .bind(auth.user_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(&state.pool)
    .await?;
    let request_base_url = request_base_url(&state.config, &headers)?;

    let recents = rows
        .into_iter()
        .map(|row| {
            Ok(RecentChannelResponse {
                channel: ChannelResponse {
                    id: row.id,
                    profile_id: row.profile_id,
                    name: row.name,
                    logo_url: rewrite_channel_logo_url(
                        &state,
                        &request_base_url,
                        auth.user_id,
                        row.profile_id,
                        row.logo_url,
                    )?,
                    category_name: row.category_name,
                    remote_stream_id: row.remote_stream_id,
                    epg_channel_id: row.epg_channel_id,
                    has_epg: row.has_epg,
                    has_catchup: row.has_catchup,
                    archive_duration_hours: row.archive_duration_hours,
                    stream_extension: row.stream_extension,
                    is_favorite: row.is_favorite,
                },
                last_played_at: row.last_played_at,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(recents))
}

pub(super) const GUIDE_DEFAULT_LIMIT: i64 = 40;
pub(super) const GUIDE_MAX_LIMIT: i64 = 100;

pub(super) async fn fetch_guide_categories(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<GuideCategorySummaryResponse>> {
    let rows = sqlx::query_as::<_, GuideCategorySummaryRow>(
        r#"
        SELECT
          COALESCE(c.category_id::text, 'uncategorized') AS id,
          COALESCE(cc.name, 'Uncategorized') AS name,
          COUNT(DISTINCT c.id) AS channel_count,
          COUNT(DISTINCT c.id) FILTER (
            WHERE p.start_at <= NOW() AND p.end_at > NOW()
          ) AS live_now_count,
          COALESCE(BOOL_OR(fcc.category_id IS NOT NULL), FALSE) AS is_favorite
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        LEFT JOIN favorite_channel_categories fcc
          ON fcc.user_id = c.user_id
         AND fcc.category_id = c.category_id
        LEFT JOIN programs p
          ON p.user_id = c.user_id
         AND p.channel_id = c.id
         AND p.end_at > NOW() - ($2 * INTERVAL '1 hour')
         AND p.start_at < NOW() + ($3 * INTERVAL '1 day')
        WHERE c.user_id = $1
        GROUP BY COALESCE(c.category_id::text, 'uncategorized'), COALESCE(cc.name, 'Uncategorized')
        ORDER BY live_now_count DESC, channel_count DESC, name ASC
        "#,
    )
    .bind(user_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_guide_category_summary).collect())
}

pub(super) async fn fetch_guide_category_summary(
    pool: &PgPool,
    user_id: Uuid,
    category_id: &str,
) -> Result<Option<GuideCategorySummaryResponse>> {
    let row = sqlx::query_as::<_, GuideCategorySummaryRow>(
        r#"
        SELECT
          COALESCE(c.category_id::text, 'uncategorized') AS id,
          COALESCE(cc.name, 'Uncategorized') AS name,
          COUNT(DISTINCT c.id) AS channel_count,
          COUNT(DISTINCT c.id) FILTER (
            WHERE p.start_at <= NOW() AND p.end_at > NOW()
          ) AS live_now_count,
          COALESCE(BOOL_OR(fcc.category_id IS NOT NULL), FALSE) AS is_favorite
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        LEFT JOIN favorite_channel_categories fcc
          ON fcc.user_id = c.user_id
         AND fcc.category_id = c.category_id
        LEFT JOIN programs p
          ON p.user_id = c.user_id
         AND p.channel_id = c.id
         AND p.end_at > NOW() - ($3 * INTERVAL '1 hour')
         AND p.start_at < NOW() + ($4 * INTERVAL '1 day')
        WHERE c.user_id = $1
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
        GROUP BY COALESCE(c.category_id::text, 'uncategorized'), COALESCE(cc.name, 'Uncategorized')
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_guide_category_summary))
}

pub(super) async fn fetch_guide_category_total_count(
    pool: &PgPool,
    user_id: Uuid,
    category_id: &str,
) -> Result<i64> {
    let total_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM channels c
        WHERE c.user_id = $1
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .fetch_one(pool)
    .await?;

    Ok(total_count)
}

pub(super) async fn fetch_guide_category_rows(
    pool: &PgPool,
    user_id: Uuid,
    category_id: &str,
    offset: i64,
    limit: i64,
) -> Result<Vec<GuideCategoryEntryRow>> {
    let rows = sqlx::query_as::<_, GuideCategoryEntryRow>(
        r#"
        SELECT
          c.id AS channel_id,
          c.profile_id,
          c.name AS channel_name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite,
          p.id AS program_id,
          p.channel_id AS program_channel_id,
          p.channel_name AS program_channel_name,
          p.title AS program_title,
          p.description AS program_description,
          p.start_at AS program_start_at,
          p.end_at AS program_end_at,
          p.can_catchup AS program_can_catchup
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        LEFT JOIN LATERAL (
          SELECT
            p.id,
            p.channel_id,
            p.channel_name,
            p.title,
            p.description,
            p.start_at,
            p.end_at,
            p.can_catchup,
            (p.start_at <= NOW() AND p.end_at > NOW()) AS is_live
          FROM programs p
          WHERE p.user_id = c.user_id
            AND p.channel_id = c.id
            AND p.end_at > NOW() - ($5 * INTERVAL '1 hour')
            AND p.start_at < NOW() + ($6 * INTERVAL '1 day')
          ORDER BY
            CASE
              WHEN p.start_at <= NOW() AND p.end_at > NOW() THEN 0
              WHEN p.start_at > NOW() THEN 1
              ELSE 2
            END ASC,
            CASE WHEN p.start_at > NOW() THEN p.start_at END ASC NULLS LAST,
            CASE WHEN p.start_at <= NOW() AND p.end_at > NOW() THEN p.start_at END DESC NULLS LAST,
            CASE WHEN p.end_at <= NOW() THEN p.end_at END DESC NULLS LAST,
            p.title ASC
          LIMIT 1
        ) p ON TRUE
        WHERE c.user_id = $1
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
        ORDER BY
          CASE
            WHEN p.start_at <= NOW() AND p.end_at > NOW() THEN 0
            WHEN p.start_at > NOW() THEN 1
            WHEN p.start_at IS NOT NULL THEN 2
            ELSE 3
          END ASC,
          CASE WHEN p.start_at > NOW() THEN p.start_at END ASC NULLS LAST,
          CASE WHEN p.end_at <= NOW() THEN p.end_at END DESC NULLS LAST,
          c.name ASC
        OFFSET $3
        LIMIT $4
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(offset)
    .bind(limit)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub(super) async fn load_guide_preferences(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>> {
    let included_category_ids = sqlx::query_scalar::<_, Vec<String>>(
        r#"
        SELECT included_category_ids
        FROM user_guide_preferences
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(included_category_ids.unwrap_or_default())
}

pub(super) fn parse_guide_category_pagination(
    query: GuideCategoryQuery,
) -> Result<(i64, i64), AppError> {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(GUIDE_DEFAULT_LIMIT);

    if offset < 0 {
        return Err(AppError::BadRequest(
            "Guide offset must be zero or greater".to_string(),
        ));
    }

    if limit <= 0 {
        return Err(AppError::BadRequest(
            "Guide limit must be greater than zero".to_string(),
        ));
    }

    Ok((offset, limit.min(GUIDE_MAX_LIMIT)))
}

pub(super) fn map_guide_category_summary(
    row: GuideCategorySummaryRow,
) -> GuideCategorySummaryResponse {
    GuideCategorySummaryResponse {
        id: row.id,
        name: row.name,
        channel_count: row.channel_count,
        live_now_count: row.live_now_count,
        is_favorite: row.is_favorite,
    }
}

pub(super) fn map_guide_category_entry(
    state: &AppState,
    request_base_url: &Url,
    user_id: Uuid,
    row: GuideCategoryEntryRow,
) -> Result<GuideChannelEntryResponse, AppError> {
    let program = map_guide_program_response(&row);

    Ok(GuideChannelEntryResponse {
        channel: ChannelResponse {
            id: row.channel_id,
            profile_id: row.profile_id,
            name: row.channel_name,
            logo_url: rewrite_channel_logo_url(
                state,
                request_base_url,
                user_id,
                row.profile_id,
                row.logo_url,
            )?,
            category_name: row.category_name,
            remote_stream_id: row.remote_stream_id,
            epg_channel_id: row.epg_channel_id,
            has_epg: row.program_id.is_some(),
            has_catchup: row.has_catchup,
            archive_duration_hours: row.archive_duration_hours,
            stream_extension: row.stream_extension,
            is_favorite: row.is_favorite,
        },
        program,
    })
}

pub(super) fn map_guide_program_response(row: &GuideCategoryEntryRow) -> Option<ProgramResponse> {
    let id = row.program_id?;
    let Some(start_at) = row.program_start_at else {
        warn!("guide entry for program {id} is missing program_start_at; omitting program payload");
        return None;
    };
    let Some(end_at) = row.program_end_at else {
        warn!("guide entry for program {id} is missing program_end_at; omitting program payload");
        return None;
    };

    Some(ProgramResponse {
        id,
        channel_id: row.program_channel_id,
        channel_name: row.program_channel_name.clone(),
        title: row.program_title.clone().unwrap_or_default(),
        description: row.program_description.clone(),
        start_at,
        end_at,
        can_catchup: row.program_can_catchup.unwrap_or(false),
    })
}

pub(super) fn next_guide_offset(offset: i64, limit: i64, total_count: i64) -> Option<i64> {
    let next_offset = offset + limit;
    (next_offset < total_count).then_some(next_offset)
}

pub(super) fn normalize_category_ids(category_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(category_ids.len());

    for category_id in category_ids {
        let category_id = category_id.trim();
        if category_id.is_empty() {
            continue;
        }

        if seen.insert(category_id.to_string()) {
            normalized.push(category_id.to_string());
        }
    }

    normalized
}

#[derive(Debug, FromRow)]
pub(super) struct RecentChannelRow {
    pub(super) id: Uuid,
    pub(super) profile_id: Uuid,
    pub(super) name: String,
    pub(super) logo_url: Option<String>,
    pub(super) category_name: Option<String>,
    pub(super) remote_stream_id: i32,
    pub(super) epg_channel_id: Option<String>,
    pub(super) has_epg: bool,
    pub(super) has_catchup: bool,
    pub(super) archive_duration_hours: Option<i32>,
    pub(super) stream_extension: Option<String>,
    pub(super) is_favorite: bool,
    pub(super) last_played_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(super) struct GuideCategorySummaryRow {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) channel_count: i64,
    pub(super) live_now_count: i64,
    pub(super) is_favorite: bool,
}

#[derive(Debug, FromRow)]
pub(super) struct GuideCategoryEntryRow {
    pub(super) channel_id: Uuid,
    pub(super) profile_id: Uuid,
    pub(super) channel_name: String,
    pub(super) logo_url: Option<String>,
    pub(super) category_name: Option<String>,
    pub(super) remote_stream_id: i32,
    pub(super) epg_channel_id: Option<String>,
    pub(super) has_catchup: bool,
    pub(super) archive_duration_hours: Option<i32>,
    pub(super) stream_extension: Option<String>,
    pub(super) is_favorite: bool,
    pub(super) program_id: Option<Uuid>,
    pub(super) program_channel_id: Option<Uuid>,
    pub(super) program_channel_name: Option<String>,
    pub(super) program_title: Option<String>,
    pub(super) program_description: Option<String>,
    pub(super) program_start_at: Option<DateTime<Utc>>,
    pub(super) program_end_at: Option<DateTime<Utc>>,
    pub(super) program_can_catchup: Option<bool>,
}

#[derive(Debug, FromRow)]
struct FavoriteCategoryRow {
    id: String,
    name: String,
    channel_count: i64,
    live_now_count: i64,
    is_favorite: bool,
    sort_order: i32,
}

#[derive(Debug, FromRow)]
struct FavoriteChannelRow {
    channel_id: Uuid,
    profile_id: Uuid,
    channel_name: String,
    logo_url: Option<String>,
    category_name: Option<String>,
    remote_stream_id: i32,
    epg_channel_id: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    stream_extension: Option<String>,
    is_favorite: bool,
    program_id: Option<Uuid>,
    program_channel_id: Option<Uuid>,
    program_channel_name: Option<String>,
    program_title: Option<String>,
    program_description: Option<String>,
    program_start_at: Option<DateTime<Utc>>,
    program_end_at: Option<DateTime<Utc>>,
    program_can_catchup: Option<bool>,
    sort_order: i32,
}

#[cfg(test)]
mod tests {
    use super::super::{request_base_url, rewrite_channel_logo_url};
    use super::*;

    #[test]
    fn parses_guide_category_pagination_defaults_and_caps_limit() {
        let (offset, limit) = parse_guide_category_pagination(GuideCategoryQuery {
            offset: None,
            limit: Some(GUIDE_MAX_LIMIT + 25),
        })
        .expect("pagination");

        assert_eq!(offset, 0);
        assert_eq!(limit, GUIDE_MAX_LIMIT);
    }

    #[test]
    fn rejects_negative_guide_category_offset() {
        let error = parse_guide_category_pagination(GuideCategoryQuery {
            offset: Some(-1),
            limit: Some(10),
        })
        .expect_err("negative offset should fail");

        match error {
            AppError::BadRequest(message) => assert!(message.contains("offset")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn computes_next_guide_offset_only_when_more_results_exist() {
        assert_eq!(next_guide_offset(0, 40, 81), Some(40));
        assert_eq!(next_guide_offset(40, 40, 80), None);
        assert_eq!(next_guide_offset(80, 40, 80), None);
    }

    #[test]
    fn guide_preferences_normalization_deduplicates_and_trims() {
        let normalized = normalize_category_ids(vec![
            " sports ".to_string(),
            "sports".to_string(),
            "".to_string(),
            "news".to_string(),
            "news".to_string(),
        ]);

        assert_eq!(normalized, vec!["sports".to_string(), "news".to_string()]);
    }

    #[test]
    fn guide_preferences_normalization_preserves_empty_arrays() {
        let normalized = normalize_category_ids(Vec::new());

        assert!(normalized.is_empty());
    }

    #[test]
    fn maps_guide_category_summary_favorite_state() {
        let summary = map_guide_category_summary(GuideCategorySummaryRow {
            id: "sports".to_string(),
            name: "Sports".to_string(),
            channel_count: 12,
            live_now_count: 3,
            is_favorite: true,
        });

        assert_eq!(summary.id, "sports");
        assert_eq!(summary.name, "Sports");
        assert_eq!(summary.channel_count, 12);
        assert_eq!(summary.live_now_count, 3);
        assert!(summary.is_favorite);
    }

    fn sample_app_state() -> AppState {
        AppState {
            pool: PgPoolOptions::new()
                .connect_lazy("postgres://euripus:euripus@localhost/euripus")
                .expect("lazy pool"),
            config: Arc::new(Config {
                bind_address: "127.0.0.1:4000".parse().expect("bind address"),
                database_url: "postgres://euripus:euripus@localhost/euripus".to_string(),
                jwt_secret: "test-jwt-secret".to_string(),
                relay_signing_secret: "test-relay-secret".to_string(),
                encryption_key: *b"0123456789abcdef0123456789abcdef",
                access_token_minutes: 15,
                refresh_token_days: 7,
                relay_token_minutes: 30,
                daily_sync_hour_local: 6,
                public_origin: Some(Url::parse("https://app.example.com").expect("public origin")),
                allowed_origins: vec!["https://app.example.com".to_string()],
                browser_cookie_secure: true,
                vpn_enabled: false,
                vpn_provider_name: None,
                meilisearch_url: None,
                meilisearch_api_key: None,
            }),
            provider_http_client: reqwest::Client::new(),
            relay_http_client: reqwest::Client::new(),
            meili: None,
            meili_readiness: Arc::new(RwLock::new(MeiliReadiness::Disabled)),
            meili_schema_ready: Arc::new(RwLock::new(true)),
            meili_bootstrapping_users: Arc::new(DashSet::new()),
            search_lexicons: Arc::new(DashMap::new()),
            session_cache: Arc::new(DashMap::new()),
            relay_profile_cache: Arc::new(DashMap::new()),
            receiver_channels: Arc::new(DashMap::new()),
        }
    }

    #[tokio::test]
    async fn maps_guide_entry_rows_into_nested_payloads() {
        let now = Utc::now();
        let state = sample_app_state();
        let request_base_url = Url::parse("https://app.example.com").expect("request base url");
        let entry = map_guide_category_entry(
            &state,
            &request_base_url,
            Uuid::from_u128(51),
            GuideCategoryEntryRow {
                channel_id: Uuid::nil(),
                profile_id: Uuid::from_u128(52),
                channel_name: "Arena 1".to_string(),
                logo_url: Some("https://example.com/logo.png".to_string()),
                category_name: Some("Uncategorized".to_string()),
                remote_stream_id: 7,
                epg_channel_id: Some("arena.1".to_string()),
                has_catchup: true,
                archive_duration_hours: Some(48),
                stream_extension: Some("m3u8".to_string()),
                is_favorite: true,
                program_id: Some(Uuid::from_u128(42)),
                program_channel_id: Some(Uuid::nil()),
                program_channel_name: Some("Arena 1".to_string()),
                program_title: Some("Matchday Live".to_string()),
                program_description: Some("Quarterfinal".to_string()),
                program_start_at: Some(now),
                program_end_at: Some(now + ChronoDuration::hours(2)),
                program_can_catchup: Some(true),
            },
        )
        .expect("guide entry");

        assert_eq!(entry.channel.name, "Arena 1");
        assert_eq!(
            entry.channel.category_name.as_deref(),
            Some("Uncategorized")
        );
        assert!(entry.channel.is_favorite);
        assert_eq!(
            entry.program.as_ref().map(|program| program.title.as_str()),
            Some("Matchday Live")
        );
        assert_eq!(
            entry
                .program
                .as_ref()
                .and_then(|program| program.channel_name.as_deref()),
            Some("Arena 1")
        );
        assert_eq!(
            entry.program.as_ref().map(|program| program.can_catchup),
            Some(true)
        );
    }

    #[tokio::test]
    async fn maps_guide_entry_rows_without_programs() {
        let state = sample_app_state();
        let request_base_url = Url::parse("https://app.example.com").expect("request base url");
        let entry = map_guide_category_entry(
            &state,
            &request_base_url,
            Uuid::from_u128(53),
            GuideCategoryEntryRow {
                channel_id: Uuid::nil(),
                profile_id: Uuid::from_u128(54),
                channel_name: "Arena 2".to_string(),
                logo_url: None,
                category_name: Some("Sports".to_string()),
                remote_stream_id: 8,
                epg_channel_id: None,
                has_catchup: false,
                archive_duration_hours: None,
                stream_extension: Some("m3u8".to_string()),
                is_favorite: false,
                program_id: None,
                program_channel_id: None,
                program_channel_name: None,
                program_title: None,
                program_description: None,
                program_start_at: None,
                program_end_at: None,
                program_can_catchup: None,
            },
        )
        .expect("guide entry");

        assert_eq!(entry.channel.name, "Arena 2");
        assert!(entry.program.is_none());
    }

    #[tokio::test]
    async fn maps_guide_entry_rows_with_incomplete_programs_without_panicking() {
        let state = sample_app_state();
        let request_base_url = Url::parse("https://app.example.com").expect("request base url");
        let entry = map_guide_category_entry(
            &state,
            &request_base_url,
            Uuid::from_u128(55),
            GuideCategoryEntryRow {
                channel_id: Uuid::nil(),
                profile_id: Uuid::from_u128(56),
                channel_name: "Arena 3".to_string(),
                logo_url: None,
                category_name: Some("Sports".to_string()),
                remote_stream_id: 9,
                epg_channel_id: None,
                has_catchup: false,
                archive_duration_hours: None,
                stream_extension: Some("m3u8".to_string()),
                is_favorite: false,
                program_id: Some(Uuid::from_u128(57)),
                program_channel_id: Some(Uuid::nil()),
                program_channel_name: Some("Arena 3".to_string()),
                program_title: Some("Broken Listing".to_string()),
                program_description: None,
                program_start_at: None,
                program_end_at: Some(Utc::now() + ChronoDuration::hours(1)),
                program_can_catchup: Some(false),
            },
        )
        .expect("guide entry");

        assert_eq!(entry.channel.name, "Arena 3");
        assert!(entry.program.is_none());
    }

    #[tokio::test]
    async fn rewrite_channel_logo_url_relays_http_logos_on_https_pages() {
        let state = sample_app_state();
        let request_base_url = Url::parse("https://app.example.com").expect("request base url");

        let logo_url = rewrite_channel_logo_url(
            &state,
            &request_base_url,
            Uuid::from_u128(41),
            Uuid::from_u128(42),
            Some("http://provider.example.com/logo.png".to_string()),
        )
        .expect("rewritten logo url")
        .expect("logo url");

        assert!(logo_url.starts_with("https://app.example.com/api/relay/asset?token="));
    }

    #[tokio::test]
    async fn rewrite_channel_logo_url_keeps_https_logos_direct() {
        let state = sample_app_state();
        let request_base_url = Url::parse("https://app.example.com").expect("request base url");

        let logo_url = rewrite_channel_logo_url(
            &state,
            &request_base_url,
            Uuid::from_u128(43),
            Uuid::from_u128(44),
            Some("https://provider.example.com/logo.png".to_string()),
        )
        .expect("rewritten logo url");

        assert_eq!(
            logo_url.as_deref(),
            Some("https://provider.example.com/logo.png")
        );
    }

    #[tokio::test]
    async fn request_base_url_prefers_public_origin_over_forwarded_headers() {
        let state = sample_app_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-forwarded-host"),
            HeaderValue::from_static("internal.example.com"),
        );
        headers.insert(
            HeaderName::from_static("x-forwarded-proto"),
            HeaderValue::from_static("http"),
        );

        let url = request_base_url(&state.config, &headers).expect("request base url");

        assert_eq!(url.as_str(), "https://app.example.com/");
    }
}
