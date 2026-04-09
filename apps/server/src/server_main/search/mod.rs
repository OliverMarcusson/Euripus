use self::queries::{search_channels_postgres, search_programs_postgres};
use super::*;

pub(super) mod indexing;
pub(super) mod lexicon;
pub(super) mod queries;

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/search/status", get(get_search_backend_status))
        .route("/search/channels", get(search_channels))
        .route("/search/programs", get(search_programs))
}

async fn get_search_backend_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<SearchBackendStatusResponse> {
    let auth = require_auth(&state, &headers).await?;
    let Some(meili) = state.meili.as_ref() else {
        return Ok(Json(SearchBackendStatusResponse {
            meilisearch: MeiliReadiness::Disabled.search_status().to_string(),
            progress_percent: None,
            indexed_documents: None,
            total_documents: None,
        }));
    };

    let counts =
        match indexing::load_search_index_counts_for_user(meili, &state.pool, auth.user_id).await {
            Ok(counts) => counts,
            Err(error) => {
                warn!(
                    user_id = %auth.user_id,
                    "failed to load Meilisearch progress for search status: {error:?}"
                );
                let readiness = *state.meili_readiness.read().await;
                return Ok(Json(SearchBackendStatusResponse {
                    meilisearch: readiness.search_status().to_string(),
                    progress_percent: None,
                    indexed_documents: None,
                    total_documents: None,
                }));
            }
        };

    let total_documents = counts.postgres_channel_documents + counts.postgres_program_documents;
    let indexed_documents =
        (counts.meili_channel_documents + counts.meili_program_documents).clamp(0, total_documents);
    let progress_percent = if total_documents > 0 {
        Some(((indexed_documents * 100) / total_documents) as i32)
    } else {
        None
    };
    let schema_ready = *state.meili_schema_ready.read().await;

    Ok(Json(SearchBackendStatusResponse {
        meilisearch: indexing::determine_meili_readiness(counts, schema_ready)
            .search_status()
            .to_string(),
        progress_percent,
        indexed_documents: Some(indexed_documents),
        total_documents: Some(total_documents),
    }))
}

async fn search_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> ApiResult<ChannelSearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let (term, offset, limit) = parse_search_pagination(query)?;
    let visibility = load_channel_visibility_map(&state.pool, auth.user_id, None).await?;
    let visible_channel_ids = visible_channel_ids_from_map(&visibility);
    let visible_channel_set = visible_channel_ids.iter().copied().collect::<HashSet<_>>();
    if indexing::meili_is_ready_for_user(&state, auth.user_id).await {
        let meili = state
            .meili
            .as_ref()
            .expect("Meilisearch client must exist when ready");
        match lexicon::get_search_lexicon(&state, auth.user_id).await {
            Ok(lexicon) => {
                match search_channels_meili(
                    &state,
                    &headers,
                    meili,
                    auth.user_id,
                    &term,
                    offset,
                    limit,
                    lexicon.as_ref(),
                    &visible_channel_set,
                )
                .await
                {
                    Ok(response) => return Ok(Json(response)),
                    Err(error) => {
                        warn!(
                            "Meilisearch channel search failed, falling back to PostgreSQL: {error:?}"
                        )
                    }
                }
            }
            Err(error) => {
                warn!("failed to load search lexicon, falling back to PostgreSQL search: {error:?}")
            }
        }
    }

    Ok(Json(
        search_channels_postgres(
            &state,
            &headers,
            auth.user_id,
            &term,
            offset,
            limit,
            &visible_channel_ids,
        )
        .await?,
    ))
}

async fn search_programs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> ApiResult<ProgramSearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let (term, offset, limit) = parse_search_pagination(query)?;
    let visibility = load_channel_visibility_map(&state.pool, auth.user_id, None).await?;
    let visible_channel_ids = visible_channel_ids_from_map(&visibility);
    let visible_channel_set = visible_channel_ids.iter().copied().collect::<HashSet<_>>();
    if indexing::meili_is_ready_for_user(&state, auth.user_id).await {
        let meili = state
            .meili
            .as_ref()
            .expect("Meilisearch client must exist when ready");
        match lexicon::get_search_lexicon(&state, auth.user_id).await {
            Ok(lexicon) => {
                match search_programs_meili(
                    meili,
                    &state.pool,
                    auth.user_id,
                    &term,
                    offset,
                    limit,
                    lexicon.as_ref(),
                    &visible_channel_set,
                )
                .await
                {
                    Ok(response) => return Ok(Json(response)),
                    Err(error) => {
                        warn!(
                            "Meilisearch program search failed, falling back to PostgreSQL: {error:?}"
                        )
                    }
                }
            }
            Err(error) => {
                warn!("failed to load search lexicon, falling back to PostgreSQL search: {error:?}")
            }
        }
    }

    Ok(Json(
        search_programs_postgres(
            &state.pool,
            auth.user_id,
            &term,
            offset,
            limit,
            &visible_channel_ids,
        )
        .await?,
    ))
}

fn parse_search_pagination(query: SearchQuery) -> Result<(String, i64, i64), AppError> {
    let term = query.q.trim().to_string();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(SEARCH_DEFAULT_LIMIT);

    if term.len() < 2 {
        return Err(AppError::BadRequest(
            "Search query must be at least 2 characters".to_string(),
        ));
    }

    if offset < 0 {
        return Err(AppError::BadRequest(
            "Search offset must be zero or greater".to_string(),
        ));
    }

    if limit <= 0 {
        return Err(AppError::BadRequest(
            "Search limit must be greater than zero".to_string(),
        ));
    }

    Ok((term, offset, limit.min(SEARCH_MAX_LIMIT)))
}

async fn execute_meili_channel_search(
    meili: &MeilisearchClient,
    user_id: Uuid,
    parsed: &lexicon::ParsedSearch,
    query: &str,
    limit: usize,
    offset: usize,
    apply_sort: bool,
) -> std::result::Result<SearchResults<MeiliChannelDoc>, AppError> {
    let base_filter = lexicon::build_meili_search_filter(user_id, parsed.filter.as_deref());
    let filter = format!("({base_filter}) AND is_hidden = false");
    let index = meili.index("channels");
    let mut search = index.search();
    search
        .with_query(query)
        .with_matching_strategy(MatchingStrategies::FREQUENCY)
        .with_filter(&filter)
        .with_offset(offset)
        .with_limit(limit);
    if apply_sort {
        search.with_sort(&["sort_rank:asc", "channel_name:asc"]);
    }
    search
        .execute::<MeiliChannelDoc>()
        .await
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

async fn search_channels_meili(
    state: &AppState,
    headers: &HeaderMap,
    meili: &MeilisearchClient,
    user_id: Uuid,
    query: &str,
    offset: i64,
    limit: i64,
    lexicon: &lexicon::SearchLexicon,
    visible_channel_ids: &HashSet<Uuid>,
) -> std::result::Result<ChannelSearchResponse, AppError> {
    let parsed = lexicon::parse_search_query(query, lexicon);
    if parsed.search.is_empty() {
        let results = execute_meili_channel_search(
            meili,
            user_id,
            &parsed,
            "",
            limit as usize,
            offset as usize,
            true,
        )
        .await?;
        let ids = results
            .hits
            .iter()
            .map(|hit| {
                Uuid::parse_str(&hit.result.entity_id)
                    .map_err(|error| AppError::Internal(anyhow!(error)))
            })
            .filter(|result| {
                result
                    .as_ref()
                    .map(|id| visible_channel_ids.contains(id))
                    .unwrap_or(true)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let total_count = ids.len() as i64;
        let mut items = load_channels_by_ids(&state.pool, &ids, user_id)
            .await
            .map_err(AppError::from)?;
        rewrite_channel_logo_urls(state, headers, user_id, &mut items)?;

        return Ok(ChannelSearchResponse {
            query: query.to_string(),
            backend: "meilisearch".to_string(),
            next_offset: next_page_offset(offset, limit, total_count),
            total_count,
            items,
        });
    }

    let primary_limit = lexicon::meili_channel_primary_limit(offset, limit);
    let primary_results = execute_meili_channel_search(
        meili,
        user_id,
        &parsed,
        &parsed.search,
        primary_limit,
        0,
        false,
    )
    .await?;
    let significant_terms = lexicon::extract_significant_search_terms(&parsed.search);
    let missing_terms = significant_terms
        .into_iter()
        .filter(|term| {
            !primary_results
                .hits
                .iter()
                .any(|hit| indexing::channel_doc_contains_term(&hit.result, term))
        })
        .collect::<Vec<_>>();

    let supplement_limit = (limit as usize).clamp(5, 15);
    let mut ordered_entity_ids = Vec::new();
    let mut seen_entity_ids = HashSet::new();

    for hit in &primary_results.hits {
        if seen_entity_ids.insert(hit.result.entity_id.clone()) {
            ordered_entity_ids.push(
                Uuid::parse_str(&hit.result.entity_id)
                    .map_err(|error| AppError::Internal(anyhow!(error)))?,
            );
        }
    }

    if !missing_terms.is_empty() {
        let mut supplemental_hits = Vec::with_capacity(missing_terms.len());
        for term in missing_terms {
            supplemental_hits.push(
                execute_meili_channel_search(
                    meili,
                    user_id,
                    &parsed,
                    &term,
                    supplement_limit,
                    0,
                    false,
                )
                .await?
                .hits,
            );
        }

        let mut index = 0;
        loop {
            let mut had_candidates = false;
            for hits in &supplemental_hits {
                let Some(hit) = hits.get(index) else {
                    continue;
                };
                had_candidates = true;
                if seen_entity_ids.insert(hit.result.entity_id.clone()) {
                    ordered_entity_ids.push(
                        Uuid::parse_str(&hit.result.entity_id)
                            .map_err(|error| AppError::Internal(anyhow!(error)))?,
                    );
                }
            }
            if !had_candidates {
                break;
            }
            index += 1;
        }
    }

    let ordered_entity_ids = ordered_entity_ids
        .into_iter()
        .filter(|id| visible_channel_ids.contains(id))
        .collect::<Vec<_>>();
    let total_count = ordered_entity_ids.len() as i64;
    let page_ids = ordered_entity_ids
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect::<Vec<_>>();

    let mut items = load_channels_by_ids(&state.pool, &page_ids, user_id)
        .await
        .map_err(AppError::from)?;
    rewrite_channel_logo_urls(state, headers, user_id, &mut items)?;

    Ok(ChannelSearchResponse {
        query: query.to_string(),
        backend: "meilisearch".to_string(),
        next_offset: next_page_offset(offset, limit, total_count),
        total_count,
        items,
    })
}

async fn search_programs_meili(
    meili: &MeilisearchClient,
    pool: &PgPool,
    user_id: Uuid,
    query: &str,
    offset: i64,
    limit: i64,
    lexicon: &lexicon::SearchLexicon,
    visible_channel_ids: &HashSet<Uuid>,
) -> std::result::Result<ProgramSearchResponse, AppError> {
    let parsed = lexicon::parse_search_query(query, lexicon);
    let base_filter = lexicon::build_meili_search_filter(user_id, parsed.filter.as_deref());
    let filter = format!("({base_filter}) AND is_hidden = false");
    let results = meili
        .index("programs")
        .search()
        .with_query(&parsed.search)
        .with_matching_strategy(MatchingStrategies::FREQUENCY)
        .with_filter(&filter)
        .with_sort(&["sort_priority:asc", "starts_at:asc", "ends_at:asc"])
        .with_offset(offset as usize)
        .with_limit(limit as usize)
        .execute::<MeiliProgramDoc>()
        .await
        .map_err(|error| AppError::Internal(anyhow!(error)))?;

    let ids = results
        .hits
        .iter()
        .map(|hit| {
            Uuid::parse_str(&hit.result.entity_id)
                .map_err(|error| AppError::Internal(anyhow!(error)))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let items = load_programs_by_ids(pool, user_id, &ids)
        .await
        .map_err(AppError::from)?
        .into_iter()
        .filter(|program| {
            program
                .channel_id
                .map(|channel_id| visible_channel_ids.contains(&channel_id))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let total_count = items.len() as i64;

    Ok(ProgramSearchResponse {
        query: query.to_string(),
        backend: "meilisearch".to_string(),
        next_offset: next_page_offset(offset, limit, total_count),
        total_count,
        items,
    })
}

fn next_page_offset(offset: i64, limit: i64, total_count: i64) -> Option<i64> {
    let next_offset = offset + limit;
    (next_offset < total_count).then_some(next_offset)
}

async fn load_channels_by_ids(
    pool: &PgPool,
    ids: &[Uuid],
    user_id: Uuid,
) -> Result<Vec<ChannelResponse>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, ChannelResponse>(
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
        WHERE c.user_id = $1
          AND c.id = ANY($2)
        "#,
    )
    .bind(user_id)
    .bind(ids)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(pool)
    .await?;

    let mut by_id = rows
        .into_iter()
        .map(|row| (row.id, row))
        .collect::<HashMap<_, _>>();
    let mut ordered = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(row) = by_id.remove(id) {
            ordered.push(row);
        }
    }

    Ok(ordered)
}

async fn load_programs_by_ids(
    pool: &PgPool,
    user_id: Uuid,
    ids: &[Uuid],
) -> Result<Vec<ProgramResponse>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, ProgramResponse>(
        r#"
        SELECT id, channel_id, channel_name, title, description, start_at, end_at, can_catchup
        FROM programs
        WHERE user_id = $1
          AND id = ANY($2)
        "#,
    )
    .bind(user_id)
    .bind(ids)
    .fetch_all(pool)
    .await?;

    let mut by_id = rows
        .into_iter()
        .map(|row| (row.id, row))
        .collect::<HashMap<_, _>>();
    let mut ordered = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(row) = by_id.remove(id) {
            ordered.push(row);
        }
    }

    Ok(ordered)
}
