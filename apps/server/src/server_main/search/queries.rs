use super::*;

#[derive(Debug, FromRow)]
pub(in crate::server_main) struct ChannelSearchRow {
    pub(in crate::server_main) total_count: i64,
    pub(in crate::server_main) id: Uuid,
    pub(in crate::server_main) profile_id: Uuid,
    pub(in crate::server_main) name: String,
    pub(in crate::server_main) logo_url: Option<String>,
    pub(in crate::server_main) category_name: Option<String>,
    pub(in crate::server_main) remote_stream_id: i32,
    pub(in crate::server_main) epg_channel_id: Option<String>,
    pub(in crate::server_main) has_epg: bool,
    pub(in crate::server_main) has_catchup: bool,
    pub(in crate::server_main) archive_duration_hours: Option<i32>,
    pub(in crate::server_main) stream_extension: Option<String>,
    pub(in crate::server_main) is_favorite: bool,
}

#[derive(Debug, FromRow)]
pub(in crate::server_main) struct ProgramSearchRow {
    pub(in crate::server_main) total_count: i64,
    pub(in crate::server_main) id: Uuid,
    pub(in crate::server_main) channel_id: Option<Uuid>,
    pub(in crate::server_main) channel_name: Option<String>,
    pub(in crate::server_main) title: String,
    pub(in crate::server_main) description: Option<String>,
    pub(in crate::server_main) start_at: DateTime<Utc>,
    pub(in crate::server_main) end_at: DateTime<Utc>,
    pub(in crate::server_main) can_catchup: bool,
}

pub(in crate::server_main) async fn search_channels_postgres(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    term: &str,
    offset: i64,
    limit: i64,
    visible_channel_ids: &[Uuid],
) -> Result<ChannelSearchResponse, AppError> {
    if visible_channel_ids.is_empty() {
        return Ok(ChannelSearchResponse {
            query: term.to_string(),
            backend: "postgres".to_string(),
            next_offset: None,
            total_count: 0,
            items: Vec::new(),
        });
    }

    let rows = sqlx::query_as::<_, ChannelSearchRow>(
        r#"
        WITH matches AS (
          SELECT
            sd.entity_id,
            COUNT(*) OVER () AS total_count,
            ROW_NUMBER() OVER (
              ORDER BY
                CASE WHEN lower(sd.title) = lower($2) THEN 0 ELSE 1 END,
                similarity(sd.search_text, $2) DESC,
                sd.title ASC
            ) AS ordinal
          FROM search_documents sd
          JOIN channels c ON c.id = sd.entity_id
          WHERE sd.user_id = $1
            AND sd.entity_type = 'channel'
            AND c.id = ANY($5)
            AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
        ),
        page AS (
          SELECT entity_id, total_count, ordinal
          FROM matches
          WHERE ordinal > $3
          ORDER BY ordinal
          LIMIT $4
        )
        SELECT
          page.total_count,
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
              AND p.end_at > NOW() - ($6 * INTERVAL '1 hour')
              AND p.start_at < NOW() + ($7 * INTERVAL '1 day')
          ) AS has_epg,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite
        FROM page
        JOIN channels c ON c.id = page.entity_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        ORDER BY page.ordinal
        "#,
    )
    .bind(user_id)
    .bind(term)
    .bind(offset)
    .bind(limit)
    .bind(visible_channel_ids)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(&state.pool)
    .await?;
    let total_count = rows.first().map(|row| row.total_count).unwrap_or(0);
    let mut items = rows
        .into_iter()
        .map(|row| ChannelResponse {
            id: row.id,
            profile_id: row.profile_id,
            name: row.name,
            logo_url: row.logo_url,
            category_name: row.category_name,
            remote_stream_id: row.remote_stream_id,
            epg_channel_id: row.epg_channel_id,
            has_epg: row.has_epg,
            has_catchup: row.has_catchup,
            archive_duration_hours: row.archive_duration_hours,
            stream_extension: row.stream_extension,
            is_favorite: row.is_favorite,
        })
        .collect::<Vec<_>>();
    rewrite_channel_logo_urls(state, headers, user_id, &mut items)?;

    Ok(ChannelSearchResponse {
        query: term.to_string(),
        backend: "postgres".to_string(),
        next_offset: next_page_offset(offset, limit, total_count),
        total_count,
        items,
    })
}

pub(in crate::server_main) async fn search_programs_postgres(
    pool: &PgPool,
    user_id: Uuid,
    term: &str,
    offset: i64,
    limit: i64,
    visible_channel_ids: &[Uuid],
) -> Result<ProgramSearchResponse, AppError> {
    let rows = sqlx::query_as::<_, ProgramSearchRow>(
        r#"
        WITH matches AS (
          SELECT
            p.id,
            COUNT(*) OVER () AS total_count,
            ROW_NUMBER() OVER (
              ORDER BY
                CASE
                  WHEN p.channel_id IS NOT NULL AND sd.starts_at <= NOW() AND sd.ends_at >= NOW() THEN 0
                  WHEN p.channel_id IS NOT NULL AND sd.ends_at <= NOW() AND p.can_catchup THEN 1
                  WHEN lower(sd.title) = lower($2) THEN 2
                  WHEN lower(sd.title) LIKE lower($2 || '%') THEN 3
                  WHEN sd.starts_at > NOW() THEN 4
                  ELSE 5
                END,
                similarity(sd.search_text, $2) DESC,
                sd.starts_at ASC
            ) AS ordinal
          FROM search_documents sd
          JOIN programs p ON p.id = sd.entity_id
          WHERE sd.user_id = $1
            AND sd.entity_type = 'program'
            AND (
              p.channel_id IS NULL
              OR p.channel_id = ANY($5)
            )
            AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
        ),
        page AS (
          SELECT id, total_count, ordinal
          FROM matches
          WHERE ordinal > $3
          ORDER BY ordinal
          LIMIT $4
        )
        SELECT
          page.total_count,
          p.id,
          p.channel_id,
          p.channel_name,
          p.title,
          p.description,
          p.start_at,
          p.end_at,
          p.can_catchup
        FROM page
        JOIN programs p ON p.id = page.id
        ORDER BY page.ordinal
        "#,
    )
    .bind(user_id)
    .bind(term)
    .bind(offset)
    .bind(limit)
    .bind(visible_channel_ids)
    .fetch_all(pool)
    .await?;
    let total_count = rows.first().map(|row| row.total_count).unwrap_or(0);
    let items = rows
        .into_iter()
        .map(|row| ProgramResponse {
            id: row.id,
            channel_id: row.channel_id,
            channel_name: row.channel_name,
            title: row.title,
            description: row.description,
            start_at: row.start_at,
            end_at: row.end_at,
            can_catchup: row.can_catchup,
        })
        .collect();

    Ok(ProgramSearchResponse {
        query: term.to_string(),
        backend: "postgres".to_string(),
        next_offset: next_page_offset(offset, limit, total_count),
        total_count,
        items,
    })
}
