use super::rules;
use super::*;

#[derive(Debug, FromRow)]
struct ChannelSearchBuildRow {
    id: Uuid,
    name: String,
    category_name: Option<String>,
    search_provider_name: Option<String>,
    search_is_ppv: bool,
    search_is_vip: bool,
    has_catchup: bool,
    epg_channel_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct ProgramSearchBuildRow {
    id: Uuid,
    channel_id: Option<Uuid>,
    channel_name: Option<String>,
    category_name: Option<String>,
    search_provider_name: Option<String>,
    search_is_ppv: bool,
    search_is_vip: bool,
    title: String,
    description: Option<String>,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct SearchDocumentInsertRow {
    entity_type: &'static str,
    entity_id: Uuid,
    title: String,
    subtitle: Option<String>,
    search_text: String,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
}

async fn load_channel_search_build_rows(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ChannelSearchBuildRow>> {
    sqlx::query_as::<_, ChannelSearchBuildRow>(
        r#"
        SELECT
          c.id,
          c.name,
          cc.name AS category_name,
          c.search_provider_name,
          c.search_is_ppv,
          c.search_is_vip,
          c.has_catchup,
          c.epg_channel_id
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

async fn load_program_search_build_rows(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ProgramSearchBuildRow>> {
    sqlx::query_as::<_, ProgramSearchBuildRow>(
        r#"
        SELECT
          p.id,
          p.channel_id,
          p.channel_name,
          cc.name AS category_name,
          p.search_provider_name,
          p.search_is_ppv,
          p.search_is_vip,
          p.title,
          p.description,
          p.start_at,
          p.end_at
        FROM programs p
        LEFT JOIN channels c ON c.id = p.channel_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE p.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

fn build_channel_search_document_row(row: &ChannelSearchBuildRow) -> SearchDocumentInsertRow {
    SearchDocumentInsertRow {
        entity_type: "channel",
        entity_id: row.id,
        title: row.name.clone(),
        subtitle: row.category_name.clone(),
        search_text: format!(
            "{} {} {} {} {} {} {}",
            row.name,
            row.category_name.as_deref().unwrap_or_default(),
            row.search_provider_name.as_deref().unwrap_or_default(),
            if row.search_is_ppv { "ppv" } else { "" },
            if row.search_is_vip { "vip" } else { "" },
            if row
                .epg_channel_id
                .as_deref()
                .is_some_and(|value| !value.is_empty())
            {
                "epg"
            } else {
                ""
            },
            if row.has_catchup {
                "catchup archive"
            } else {
                "live"
            }
        )
        .trim()
        .to_string(),
        starts_at: None,
        ends_at: None,
    }
}

fn build_program_search_document_row(row: &ProgramSearchBuildRow) -> SearchDocumentInsertRow {
    SearchDocumentInsertRow {
        entity_type: "program",
        entity_id: row.id,
        title: row.title.clone(),
        subtitle: row.channel_name.clone(),
        search_text: format!(
            "{} {} {} {} {} {}",
            row.title,
            row.channel_name.as_deref().unwrap_or_default(),
            row.description.as_deref().unwrap_or_default(),
            row.search_provider_name.as_deref().unwrap_or_default(),
            if row.search_is_ppv { "ppv" } else { "" },
            if row.search_is_vip { "vip" } else { "" }
        )
        .trim()
        .to_string(),
        starts_at: Some(row.start_at),
        ends_at: Some(row.end_at),
    }
}

pub(in crate::server_main) async fn refresh_search_metadata(
    state: &AppState,
    user_id: Uuid,
) -> Result<()> {
    let started_at = Instant::now();
    let pool = &state.pool;
    let compiled_rules = rules::load_compiled_rules(pool).await?;
    let channel_rows = load_channel_search_build_rows(pool, user_id).await?;
    let channel_event_titles = load_channel_event_titles(pool, user_id, None).await?;
    let mut channel_metadata = HashMap::<Uuid, rules::EvaluatedAdminMetadata>::new();
    let mut channel_updates = Vec::with_capacity(channel_rows.len());

    for row in &channel_rows {
        let event_titles = channel_event_titles
            .get(&row.id)
            .cloned()
            .unwrap_or_default();
        let metadata = rules::evaluate_patterns(
            &compiled_rules,
            rules::AdminSearchEvaluationInput {
                channel_name: Some(&row.name),
                category_name: row.category_name.as_deref(),
                program_title: event_titles.first().map(String::as_str),
            },
        );

        channel_metadata.insert(row.id, metadata.clone());
        channel_updates.push((
            row.id,
            metadata.country_code,
            metadata.provider_name,
            metadata.is_ppv,
            metadata.is_vip,
        ));
    }

    apply_channel_search_metadata_updates(pool, &channel_updates).await?;

    let program_rows = load_program_search_build_rows(pool, user_id).await?;
    let mut program_updates = Vec::with_capacity(program_rows.len());
    for row in &program_rows {
        let metadata = row
            .channel_id
            .and_then(|channel_id| channel_metadata.get(&channel_id).cloned())
            .unwrap_or_else(|| {
                rules::evaluate_patterns(
                    &compiled_rules,
                    rules::AdminSearchEvaluationInput {
                        channel_name: row.channel_name.as_deref(),
                        category_name: row.category_name.as_deref(),
                        program_title: Some(&row.title),
                    },
                )
            });

        program_updates.push((
            row.id,
            metadata.country_code,
            metadata.provider_name,
            metadata.is_ppv,
            metadata.is_vip,
        ));
    }

    apply_program_search_metadata_updates(pool, &program_updates).await?;

    info!(
        user_id = %user_id,
        channel_rows = channel_rows.len(),
        program_rows = program_rows.len(),
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "refreshed PostgreSQL search metadata"
    );
    Ok(())
}

pub(in crate::server_main) async fn rebuild_postgres_search_documents(
    state: &AppState,
    user_id: Uuid,
) -> Result<()> {
    let started_at = Instant::now();
    let pool = &state.pool;
    let visibility = load_channel_visibility_map(state, user_id, None).await?;
    let visible_channel_ids = visible_channel_ids_from_map(&visibility);
    let channel_rows = load_channel_search_build_rows(pool, user_id).await?;
    let channel_documents = channel_rows
        .iter()
        .filter(|row| visible_channel_ids.contains(&row.id))
        .map(build_channel_search_document_row)
        .collect::<Vec<_>>();
    let program_rows = load_program_search_build_rows(pool, user_id).await?;
    let program_documents = program_rows
        .iter()
        .filter(|row| {
            row.channel_id
                .map(|channel_id| visible_channel_ids.contains(&channel_id))
                .unwrap_or(true)
        })
        .map(build_program_search_document_row)
        .collect::<Vec<_>>();

    let mut document_rows = Vec::with_capacity(channel_documents.len() + program_documents.len());
    document_rows.extend(channel_documents.iter().cloned());
    document_rows.extend(program_documents.iter().cloned());

    let mut transaction = pool.begin().await?;
    sqlx::query(
        r#"
        DELETE FROM search_documents
        WHERE user_id = $1
          AND entity_type IN ('channel', 'program')
        "#,
    )
    .bind(user_id)
    .execute(&mut *transaction)
    .await?;
    insert_search_documents(&mut transaction, user_id, &document_rows).await?;
    transaction.commit().await?;

    info!(
        user_id = %user_id,
        channel_documents = channel_documents.len(),
        program_documents = program_documents.len(),
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "rebuilt PostgreSQL fallback search documents"
    );
    Ok(())
}

pub(in crate::server_main) async fn rebuild_search_documents(
    state: &AppState,
    user_id: Uuid,
) -> Result<()> {
    refresh_search_metadata(state, user_id).await?;
    rebuild_postgres_search_documents(state, user_id).await
}

async fn apply_channel_search_metadata_updates(
    pool: &PgPool,
    updates: &[(Uuid, Option<String>, Option<String>, bool, bool)],
) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    for chunk in updates.chunks(1000) {
        let mut query = QueryBuilder::<Postgres>::new(
            "UPDATE channels AS c SET \
             search_country_code = v.search_country_code, \
             search_provider_name = v.search_provider_name, \
             search_is_ppv = v.search_is_ppv, \
             search_is_vip = v.search_is_vip \
             FROM (",
        );
        query.push_values(chunk, |mut builder, row| {
            builder
                .push_bind(row.0)
                .push_bind(row.1.clone())
                .push_bind(row.2.clone())
                .push_bind(row.3)
                .push_bind(row.4);
        });
        query.push(
            ") AS v(id, search_country_code, search_provider_name, search_is_ppv, search_is_vip) \
             WHERE c.id = v.id",
        );
        query.build().execute(pool).await?;
    }

    Ok(())
}

async fn apply_program_search_metadata_updates(
    pool: &PgPool,
    updates: &[(Uuid, Option<String>, Option<String>, bool, bool)],
) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    for chunk in updates.chunks(1000) {
        let mut query = QueryBuilder::<Postgres>::new(
            "UPDATE programs AS p SET \
             search_country_code = v.search_country_code, \
             search_provider_name = v.search_provider_name, \
             search_is_ppv = v.search_is_ppv, \
             search_is_vip = v.search_is_vip \
             FROM (",
        );
        query.push_values(chunk, |mut builder, row| {
            builder
                .push_bind(row.0)
                .push_bind(row.1.clone())
                .push_bind(row.2.clone())
                .push_bind(row.3)
                .push_bind(row.4);
        });
        query.push(
            ") AS v(id, search_country_code, search_provider_name, search_is_ppv, search_is_vip) \
             WHERE p.id = v.id",
        );
        query.build().execute(pool).await?;
    }

    Ok(())
}

async fn insert_search_documents(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    rows: &[SearchDocumentInsertRow],
) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }

    for chunk in rows.chunks(1000) {
        let mut query = QueryBuilder::<Postgres>::new(
            "INSERT INTO search_documents \
             (user_id, entity_type, entity_id, title, subtitle, search_text, starts_at, ends_at) ",
        );
        query.push_values(chunk, |mut builder, row| {
            builder
                .push_bind(user_id)
                .push_bind(row.entity_type)
                .push_bind(row.entity_id)
                .push_bind(&row.title)
                .push_bind(&row.subtitle)
                .push_bind(&row.search_text)
                .push_bind(row.starts_at)
                .push_bind(row.ends_at);
        });
        query.build().execute(&mut **transaction).await?;
    }

    Ok(())
}

async fn load_channel_event_titles(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Option<Uuid>,
) -> Result<HashMap<Uuid, Vec<String>>> {
    let rows = sqlx::query_as::<_, ChannelEventTitlesRow>(
        r#"
        WITH ranked AS (
          SELECT
            p.channel_id,
            p.title,
            ROW_NUMBER() OVER (
              PARTITION BY p.channel_id
              ORDER BY
                CASE
                  WHEN p.start_at <= NOW() AND p.end_at >= NOW() THEN 0
                  WHEN p.start_at > NOW() THEN 1
                  ELSE 2
                END,
                p.start_at ASC,
                p.title ASC
            ) AS rank
          FROM programs p
          WHERE p.user_id = $1
            AND p.channel_id IS NOT NULL
            AND ($2::uuid IS NULL OR p.profile_id = $2)
            AND p.end_at > NOW() - ($3 * INTERVAL '1 hour')
            AND p.start_at < NOW() + ($4 * INTERVAL '1 day')
        )
        SELECT channel_id, array_agg(title ORDER BY rank) AS titles
        FROM ranked
        WHERE rank <= 3
        GROUP BY channel_id
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.channel_id, row.titles))
        .collect::<HashMap<_, _>>())
}
