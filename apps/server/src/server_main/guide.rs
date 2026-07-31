use super::*;

mod favorites;

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
        .route("/favorites", get(favorites::list_favorites))
        .route("/favorites/ppv", get(favorites::list_ppv_favorites))
        .route(
            "/favorites/ppv/{channel_id}",
            post(favorites::add_ppv_favorite).delete(favorites::remove_ppv_favorite),
        )
        .route(
            "/favorites/ppv/order",
            put(favorites::save_ppv_favorite_order),
        )
        .route(
            "/favorites/{channel_id}",
            post(favorites::add_favorite).delete(favorites::remove_favorite),
        )
        .route(
            "/favorites/categories/{category_id}",
            post(favorites::add_category_favorite).delete(favorites::remove_category_favorite),
        )
        .route("/favorites/order", put(favorites::save_favorite_order))
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
pub(super) struct GuideOverviewQuery {
    pub(super) with_epg_only: Option<bool>,
    pub(super) quality_channels_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChannelListQuery {
    pub(super) quality_channels_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuideCategoryQuery {
    pub(super) offset: Option<i64>,
    pub(super) limit: Option<i64>,
    pub(super) with_epg_only: Option<bool>,
    pub(super) quality_channels_only: Option<bool>,
}

async fn list_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ChannelListQuery>,
) -> ApiResult<Vec<ChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let visibility = load_channel_visibility_map(&state, auth.user_id, None).await?;
    let mut visible_channel_ids = visibility
        .iter()
        .filter_map(|(id, visibility)| (!visibility.is_hidden).then_some(*id))
        .collect::<HashSet<_>>();
    if query.quality_channels_only.unwrap_or(false) {
        visible_channel_ids = quality_channel_ids(
            &state.pool,
            auth.user_id,
            visible_channel_ids.into_iter().collect(),
        )
        .await?
        .into_iter()
        .collect();
    }
    let mut channels = fetch_channels(&state.pool, auth.user_id).await?;
    channels.retain(|channel| visible_channel_ids.contains(&channel.id));
    rewrite_channel_logo_urls(&state, &headers, auth.user_id, &mut channels)?;
    Ok(Json(channels))
}

async fn get_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<ChannelResponse> {
    let auth = require_auth(&state, &headers).await?;
    let visibility = load_channel_visibility_map(&state, auth.user_id, None).await?;
    if visibility.get(&id).is_some_and(|value| value.is_hidden) {
        return Err(AppError::NotFound("Channel not found".to_string()));
    }
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
          ) AS is_favorite,
          c.search_is_ppv AS is_ppv,
          EXISTS(
            SELECT 1 FROM favorite_ppv_channels fpc
            WHERE fpc.user_id = c.user_id AND fpc.channel_id = c.id
          ) AS is_ppv_favorite
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1 AND c.id = $2
          AND c.profile_id = (SELECT live_provider_id FROM users WHERE id = $1)
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

async fn get_guide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GuideOverviewQuery>,
) -> Result<Response, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let visibility = load_channel_visibility_map(&state, auth.user_id, None).await?;
    let mut visible_channel_ids = visible_channel_ids_from_map(&visibility);
    if query.quality_channels_only.unwrap_or(false) {
        visible_channel_ids =
            quality_channel_ids(&state.pool, auth.user_id, visible_channel_ids).await?;
    }
    let payload = GuideResponse {
        categories: fetch_guide_categories(
            &state.pool,
            auth.user_id,
            &visible_channel_ids,
            query.with_epg_only.unwrap_or(false),
        )
        .await?,
    };
    json_response_with_revalidation(&headers, &payload)
}

async fn get_guide_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let payload = GuidePreferencesResponse {
        included_category_ids: load_guide_preferences(&state.pool, auth.user_id).await?,
    };

    json_response_with_revalidation(&headers, &payload)
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
) -> Result<Response, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let with_epg_only = query.with_epg_only.unwrap_or(false);
    let quality_channels_only = query.quality_channels_only.unwrap_or(false);
    let (offset, limit) = parse_guide_category_pagination(query)?;
    let visibility = load_channel_visibility_map(&state, auth.user_id, None).await?;
    let mut visible_channel_ids = visible_channel_ids_from_map(&visibility);
    if quality_channels_only {
        visible_channel_ids =
            quality_channel_ids(&state.pool, auth.user_id, visible_channel_ids).await?;
    }
    let category = fetch_guide_category_summary(
        &state.pool,
        auth.user_id,
        &category_id,
        &visible_channel_ids,
        with_epg_only,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Guide category not found".to_string()))?;
    let total_count = fetch_guide_category_total_count(
        &state.pool,
        auth.user_id,
        &category_id,
        &visible_channel_ids,
        with_epg_only,
    )
    .await?;
    let rows = fetch_guide_category_rows(
        &state.pool,
        auth.user_id,
        &category_id,
        offset,
        limit,
        &visible_channel_ids,
        with_epg_only,
    )
    .await?;
    let request_base_url = request_base_url(&state.config, &headers)?;
    let entries = rows
        .into_iter()
        .map(|row| map_guide_category_entry(&state, &request_base_url, auth.user_id, row))
        .collect::<Result<Vec<_>, _>>()?;

    let payload = GuideCategoryResponse {
        category,
        next_offset: next_guide_offset(offset, limit, total_count),
        total_count,
        entries,
    };

    json_response_with_revalidation(&headers, &payload)
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

async fn list_recents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
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
          c.search_is_ppv AS is_ppv,
          EXISTS(
            SELECT 1 FROM favorite_ppv_channels fpc
            WHERE fpc.user_id = c.user_id AND fpc.channel_id = c.id
          ) AS is_ppv_favorite,
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
                    is_ppv: row.is_ppv,
                    is_ppv_favorite: row.is_ppv_favorite,
                },
                last_played_at: row.last_played_at,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    json_response_with_revalidation(&headers, &recents)
}

pub(super) async fn quality_channel_ids(
    pool: &PgPool,
    user_id: Uuid,
    candidate_ids: Vec<Uuid>,
) -> Result<Vec<Uuid>> {
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sqlx::query_scalar::<_, Uuid>(r#"
        SELECT c.id FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1 AND c.id = ANY($2)
          AND (
            NOT EXISTS (SELECT 1 FROM admin_quality_channel_prefixes)
            OR EXISTS (SELECT 1 FROM admin_quality_channel_prefixes q
              WHERE UPPER((regexp_match(c.name, '^[[:space:]]*[|]?[[:space:]]*([A-Za-z0-9]{2,3})[[:space:]]*[|]'))[1]) = RTRIM(q.prefix, '|')
                 OR UPPER((regexp_match(COALESCE(cc.name, ''), '^[[:space:]]*[|]?[[:space:]]*([A-Za-z0-9]{2,3})[[:space:]]*[|]'))[1]) = RTRIM(q.prefix, '|'))
            OR (
              COALESCE((SELECT include_categories_without_country_prefix FROM admin_quality_channel_settings WHERE singleton = TRUE), FALSE)
              AND COALESCE(cc.name, '') !~ '^[[:space:]]*[|]?[[:space:]]*[A-Za-z]{2,3}[[:space:]]*[|]'
            )
          )
    "#).bind(user_id).bind(candidate_ids).fetch_all(pool).await?)
}

pub(super) const GUIDE_DEFAULT_LIMIT: i64 = 40;
pub(super) const GUIDE_MAX_LIMIT: i64 = 100;

pub(super) async fn fetch_guide_categories(
    pool: &PgPool,
    user_id: Uuid,
    visible_channel_ids: &[Uuid],
    with_epg_only: bool,
) -> Result<Vec<GuideCategorySummaryResponse>> {
    if visible_channel_ids.is_empty() {
        return Ok(Vec::new());
    }

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
          AND c.id = ANY($4)
          AND (
            NOT $5
            OR EXISTS(
              SELECT 1
              FROM programs epg
              WHERE epg.user_id = c.user_id
                AND epg.channel_id = c.id
                AND epg.end_at > NOW() - ($2 * INTERVAL '1 hour')
                AND epg.start_at < NOW() + ($3 * INTERVAL '1 day')
            )
          )
        GROUP BY COALESCE(c.category_id::text, 'uncategorized'), COALESCE(cc.name, 'Uncategorized')
        ORDER BY live_now_count DESC, channel_count DESC, name ASC
        "#,
    )
    .bind(user_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .bind(visible_channel_ids)
    .bind(with_epg_only)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(map_guide_category_summary).collect())
}

pub(super) async fn fetch_guide_category_summary(
    pool: &PgPool,
    user_id: Uuid,
    category_id: &str,
    visible_channel_ids: &[Uuid],
    with_epg_only: bool,
) -> Result<Option<GuideCategorySummaryResponse>> {
    if visible_channel_ids.is_empty() {
        return Ok(None);
    }

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
          AND c.id = ANY($5)
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
          AND (
            NOT $6
            OR EXISTS(
              SELECT 1
              FROM programs epg
              WHERE epg.user_id = c.user_id
                AND epg.channel_id = c.id
                AND epg.end_at > NOW() - ($3 * INTERVAL '1 hour')
                AND epg.start_at < NOW() + ($4 * INTERVAL '1 day')
            )
          )
        GROUP BY COALESCE(c.category_id::text, 'uncategorized'), COALESCE(cc.name, 'Uncategorized')
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .bind(visible_channel_ids)
    .bind(with_epg_only)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(map_guide_category_summary))
}

pub(super) async fn fetch_guide_category_total_count(
    pool: &PgPool,
    user_id: Uuid,
    category_id: &str,
    visible_channel_ids: &[Uuid],
    with_epg_only: bool,
) -> Result<i64> {
    if visible_channel_ids.is_empty() {
        return Ok(0);
    }

    let total_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM channels c
        WHERE c.user_id = $1
          AND c.id = ANY($3)
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
          AND (
            NOT $4
            OR EXISTS(
              SELECT 1
              FROM programs epg
              WHERE epg.user_id = c.user_id
                AND epg.channel_id = c.id
                AND epg.end_at > NOW() - ($5 * INTERVAL '1 hour')
                AND epg.start_at < NOW() + ($6 * INTERVAL '1 day')
            )
          )
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(visible_channel_ids)
    .bind(with_epg_only)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
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
    visible_channel_ids: &[Uuid],
    with_epg_only: bool,
) -> Result<Vec<GuideCategoryEntryRow>> {
    if visible_channel_ids.is_empty() {
        return Ok(Vec::new());
    }

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
          c.search_is_ppv AS is_ppv,
          EXISTS(
            SELECT 1 FROM favorite_ppv_channels fpc
            WHERE fpc.user_id = c.user_id AND fpc.channel_id = c.id
          ) AS is_ppv_favorite,
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
          AND c.id = ANY($7)
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
          AND (
            NOT $8
            OR p.id IS NOT NULL
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
    .bind(visible_channel_ids)
    .bind(with_epg_only)
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
            is_ppv: row.is_ppv,
            is_ppv_favorite: row.is_ppv_favorite,
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
    pub(super) is_ppv: bool,
    pub(super) is_ppv_favorite: bool,
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
    pub(super) is_ppv: bool,
    pub(super) is_ppv_favorite: bool,
    pub(super) program_id: Option<Uuid>,
    pub(super) program_channel_id: Option<Uuid>,
    pub(super) program_channel_name: Option<String>,
    pub(super) program_title: Option<String>,
    pub(super) program_description: Option<String>,
    pub(super) program_start_at: Option<DateTime<Utc>>,
    pub(super) program_end_at: Option<DateTime<Utc>>,
    pub(super) program_can_catchup: Option<bool>,
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;
