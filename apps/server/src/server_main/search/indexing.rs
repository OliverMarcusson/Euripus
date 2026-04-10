use super::lexicon::SearchLexicon;
use super::rules;
use super::*;

#[derive(Debug, FromRow)]
struct ChannelSearchBuildRow {
    id: Uuid,
    name: String,
    category_name: Option<String>,
    has_catchup: bool,
    epg_channel_id: Option<String>,
}

#[derive(Debug, FromRow)]
struct ProgramSearchBuildRow {
    id: Uuid,
    channel_id: Option<Uuid>,
    channel_name: Option<String>,
    category_name: Option<String>,
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

pub(in crate::server_main) async fn rebuild_search_documents(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<()> {
    let visibility = load_channel_visibility_map(pool, user_id, None).await?;
    let visible_channel_ids = visible_channel_ids_from_map(&visibility);
    let compiled_rules = rules::load_compiled_rules(pool).await?;

    let channel_rows = sqlx::query_as::<_, ChannelSearchBuildRow>(
        r#"
        SELECT
          c.id,
          c.name,
          cc.name AS category_name,
          c.has_catchup,
          c.epg_channel_id
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let channel_event_titles = load_channel_event_titles(pool, user_id, None).await?;
    let mut channel_metadata = HashMap::<Uuid, rules::EvaluatedAdminMetadata>::new();
    let mut channel_updates = Vec::with_capacity(channel_rows.len());
    let mut document_rows = Vec::new();

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
        let search_text = format!(
            "{} {} {} {} {} {} {}",
            row.name,
            row.category_name.as_deref().unwrap_or_default(),
            metadata.provider_name.as_deref().unwrap_or_default(),
            if metadata.is_ppv { "ppv" } else { "" },
            if metadata.is_vip { "vip" } else { "" },
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
        .to_string();

        if visible_channel_ids.contains(&row.id) {
            document_rows.push(SearchDocumentInsertRow {
                entity_type: "channel",
                entity_id: row.id,
                title: row.name.clone(),
                subtitle: row.category_name.clone(),
                search_text,
                starts_at: None,
                ends_at: None,
            });
        }

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

    sqlx::query(
        r#"
        DELETE FROM search_documents
        WHERE user_id = $1
          AND entity_type IN ('channel', 'program')
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    let program_rows = sqlx::query_as::<_, ProgramSearchBuildRow>(
        r#"
        SELECT
          p.id,
          p.channel_id,
          p.channel_name,
          cc.name AS category_name,
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
    .await?;

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

        let search_text = format!(
            "{} {} {} {} {} {}",
            row.title,
            row.channel_name.as_deref().unwrap_or_default(),
            row.description.as_deref().unwrap_or_default(),
            metadata.provider_name.as_deref().unwrap_or_default(),
            if metadata.is_ppv { "ppv" } else { "" },
            if metadata.is_vip { "vip" } else { "" }
        )
        .trim()
        .to_string();

        if row
            .channel_id
            .map(|channel_id| visible_channel_ids.contains(&channel_id))
            .unwrap_or(true)
        {
            document_rows.push(SearchDocumentInsertRow {
                entity_type: "program",
                entity_id: row.id,
                title: row.title.clone(),
                subtitle: row.channel_name.clone(),
                search_text,
                starts_at: Some(row.start_at),
                ends_at: Some(row.end_at),
            });
        }

        program_updates.push((
            row.id,
            metadata.country_code,
            metadata.provider_name,
            metadata.is_ppv,
            metadata.is_vip,
        ));
    }

    apply_program_search_metadata_updates(pool, &program_updates).await?;
    insert_search_documents(pool, user_id, &document_rows).await?;

    Ok(())
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
    pool: &PgPool,
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
        query.build().execute(pool).await?;
    }

    Ok(())
}

pub(in crate::server_main) async fn inspect_meili_readiness(
    client: &MeilisearchClient,
    pool: &PgPool,
    schema_ready: bool,
) -> Result<MeiliReadiness> {
    let counts = load_search_index_counts(client, pool).await?;
    Ok(determine_meili_readiness(counts, schema_ready))
}

pub(in crate::server_main) async fn inspect_meili_readiness_for_user(
    client: &MeilisearchClient,
    pool: &PgPool,
    user_id: Uuid,
    schema_ready: bool,
) -> Result<MeiliReadiness> {
    let counts = load_search_index_counts_for_user(client, pool, user_id).await?;
    Ok(determine_meili_readiness(counts, schema_ready))
}

fn contains_normalized_phrase(text: &str, phrase: &str) -> bool {
    if text == phrase {
        return true;
    }
    text.starts_with(&format!("{phrase} "))
        || text.ends_with(&format!(" {phrase}"))
        || text.contains(&format!(" {phrase} "))
}

fn detect_provider_for_texts(
    lexicon: &lexicon::SearchLexicon,
    texts: &[&str],
) -> (Option<String>, Vec<String>) {
    let normalized_texts = texts
        .iter()
        .map(|text| lexicon::normalize_search_text(text))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();

    for alias in &lexicon.provider_aliases {
        let alias_phrase = alias.alias_tokens.join(" ");
        if normalized_texts
            .iter()
            .any(|text| contains_normalized_phrase(text, &alias_phrase))
        {
            let labels = lexicon
                .provider_labels
                .get(&alias.key)
                .cloned()
                .unwrap_or_else(|| vec![alias.alias.clone()]);
            return (Some(alias.key.clone()), labels);
        }
    }

    (None, Vec::new())
}

fn detect_provider_for_metadata(
    lexicon: &lexicon::SearchLexicon,
    texts: &[&str],
) -> (Option<String>, Vec<String>) {
    let mut best_match: Option<(usize, usize, String, Vec<String>)> = None;

    for text in texts {
        for candidate in lexicon::collect_provider_candidates(text, &lexicon.known_prefixes) {
            let token_count = candidate.split_whitespace().count();
            if token_count == 0 {
                continue;
            }

            let key = lexicon::provider_key_from_tokens(
                &candidate
                    .split_whitespace()
                    .map(|token| token.to_string())
                    .collect::<Vec<_>>(),
            );
            let Some(labels) = lexicon.provider_labels.get(&key) else {
                continue;
            };

            let candidate_len = candidate.len();
            let should_replace = best_match
                .as_ref()
                .map(|(best_token_count, best_len, _, _)| {
                    token_count > *best_token_count
                        || (token_count == *best_token_count && candidate_len > *best_len)
                })
                .unwrap_or(true);
            if should_replace {
                best_match = Some((token_count, candidate_len, key.clone(), labels.clone()));
            }
        }
    }

    if let Some((_, _, key, labels)) = best_match {
        return (Some(key), labels);
    }

    detect_provider_for_texts(lexicon, texts)
}

pub(in crate::server_main) fn derive_search_metadata(
    lexicon: &lexicon::SearchLexicon,
    channel_name: &str,
    category_name: Option<&str>,
    extra_event_titles: &[String],
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    bool,
    bool,
    i32,
) {
    let prefix = category_name
        .and_then(lexicon::extract_catalog_prefix)
        .or_else(|| lexicon::extract_catalog_prefix(channel_name));
    let country_code = prefix
        .as_ref()
        .filter(|prefix| lexicon.country_prefixes.contains(*prefix))
        .cloned();
    let region_code = prefix
        .as_ref()
        .filter(|prefix| lexicon.region_prefixes.contains(*prefix))
        .cloned();

    let mut provider_texts = vec![channel_name];
    if let Some(category_name) = category_name {
        provider_texts.push(category_name);
    }
    let (provider_key, provider_labels) = detect_provider_for_metadata(lexicon, &provider_texts);

    let mut texts = provider_texts;
    for title in extra_event_titles {
        texts.push(title);
    }

    let combined_text = texts.join(" ");
    let broad_categories = derive_broad_categories(&combined_text);
    let is_placeholder_channel = is_placeholder_channel_name(channel_name);
    let is_event_channel = detect_event_channel(channel_name)
        || extra_event_titles
            .iter()
            .any(|title| detect_event_channel(title));
    let event_keywords =
        derive_event_keywords(&format!("{channel_name} {}", extra_event_titles.join(" ")));

    let mut sort_rank = 2;
    if is_placeholder_channel {
        sort_rank = 5;
    } else if is_event_channel {
        sort_rank = 0;
    } else if broad_categories.iter().any(|category| category == "sports") {
        sort_rank = 1;
    }

    (
        country_code,
        region_code,
        provider_key,
        provider_labels,
        broad_categories,
        event_keywords,
        is_event_channel,
        is_placeholder_channel,
        sort_rank,
    )
}

fn derive_broad_categories(value: &str) -> Vec<String> {
    let normalized = lexicon::normalize_search_text(value);
    let mut categories = Vec::new();

    if contains_any_keyword(
        &normalized,
        &[
            "sport",
            "sports",
            "football",
            "soccer",
            "league",
            "champions",
            "nhl",
            "mlb",
            "nba",
            "nfl",
            "golf",
            "tennis",
            "formula 1",
            "f1",
            "ufc",
            "boxing",
            "pga",
            "atp",
            "wta",
            "cycling",
            "race",
            "racing",
            "masters",
        ],
    ) {
        push_unique(&mut categories, "sports");
    }
    if contains_any_keyword(
        &normalized,
        &[
            "news",
            "noticias",
            "noticiero",
            "journal",
            "journaux",
            "cnn",
        ],
    ) {
        push_unique(&mut categories, "news");
    }
    if contains_any_keyword(
        &normalized,
        &["movie", "movies", "cinema", "film", "films", "box office"],
    ) {
        push_unique(&mut categories, "movies");
    }
    if contains_any_keyword(
        &normalized,
        &["series", "drama", "show", "shows", "telenovela", "soap"],
    ) {
        push_unique(&mut categories, "series");
    }
    if contains_any_keyword(
        &normalized,
        &["kids", "cartoon", "junior", "nick", "disney jr", "children"],
    ) {
        push_unique(&mut categories, "kids");
    }
    if contains_any_keyword(&normalized, &["music", "mtv", "radio", "songs", "hits"]) {
        push_unique(&mut categories, "music");
    }
    if contains_any_keyword(
        &normalized,
        &[
            "documentary",
            "docu",
            "history",
            "discovery",
            "nature",
            "animal planet",
        ],
    ) {
        push_unique(&mut categories, "documentary");
    }
    if categories.is_empty()
        || contains_any_keyword(
            &normalized,
            &["general", "entertainment", "variety", "family"],
        )
    {
        push_unique(&mut categories, "general");
    }

    categories
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn derive_event_keywords(value: &str) -> Vec<String> {
    let mut keywords = lexicon::extract_significant_search_terms(value);
    let normalized = lexicon::normalize_search_text(value);
    for abbreviation in lexicon::short_search_abbreviations() {
        if contains_normalized_phrase(&normalized, abbreviation)
            && !keywords.iter().any(|keyword| keyword == abbreviation)
        {
            keywords.push((*abbreviation).to_string());
        }
    }
    keywords
}

fn contains_any_keyword(text: &str, keywords: &[&str]) -> bool {
    keywords
        .iter()
        .any(|keyword| contains_normalized_phrase(text, &lexicon::normalize_search_text(keyword)))
}

fn detect_event_channel(value: &str) -> bool {
    let normalized = lexicon::normalize_search_text(value);
    value.contains('@')
        || normalized.contains(" vs ")
        || value.contains(" - ")
        || contains_any_keyword(
            &normalized,
            &[
                "the masters",
                "premier league",
                "champions league",
                "formula 1",
                "f1",
                "nba",
                "nfl",
                "nhl",
                "mlb",
                "pga",
                "atp",
                "wta",
                "ufc",
                "boxing",
                "golf",
                "tennis",
            ],
        )
}

fn is_placeholder_channel_name(value: &str) -> bool {
    let normalized = lexicon::normalize_search_text(value);
    contains_any_keyword(
        &normalized,
        &["no event streaming", "event only", "streaming now"],
    )
}

pub(in crate::server_main) fn channel_doc_contains_term(doc: &MeiliChannelDoc, term: &str) -> bool {
    let term = term.to_ascii_lowercase();
    doc.channel_name.to_ascii_lowercase().contains(&term)
        || doc
            .subtitle
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(&term)
        || doc
            .category_name_raw
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(&term)
        || doc
            .provider_name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(&term)
        || doc
            .event_titles
            .iter()
            .any(|title| title.to_ascii_lowercase().contains(&term))
        || doc.search_text.to_ascii_lowercase().contains(&term)
}

pub(in crate::server_main) async fn load_search_index_counts(
    client: &MeilisearchClient,
    pool: &PgPool,
) -> Result<SearchIndexCounts> {
    let postgres_counts = sqlx::query_as::<_, SearchDocumentCountsRow>(
        r#"
        SELECT
          COUNT(*) FILTER (WHERE entity_type = 'channel') AS channel_documents,
          COUNT(*) FILTER (WHERE entity_type = 'program') AS program_documents
        FROM search_documents
        "#,
    )
    .fetch_one(pool)
    .await?;
    let channels_stats = client.index("channels").get_stats().await?;
    let programs_stats = client.index("programs").get_stats().await?;

    Ok(SearchIndexCounts {
        postgres_channel_documents: postgres_counts.channel_documents,
        postgres_program_documents: postgres_counts.program_documents,
        meili_channel_documents: channels_stats.number_of_documents as i64,
        meili_program_documents: programs_stats.number_of_documents as i64,
    })
}

pub(in crate::server_main) async fn load_search_index_counts_for_user(
    client: &MeilisearchClient,
    pool: &PgPool,
    user_id: Uuid,
) -> Result<SearchIndexCounts> {
    let postgres_counts = sqlx::query_as::<_, SearchDocumentCountsRow>(
        r#"
        SELECT
          COUNT(*) FILTER (WHERE entity_type = 'channel') AS channel_documents,
          COUNT(*) FILTER (WHERE entity_type = 'program') AS program_documents
        FROM search_documents
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let filter = format!("user_id = \"{user_id}\"");
    let channel_results = client
        .index("channels")
        .search()
        .with_query("")
        .with_filter(&filter)
        .with_limit(1)
        .execute::<MeiliChannelDoc>()
        .await?;
    let program_results = client
        .index("programs")
        .search()
        .with_query("")
        .with_filter(&filter)
        .with_limit(1)
        .execute::<MeiliProgramDoc>()
        .await?;

    Ok(SearchIndexCounts {
        postgres_channel_documents: postgres_counts.channel_documents,
        postgres_program_documents: postgres_counts.program_documents,
        meili_channel_documents: channel_results
            .estimated_total_hits
            .map(|value| value as i64)
            .unwrap_or(channel_results.hits.len() as i64),
        meili_program_documents: program_results
            .estimated_total_hits
            .map(|value| value as i64)
            .unwrap_or(program_results.hits.len() as i64),
    })
}

pub(in crate::server_main) fn determine_meili_readiness(
    counts: SearchIndexCounts,
    schema_ready: bool,
) -> MeiliReadiness {
    let postgres_documents = counts.postgres_channel_documents + counts.postgres_program_documents;
    if postgres_documents == 0 {
        return MeiliReadiness::Ready;
    }

    if !schema_ready {
        return MeiliReadiness::Bootstrapping;
    }

    if counts.postgres_channel_documents == counts.meili_channel_documents
        && counts.postgres_program_documents == counts.meili_program_documents
    {
        MeiliReadiness::Ready
    } else {
        MeiliReadiness::Bootstrapping
    }
}

pub(in crate::server_main) async fn inspect_meili_schema_readiness(
    client: &MeilisearchClient,
) -> Result<bool> {
    Ok(load_meili_schema_version(client, "channels").await?
        && load_meili_schema_version(client, "programs").await?)
}

pub(in crate::server_main) async fn load_meili_schema_version(
    client: &MeilisearchClient,
    index_name: &str,
) -> Result<bool> {
    let synonyms = client.index(index_name).get_synonyms().await?;
    Ok(synonyms
        .get(MEILI_SCHEMA_VERSION_KEY)
        .map(|values| values.iter().any(|value| value == MEILI_SCHEMA_VERSION))
        .unwrap_or(false))
}

pub(in crate::server_main) fn apply_meili_schema_version(
    mut synonyms: HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    synonyms.insert(
        MEILI_SCHEMA_VERSION_KEY.to_string(),
        vec![MEILI_SCHEMA_VERSION.to_string()],
    );
    synonyms
}

pub(in crate::server_main) fn spawn_meili_startup_worker(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Some(meili) = state.meili.clone() else {
            return;
        };

        info!("continuing Meilisearch startup in the background");
        let strategy = ExponentialBackoff::from_millis(500).factor(2).take(4);
        let setup_result = Retry::spawn(strategy, || {
            let client = meili.clone();
            let pool = state.pool.clone();
            async move { configure_meili_indexes(&client, &pool).await }
        })
        .await;

        match setup_result {
            Ok(()) => {
                *state.meili_schema_ready.write().await = true;
                match inspect_meili_readiness(&meili, &state.pool, true).await {
                    Ok(MeiliReadiness::Ready) => {
                        *state.meili_readiness.write().await = MeiliReadiness::Ready;
                        info!("Meilisearch background startup completed successfully");
                    }
                    Ok(MeiliReadiness::Bootstrapping) => {
                        *state.meili_readiness.write().await = MeiliReadiness::Bootstrapping;
                        warn!(
                            "Meilisearch schema is ready; continuing document bootstrap in the background"
                        );
                        spawn_meili_bootstrap_worker(state.clone());
                    }
                    Ok(MeiliReadiness::Disabled) => {
                        *state.meili_readiness.write().await = MeiliReadiness::Disabled;
                    }
                    Err(error) => {
                        warn!(
                            "failed to inspect Meilisearch readiness after background startup: {error:?}"
                        );
                    }
                }
            }
            Err(error) => {
                warn!(
                    "failed to finish Meilisearch startup in the background; PostgreSQL fallback remains active: {error:?}"
                );
            }
        }
    })
}

pub(in crate::server_main) fn spawn_meili_bootstrap_worker(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Some(meili) = state.meili.clone() else {
            return;
        };

        info!("starting Meilisearch bootstrap from PostgreSQL");
        let started_at = Instant::now();
        let user_ids = match load_meili_bootstrap_user_ids(&state.pool).await {
            Ok(user_ids) => user_ids,
            Err(error) => {
                warn!("failed to load Meilisearch bootstrap users: {error:?}");
                return;
            }
        };

        let mut success_count = 0usize;
        let mut failure_count = 0usize;
        for user_id in user_ids {
            state.meili_bootstrapping_users.insert(user_id);
            info!("bootstrapping Meilisearch documents for user {user_id}");
            let rebuild_result = rebuild_meili_indexes(&state, &meili, user_id, None).await;
            state.meili_bootstrapping_users.remove(&user_id);
            match rebuild_result {
                Ok(()) => {
                    success_count += 1;
                }
                Err(error) => {
                    failure_count += 1;
                    warn!(
                        "failed to bootstrap Meilisearch documents for user {user_id}: {error:?}"
                    );
                }
            }
        }

        match inspect_meili_readiness(&meili, &state.pool, true).await {
            Ok(MeiliReadiness::Ready) => {
                *state.meili_schema_ready.write().await = true;
                *state.meili_readiness.write().await = MeiliReadiness::Ready;
                info!(
                    success_count,
                    failure_count,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "finished Meilisearch bootstrap"
                )
            }
            Ok(MeiliReadiness::Bootstrapping) => {
                *state.meili_readiness.write().await = MeiliReadiness::Bootstrapping;
                warn!(
                    success_count,
                    failure_count,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "Meilisearch bootstrap completed with incomplete coverage; PostgreSQL fallback remains active"
                )
            }
            Ok(MeiliReadiness::Disabled) => {}
            Err(error) => {
                warn!("failed to refresh Meilisearch readiness after bootstrap: {error:?}")
            }
        }
    })
}

pub(in crate::server_main) async fn load_meili_bootstrap_user_ids(
    pool: &PgPool,
) -> Result<Vec<Uuid>> {
    let user_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT DISTINCT user_id
        FROM search_documents
        WHERE NOT EXISTS (
          SELECT 1
          FROM provider_profiles pp
          WHERE pp.user_id = search_documents.user_id
            AND pp.status = 'syncing'
        )
          AND NOT EXISTS (
            SELECT 1
            FROM sync_jobs sj
            WHERE sj.user_id = search_documents.user_id
              AND sj.status IN ('queued', 'running')
        )
        ORDER BY user_id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(user_ids)
}

pub(in crate::server_main) async fn meili_is_ready(state: &AppState) -> bool {
    matches!(*state.meili_readiness.read().await, MeiliReadiness::Ready)
}

pub(in crate::server_main) async fn meili_is_ready_for_user(
    state: &AppState,
    user_id: Uuid,
) -> bool {
    let Some(meili) = state.meili.as_ref() else {
        return false;
    };

    if !*state.meili_schema_ready.read().await {
        return false;
    }

    if state.meili_bootstrapping_users.contains(&user_id) {
        return false;
    }

    if meili_is_ready(state).await {
        return true;
    }

    matches!(
        inspect_meili_readiness_for_user(meili, &state.pool, user_id, true).await,
        Ok(MeiliReadiness::Ready)
    )
}

pub(in crate::server_main) async fn refresh_meili_readiness(
    state: &AppState,
) -> Result<MeiliReadiness> {
    let schema_ready = *state.meili_schema_ready.read().await;
    let readiness = match &state.meili {
        Some(meili) => inspect_meili_readiness(meili, &state.pool, schema_ready).await?,
        None => MeiliReadiness::Disabled,
    };
    *state.meili_readiness.write().await = readiness;
    Ok(readiness)
}

#[cfg(test)]
mod tests {
    use super::*;

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
                admin_password: None,
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

    #[test]
    fn determine_meili_readiness_is_ready_when_postgres_is_empty() {
        let readiness = determine_meili_readiness(
            SearchIndexCounts {
                postgres_channel_documents: 0,
                postgres_program_documents: 0,
                meili_channel_documents: 0,
                meili_program_documents: 0,
            },
            true,
        );

        assert_eq!(readiness, MeiliReadiness::Ready);
    }

    #[test]
    fn determine_meili_readiness_requires_matching_counts() {
        let readiness = determine_meili_readiness(
            SearchIndexCounts {
                postgres_channel_documents: 10,
                postgres_program_documents: 25,
                meili_channel_documents: 10,
                meili_program_documents: 24,
            },
            true,
        );

        assert_eq!(readiness, MeiliReadiness::Bootstrapping);
    }

    #[test]
    fn determine_meili_readiness_is_ready_when_counts_match() {
        let readiness = determine_meili_readiness(
            SearchIndexCounts {
                postgres_channel_documents: 10,
                postgres_program_documents: 25,
                meili_channel_documents: 10,
                meili_program_documents: 25,
            },
            true,
        );

        assert_eq!(readiness, MeiliReadiness::Ready);
    }

    #[test]
    fn determine_meili_readiness_requires_schema_compatibility() {
        let readiness = determine_meili_readiness(
            SearchIndexCounts {
                postgres_channel_documents: 10,
                postgres_program_documents: 25,
                meili_channel_documents: 10,
                meili_program_documents: 25,
            },
            false,
        );

        assert_eq!(readiness, MeiliReadiness::Bootstrapping);
    }

    #[tokio::test]
    async fn meili_is_ready_for_user_rejects_bootstrapping_users_even_when_globally_ready() {
        let mut state = sample_app_state();
        let user_id = Uuid::from_u128(91);
        state.meili = Some(Arc::new(
            MeilisearchClient::new("http://localhost:7700", None::<String>).expect("meili client"),
        ));
        *state.meili_readiness.write().await = MeiliReadiness::Ready;
        state.meili_bootstrapping_users.insert(user_id);

        assert!(!meili_is_ready_for_user(&state, user_id).await);
    }

    #[test]
    fn apply_meili_schema_version_overwrites_the_reserved_marker() {
        let mut synonyms = HashMap::new();
        synonyms.insert(
            MEILI_SCHEMA_VERSION_KEY.to_string(),
            vec!["legacy".to_string()],
        );

        let updated = apply_meili_schema_version(synonyms);

        assert_eq!(
            updated.get(MEILI_SCHEMA_VERSION_KEY),
            Some(&vec![MEILI_SCHEMA_VERSION.to_string()])
        );
    }

    #[test]
    fn channel_doc_contains_term_checks_title_subtitle_and_search_text() {
        let doc = MeiliChannelDoc {
            id: "doc".to_string(),
            user_id: "user".to_string(),
            profile_id: Uuid::nil().to_string(),
            entity_id: Uuid::nil().to_string(),
            channel_name: "Live from Augusta".to_string(),
            subtitle: Some("SE| VIAPLAY PPV".to_string()),
            category_name_raw: Some("SE| VIAPLAY PPV".to_string()),
            country_code: Some("SE".to_string()),
            provider_name: Some("viaplay".to_string()),
            is_ppv: true,
            is_vip: false,
            has_epg: false,
            broad_categories: vec!["sports".to_string()],
            event_titles: vec!["The Masters".to_string()],
            event_keywords: vec!["masters".to_string()],
            is_event_channel: true,
            is_placeholder_channel: false,
            is_hidden: false,
            has_catchup: false,
            archive_duration_hours: None,
            epg_channel_id: None,
            search_text: "Live from Augusta SE| VIAPLAY PPV The Masters live".to_string(),
            sort_rank: 0,
            updated_at: 0,
        };

        assert!(channel_doc_contains_term(&doc, "augusta"));
        assert!(channel_doc_contains_term(&doc, "viaplay"));
        assert!(channel_doc_contains_term(&doc, "masters"));
    }

    #[test]
    fn derive_search_metadata_marks_event_and_placeholder_channels() {
        let lexicon = lexicon::SearchLexicon {
            known_prefixes: ["SE", "UK", "ASIA", "4K"]
                .into_iter()
                .map(String::from)
                .collect(),
            country_prefixes: ["SE", "UK"].into_iter().map(String::from).collect(),
            region_prefixes: ["ASIA", "4K"].into_iter().map(String::from).collect(),
            provider_aliases: vec![
                lexicon::ProviderAlias {
                    alias: "sky sports".to_string(),
                    normalized_alias: "sky sports".to_string(),
                    alias_tokens: vec!["sky".to_string(), "sports".to_string()],
                    key: "sky".to_string(),
                },
                lexicon::ProviderAlias {
                    alias: "viaplay".to_string(),
                    normalized_alias: "viaplay".to_string(),
                    alias_tokens: vec!["viaplay".to_string()],
                    key: "viaplay".to_string(),
                },
                lexicon::ProviderAlias {
                    alias: "tv3".to_string(),
                    normalized_alias: "tv3".to_string(),
                    alias_tokens: vec!["tv3".to_string()],
                    key: "tv3".to_string(),
                },
            ],
            provider_labels: HashMap::from([
                (
                    "sky".to_string(),
                    vec!["sky".to_string(), "sky sports".to_string()],
                ),
                ("viaplay".to_string(), vec!["viaplay".to_string()]),
                ("tv3".to_string(), vec!["tv3".to_string()]),
            ]),
            typo_disabled_words: lexicon::short_search_abbreviations()
                .iter()
                .map(|value| value.to_string())
                .collect(),
        };
        let (
            country_code,
            region_code,
            provider_key,
            provider_labels,
            broad_categories,
            event_keywords,
            is_event_channel,
            is_placeholder_channel,
            sort_rank,
        ) = derive_search_metadata(
            &lexicon,
            "NO EVENT STREAMING NOW - | 8K EXCLUSIVE | SE: DAZN PPV 66",
            Some("SE| VIAPLAY PPV"),
            &["The Masters".to_string()],
        );

        assert_eq!(country_code.as_deref(), Some("SE"));
        assert!(region_code.is_none());
        assert_eq!(provider_key.as_deref(), Some("viaplay"));
        assert!(provider_labels.iter().any(|label| label == "viaplay"));
        assert!(broad_categories.iter().any(|category| category == "sports"));
        assert!(event_keywords.iter().any(|keyword| keyword == "masters"));
        assert!(is_event_channel);
        assert!(is_placeholder_channel);
        assert_eq!(sort_rank, 5);
    }
}

pub(in crate::server_main) async fn configure_meili_indexes(
    client: &MeilisearchClient,
    pool: &PgPool,
) -> Result<()> {
    let lexicon = lexicon::load_search_lexicon(pool, None).await?;
    configure_meili_index(
        client,
        "channels",
        &[
            "user_id",
            "profile_id",
            "country_code",
            "provider_name",
            "is_ppv",
            "is_vip",
            "has_epg",
            "broad_categories",
            "has_catchup",
            "is_event_channel",
            "is_placeholder_channel",
            "is_hidden",
        ],
        &[
            "channel_name",
            "event_titles",
            "provider_name",
            "broad_categories",
            "category_name_raw",
            "search_text",
        ],
        &["sort_rank", "channel_name", "updated_at"],
        &lexicon,
    )
    .await?;
    configure_meili_index(
        client,
        "programs",
        &[
            "user_id",
            "profile_id",
            "country_code",
            "provider_name",
            "is_ppv",
            "is_vip",
            "has_epg",
            "broad_categories",
            "can_catchup",
            "is_hidden",
        ],
        &[
            "title",
            "channel_name",
            "description",
            "provider_name",
            "broad_categories",
            "search_text",
        ],
        &["sort_priority", "starts_at", "ends_at"],
        &lexicon,
    )
    .await?;
    Ok(())
}

pub(in crate::server_main) async fn configure_meili_index(
    client: &MeilisearchClient,
    name: &str,
    filterable_attributes: &[&str],
    searchable_attributes: &[&str],
    sortable_attributes: &[&str],
    lexicon: &SearchLexicon,
) -> Result<()> {
    if let Ok(task) = client.create_index(name, Some("id")).await {
        task.wait_for_completion(client, None, None).await?;
    }

    let index = client.index(name);
    index
        .set_filterable_attributes(filterable_attributes)
        .await?
        .wait_for_completion(client, None, None)
        .await?;
    index
        .set_searchable_attributes(searchable_attributes)
        .await?
        .wait_for_completion(client, None, None)
        .await?;
    index
        .set_sortable_attributes(sortable_attributes)
        .await?
        .wait_for_completion(client, None, None)
        .await?;

    let synonyms = apply_meili_schema_version(lexicon::build_meili_synonyms(lexicon));
    index
        .set_synonyms(&synonyms)
        .await?
        .wait_for_completion(client, None, None)
        .await?;

    index
        .set_pagination(PaginationSetting {
            max_total_hits: MEILI_MAX_TOTAL_HITS,
        })
        .await?
        .wait_for_completion(client, None, None)
        .await?;

    let typo_tolerance = TypoToleranceSettings {
        enabled: Some(true),
        disable_on_attributes: None,
        disable_on_words: Some(lexicon.typo_disabled_words.iter().cloned().collect()),
        min_word_size_for_typos: Some(MinWordSizeForTypos {
            one_typo: Some(5),
            two_typos: Some(9),
        }),
    };
    index
        .set_typo_tolerance(&typo_tolerance)
        .await?
        .wait_for_completion(client, None, None)
        .await?;

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

async fn delete_meili_documents(
    meili: &MeilisearchClient,
    index_name: &str,
    filter: &str,
) -> Result<()> {
    let index = meili.index(index_name);
    let mut query = DocumentDeletionQuery::new(&index);
    query.with_filter(filter);
    index
        .delete_documents_with(&query)
        .await?
        .wait_for_completion(
            meili,
            Some(MEILI_TASK_POLL_INTERVAL),
            Some(MEILI_TASK_TIMEOUT),
        )
        .await?;
    Ok(())
}

async fn wait_for_pending_meili_batch(
    meili: &MeilisearchClient,
    pending_batches: &mut VecDeque<PendingMeiliBatch>,
    user_id: Uuid,
    profile_id: Option<Uuid>,
) -> Result<()> {
    let Some(pending) = pending_batches.pop_front() else {
        return Ok(());
    };

    pending
        .task
        .wait_for_completion(
            meili,
            Some(MEILI_TASK_POLL_INTERVAL),
            Some(MEILI_TASK_TIMEOUT),
        )
        .await?;

    info!(
        user_id = %user_id,
        profile_id = ?profile_id,
        phase = pending.phase,
        batch = pending.batch_number,
        indexed_documents = pending.indexed_documents,
        "completed Meilisearch batch"
    );
    Ok(())
}

async fn flush_pending_meili_batches(
    meili: &MeilisearchClient,
    pending_batches: &mut VecDeque<PendingMeiliBatch>,
    user_id: Uuid,
    profile_id: Option<Uuid>,
) -> Result<()> {
    while !pending_batches.is_empty() {
        wait_for_pending_meili_batch(meili, pending_batches, user_id, profile_id).await?;
    }
    Ok(())
}

pub(in crate::server_main) async fn rebuild_meili_indexes(
    state: &AppState,
    meili: &MeilisearchClient,
    user_id: Uuid,
    profile_id: Option<Uuid>,
) -> Result<()> {
    let pool = &state.pool;
    let user_id_str = user_id.to_string();
    let reindex_started_at = Instant::now();
    let profile_filter = profile_id
        .map(|profile_id| format!(r#"user_id = "{user_id}" AND profile_id = "{profile_id}""#))
        .unwrap_or_else(|| format!(r#"user_id = "{user_id}""#));
    info!(user_id = %user_id, profile_id = ?profile_id, "starting Meilisearch reindex");
    delete_meili_documents(meili, "channels", &profile_filter).await?;
    delete_meili_documents(meili, "programs", &profile_filter).await?;

    let channel_event_titles = load_channel_event_titles(pool, user_id, profile_id).await?;

    let channels_index = meili.index("channels");
    let mut last_channel_id = None;
    let mut indexed_channel_docs = 0usize;
    let mut indexed_channel_batches = 0usize;
    let mut channel_program_metadata = HashMap::<Uuid, ChannelProgramMetadata>::new();
    let mut pending_channel_batches = VecDeque::new();
    loop {
        let rows = sqlx::query_as::<_, MeiliChannelRow>(
            r#"
            SELECT
              c.id,
              c.profile_id,
              c.name,
              cc.name AS category_name,
              c.search_country_code,
              c.search_provider_name,
              c.search_is_ppv,
              c.search_is_vip,
              c.has_catchup,
              c.archive_duration_hours,
              c.epg_channel_id,
              EXISTS(
                SELECT 1
                FROM programs p
                WHERE p.user_id = c.user_id
                  AND p.channel_id = c.id
                  AND p.end_at > NOW() - ($5 * INTERVAL '1 hour')
                  AND p.start_at < NOW() + ($6 * INTERVAL '1 day')
              ) AS has_epg,
              c.updated_at
            FROM channels c
            LEFT JOIN channel_categories cc ON cc.id = c.category_id
            WHERE c.user_id = $1
              AND ($2::uuid IS NULL OR c.profile_id = $2)
              AND ($3::uuid IS NULL OR c.id > $3)
            ORDER BY c.id ASC
            LIMIT $4
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(last_channel_id)
        .bind(MEILI_INDEX_BATCH_SIZE)
        .bind(EPG_RETENTION_PAST_HOURS)
        .bind(EPG_RETENTION_FUTURE_DAYS)
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            break;
        }

        let batch_size = rows.len();
        let docs = rows
            .iter()
            .map(|row| {
                let event_titles = channel_event_titles
                    .get(&row.id)
                    .cloned()
                    .unwrap_or_default();
                let broad_categories = derive_broad_categories(&format!(
                    "{} {} {}",
                    row.name,
                    row.category_name.as_deref().unwrap_or_default(),
                    event_titles.join(" ")
                ));
                let event_keywords =
                    derive_event_keywords(&format!("{} {}", row.name, event_titles.join(" ")));
                let is_placeholder_channel = is_placeholder_channel_name(&row.name);
                let is_event_channel = detect_event_channel(&row.name)
                    || event_titles.iter().any(|title| detect_event_channel(title));
                let mut sort_rank = 2;
                if is_placeholder_channel {
                    sort_rank = 5;
                } else if is_event_channel {
                    sort_rank = 0;
                } else if broad_categories.iter().any(|category| category == "sports") {
                    sort_rank = 1;
                }
                let visibility = classify_channel_visibility(
                    &row.name,
                    row.category_name.as_deref(),
                    &event_titles,
                );

                MeiliChannelDoc {
                    id: format!("{}_{}", user_id, row.id),
                    user_id: user_id_str.clone(),
                    profile_id: row.profile_id.to_string(),
                    entity_id: row.id.to_string(),
                    channel_name: row.name.clone(),
                    subtitle: row.category_name.clone(),
                    category_name_raw: row.category_name.clone(),
                    country_code: row.search_country_code.clone(),
                    provider_name: row.search_provider_name.clone(),
                    is_ppv: row.search_is_ppv,
                    is_vip: row.search_is_vip,
                    has_epg: row.has_epg,
                    broad_categories,
                    event_titles: event_titles.clone(),
                    event_keywords: event_keywords.clone(),
                    is_event_channel,
                    is_placeholder_channel,
                    is_hidden: visibility.is_hidden,
                    has_catchup: row.has_catchup,
                    archive_duration_hours: row.archive_duration_hours,
                    epg_channel_id: row.epg_channel_id.clone(),
                    search_text: format!(
                        "{} {} {} {} {} {} {} {} {}",
                        row.name,
                        row.category_name.as_deref().unwrap_or_default(),
                        event_titles.join(" "),
                        event_keywords.join(" "),
                        row.search_provider_name.as_deref().unwrap_or_default(),
                        if row.search_is_ppv { "ppv" } else { "" },
                        if row.search_is_vip { "vip" } else { "" },
                        if row.has_epg { "epg" } else { "" },
                        if row.has_catchup {
                            "catchup archive"
                        } else {
                            "live"
                        }
                    )
                    .trim()
                    .to_string(),
                    sort_rank,
                    updated_at: row.updated_at.timestamp(),
                }
            })
            .collect::<Vec<_>>();
        for (row, doc) in rows.iter().zip(docs.iter()) {
            channel_program_metadata.insert(
                row.id,
                ChannelProgramMetadata {
                    country_code: doc.country_code.clone(),
                    provider_name: doc.provider_name.clone(),
                    is_ppv: doc.is_ppv,
                    is_vip: doc.is_vip,
                    broad_categories: doc.broad_categories.clone(),
                    is_hidden: doc.is_hidden,
                },
            );
        }
        indexed_channel_docs += batch_size;
        indexed_channel_batches += 1;
        pending_channel_batches.push_back(PendingMeiliBatch {
            task: channels_index.add_or_replace(&docs, Some("id")).await?,
            phase: "channel",
            batch_number: indexed_channel_batches,
            indexed_documents: indexed_channel_docs,
        });
        if pending_channel_batches.len() >= MEILI_MAX_IN_FLIGHT_TASKS {
            wait_for_pending_meili_batch(meili, &mut pending_channel_batches, user_id, profile_id)
                .await?;
        }
        last_channel_id = rows.last().map(|row| row.id);
    }
    flush_pending_meili_batches(meili, &mut pending_channel_batches, user_id, profile_id).await?;

    let programs_index = meili.index("programs");
    let mut last_program_id = None;
    let mut indexed_program_docs = 0usize;
    let mut indexed_program_batches = 0usize;
    let mut pending_program_batches = VecDeque::new();
    info!(
        user_id = %user_id,
        profile_id = ?profile_id,
        "starting Meilisearch program indexing phase"
    );
    loop {
        let rows = sqlx::query_as::<_, MeiliProgramRow>(
            r#"
            SELECT
              p.id,
              p.profile_id,
              p.channel_id,
              p.channel_name,
              cc.name AS category_name,
              p.search_country_code,
              p.search_provider_name,
              p.search_is_ppv,
              p.search_is_vip,
              p.title,
              p.description,
              p.start_at,
              p.end_at,
              p.can_catchup
            FROM programs p
            LEFT JOIN channels c ON c.id = p.channel_id
            LEFT JOIN channel_categories cc ON cc.id = c.category_id
            WHERE p.user_id = $1
              AND ($2::uuid IS NULL OR p.profile_id = $2)
              AND ($3::uuid IS NULL OR p.id > $3)
            ORDER BY p.id ASC
            LIMIT $4
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(last_program_id)
        .bind(MEILI_INDEX_BATCH_SIZE)
        .fetch_all(pool)
        .await?;
        if rows.is_empty() {
            break;
        }

        let now = Utc::now();
        let batch_size = rows.len();
        let docs = rows
            .iter()
            .map(|row| {
                let (
                    country_code,
                    provider_name,
                    is_ppv,
                    is_vip,
                    broad_categories,
                    is_hidden,
                    has_epg,
                ) = row
                    .channel_id
                    .and_then(|channel_id| channel_program_metadata.get(&channel_id))
                    .map(|metadata| {
                        (
                            metadata.country_code.clone(),
                            metadata.provider_name.clone(),
                            metadata.is_ppv,
                            metadata.is_vip,
                            metadata.broad_categories.clone(),
                            metadata.is_hidden,
                            true,
                        )
                    })
                    .unwrap_or_else(|| {
                        let broad_categories = derive_broad_categories(&format!(
                            "{} {} {}",
                            row.title,
                            row.channel_name.as_deref().unwrap_or_default(),
                            row.description.as_deref().unwrap_or_default()
                        ));
                        (
                            row.search_country_code.clone(),
                            row.search_provider_name.clone(),
                            row.search_is_ppv,
                            row.search_is_vip,
                            broad_categories,
                            false,
                            true,
                        )
                    });

                MeiliProgramDoc {
                    id: format!("{}_{}", user_id, row.id),
                    user_id: user_id_str.clone(),
                    profile_id: row.profile_id.to_string(),
                    entity_id: row.id.to_string(),
                    country_code,
                    provider_name: provider_name.clone(),
                    is_ppv,
                    is_vip,
                    has_epg,
                    broad_categories: broad_categories.clone(),
                    channel_name: row.channel_name.clone(),
                    title: row.title.clone(),
                    description: row.description.clone(),
                    search_text: format!(
                        "{} {} {} {} {} {} {}",
                        row.title,
                        row.channel_name.as_deref().unwrap_or_default(),
                        row.description.as_deref().unwrap_or_default(),
                        provider_name.as_deref().unwrap_or_default(),
                        if is_ppv { "ppv" } else { "" },
                        if is_vip { "vip" } else { "" },
                        broad_categories.join(" ")
                    )
                    .trim()
                    .to_string(),
                    starts_at: row.start_at.timestamp(),
                    ends_at: row.end_at.timestamp(),
                    can_catchup: row.can_catchup,
                    channel_id: row.channel_id.map(|value| value.to_string()),
                    is_hidden,
                    sort_priority: if row.channel_id.is_some()
                        && row.start_at <= now
                        && row.end_at >= now
                    {
                        0
                    } else if row.channel_id.is_some() && row.end_at <= now && row.can_catchup {
                        1
                    } else if row.start_at > now {
                        2
                    } else {
                        3
                    },
                }
            })
            .collect::<Vec<_>>();
        indexed_program_docs += batch_size;
        indexed_program_batches += 1;
        pending_program_batches.push_back(PendingMeiliBatch {
            task: programs_index.add_or_replace(&docs, Some("id")).await?,
            phase: "program",
            batch_number: indexed_program_batches,
            indexed_documents: indexed_program_docs,
        });
        if pending_program_batches.len() >= MEILI_MAX_IN_FLIGHT_TASKS {
            wait_for_pending_meili_batch(meili, &mut pending_program_batches, user_id, profile_id)
                .await?;
        }
        last_program_id = rows.last().map(|row| row.id);
    }
    flush_pending_meili_batches(meili, &mut pending_program_batches, user_id, profile_id).await?;

    info!(
        user_id = %user_id,
        profile_id = ?profile_id,
        channel_documents = indexed_channel_docs,
        program_documents = indexed_program_docs,
        elapsed_ms = reindex_started_at.elapsed().as_millis() as u64,
        "finished Meilisearch reindex"
    );
    Ok(())
}

pub(in crate::server_main) async fn refresh_meili_channels_delta(
    state: &AppState,
    meili: &MeilisearchClient,
    user_id: Uuid,
    profile_id: Uuid,
    changed_remote_stream_ids: &[i32],
    removed_channel_ids: &[Uuid],
) -> Result<()> {
    let started_at = Instant::now();
    let _ = changed_remote_stream_ids;
    let _ = removed_channel_ids;
    rebuild_meili_indexes(state, meili, user_id, Some(profile_id)).await?;

    info!(
        user_id = %user_id,
        profile_id = %profile_id,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "finished Meilisearch profile rebuild"
    );
    Ok(())
}
