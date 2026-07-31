use super::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveFavoriteOrderPayload {
    pub(super) category_ids: Vec<Uuid>,
    pub(super) channel_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SavePpvFavoriteOrderPayload {
    pub(super) channel_ids: Vec<Uuid>,
}

pub(super) async fn list_favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
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

    let payload = category_favorites
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
                            is_ppv: row.is_ppv,
                            is_ppv_favorite: row.is_ppv_favorite,
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
        .collect::<Vec<_>>();

    json_response_with_revalidation(&headers, &payload)
}

pub(super) async fn list_ppv_favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let request_base_url = request_base_url(&state.config, &headers)?;
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
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite,
          c.search_is_ppv AS is_ppv,
          TRUE AS is_ppv_favorite,
          p.id AS program_id,
          p.channel_id AS program_channel_id,
          p.channel_name AS program_channel_name,
          p.title AS program_title,
          p.description AS program_description,
          p.start_at AS program_start_at,
          p.end_at AS program_end_at,
          p.can_catchup AS program_can_catchup,
          fpc.sort_order
        FROM favorite_ppv_channels fpc
        JOIN channels c ON c.id = fpc.channel_id
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
        WHERE fpc.user_id = $1
        ORDER BY
          fpc.sort_order ASC,
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

    let payload = favorites
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
                    is_ppv: row.is_ppv,
                    is_ppv_favorite: row.is_ppv_favorite,
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
        .collect::<Result<Vec<_>, _>>()?;

    json_response_with_revalidation(&headers, &payload)
}

pub(super) async fn save_favorite_order(
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

pub(super) async fn save_ppv_favorite_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SavePpvFavoriteOrderPayload>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let channel_ids = normalize_uuid_ids(payload.channel_ids);

    let mut transaction = state.pool.begin().await?;

    validate_ppv_favorite_channel_ids(&mut transaction, auth.user_id, &channel_ids).await?;
    replace_ppv_favorite_channel_order(&mut transaction, auth.user_id, &channel_ids).await?;

    transaction.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn add_favorite(
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

pub(super) async fn remove_favorite(
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

pub(super) async fn add_ppv_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let sort_order = next_ppv_favorite_channel_sort_order(&state.pool, auth.user_id).await?;
    sqlx::query(
        r#"
        INSERT INTO favorite_ppv_channels (user_id, channel_id, sort_order)
        SELECT $1, c.id, $3
        FROM channels c
        WHERE c.user_id = $1
          AND c.id = $2
          AND (
            c.search_is_ppv = TRUE
            OR EXISTS(
              SELECT 1
              FROM programs p
              WHERE p.user_id = c.user_id
                AND p.channel_id = c.id
                AND p.search_is_ppv = TRUE
                AND p.end_at > NOW() - ($4 * INTERVAL '1 hour')
                AND p.start_at < NOW() + ($5 * INTERVAL '1 day')
            )
          )
        ON CONFLICT (user_id, channel_id) DO NOTHING
        "#,
    )
    .bind(auth.user_id)
    .bind(channel_id)
    .bind(sort_order)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn remove_ppv_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    sqlx::query("DELETE FROM favorite_ppv_channels WHERE user_id = $1 AND channel_id = $2")
        .bind(auth.user_id)
        .bind(channel_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn add_category_favorite(
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

pub(super) async fn remove_category_favorite(
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

async fn next_ppv_favorite_channel_sort_order(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<i32, AppError> {
    Ok(sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM favorite_ppv_channels WHERE user_id = $1",
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

async fn validate_ppv_favorite_channel_ids(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    let existing = sqlx::query_scalar::<_, Uuid>(
        "SELECT channel_id FROM favorite_ppv_channels WHERE user_id = $1 ORDER BY sort_order ASC",
    )
    .bind(user_id)
    .fetch_all(transaction.as_mut())
    .await?;

    if existing.len() != ids.len()
        || existing.iter().copied().collect::<HashSet<_>>()
            != ids.iter().copied().collect::<HashSet<_>>()
    {
        return Err(AppError::BadRequest(
            "PPV favorite order must include every PPV favorite channel exactly once.".to_string(),
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

async fn replace_ppv_favorite_channel_order(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<(), AppError> {
    for (index, id) in ids.iter().enumerate() {
        sqlx::query(
            "UPDATE favorite_ppv_channels SET sort_order = $3 WHERE user_id = $1 AND channel_id = $2",
        )
        .bind(user_id)
        .bind(id)
        .bind(index as i32)
        .execute(transaction.as_mut())
        .await?;
    }

    Ok(())
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
    is_ppv: bool,
    is_ppv_favorite: bool,
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
