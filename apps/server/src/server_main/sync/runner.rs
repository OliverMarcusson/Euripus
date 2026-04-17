use super::*;

#[derive(Debug)]
enum SearchRefreshScope {
    FullProfile {
        profile_id: Uuid,
    },
    ChannelDelta {
        profile_id: Uuid,
        changed_remote_stream_ids: Vec<i32>,
        removed_channel_ids: Vec<Uuid>,
        removed_program_ids: Vec<Uuid>,
    },
}

pub(super) fn spawn_sync_job(
    state: AppState,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(error) = run_sync_job(state.clone(), user_id, profile_id, job_id).await {
            error!("sync job {job_id} failed: {error:?}");
            let _ = sqlx::query(
                r#"
                UPDATE sync_jobs
                SET
                  status = 'failed',
                  finished_at = NOW(),
                  current_phase = 'failed',
                  phase_message = $2,
                  error_message = $2
                WHERE id = $1
                "#,
            )
            .bind(job_id)
            .bind(error.to_string())
            .execute(&state.pool)
            .await;

            let _ = sqlx::query(
                r#"
                UPDATE provider_profiles
                SET status = 'error', last_sync_error = $2, updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(profile_id)
            .bind(error.to_string())
            .execute(&state.pool)
            .await;
        }
    })
}

pub(super) fn should_refresh_channels(job_type: &str, existing_channel_count: i64) -> bool {
    matches!(job_type, "full" | "channels") || existing_channel_count == 0
}

pub(super) fn should_sync_epg(job_type: &str) -> bool {
    job_type != "channels"
}

async fn fetch_epg_feeds(
    client: &reqwest::Client,
    credentials: &XtreamCredentials,
    external_sources: &[EpgSourceRecord],
) -> Result<(Vec<FetchedEpgFeed>, Vec<EpgSourceSyncStatus>)> {
    let mut fetched_feeds = Vec::new();
    let mut source_statuses = Vec::new();
    let mut built_in_error = None;
    let mut join_set = JoinSet::new();
    let mut next_source_index = 0usize;

    {
        let client = client.clone();
        let credentials = credentials.clone();
        let next_priority = external_sources
            .iter()
            .map(|source| source.priority)
            .max()
            .unwrap_or(-1)
            + 1;
        join_set.spawn(async move {
            EpgFetchResult::BuiltIn(
                async move {
                    let feed = xtreme::fetch_xmltv(&client, &credentials).await?;
                    Ok(FetchedEpgFeed {
                        source_id: None,
                        source_kind: "xtream".to_string(),
                        source_label: xtreme::build_xmltv_url(&credentials)?.to_string(),
                        priority: next_priority,
                        feed,
                    })
                }
                .await,
            )
        });
    }

    while next_source_index < external_sources.len() && join_set.len() < EPG_FETCH_CONCURRENCY {
        let source = external_sources[next_source_index].clone();
        let client = client.clone();
        join_set.spawn(async move {
            EpgFetchResult::External(fetch_external_epg_source(client, source).await)
        });
        next_source_index += 1;
    }

    while let Some(result) = join_set.join_next().await {
        match result? {
            EpgFetchResult::BuiltIn(Ok(feed)) => {
                info!(
                    source_kind = %feed.source_kind,
                    source = %feed.source_label,
                    programme_count = feed.feed.programmes.len(),
                    channel_count = feed.feed.channels.len(),
                    "fetched built-in Xtream EPG source"
                );
                fetched_feeds.push(feed)
            }
            EpgFetchResult::BuiltIn(Err(error)) => {
                built_in_error = Some(error.to_string());
                error!("failed to fetch built-in Xtream XMLTV feed: {error:?}");
            }
            EpgFetchResult::External(ExternalEpgFetchResult::Success(feed)) => {
                info!(
                    source_kind = %feed.source_kind,
                    source = %feed.source_label,
                    programme_count = feed.feed.programmes.len(),
                    channel_count = feed.feed.channels.len(),
                    "fetched external EPG source"
                );
                fetched_feeds.push(feed)
            }
            EpgFetchResult::External(ExternalEpgFetchResult::Failure(status)) => {
                source_statuses.push(status)
            }
        }

        if next_source_index < external_sources.len() {
            let source = external_sources[next_source_index].clone();
            let client = client.clone();
            join_set.spawn(async move {
                EpgFetchResult::External(fetch_external_epg_source(client, source).await)
            });
            next_source_index += 1;
        }
    }

    fetched_feeds.sort_by_key(|feed| feed.priority);

    if fetched_feeds.is_empty() {
        if let Some(error_message) = built_in_error {
            warn!(
                "all XMLTV feeds failed; continuing sync without guide data because the built-in Xtream XMLTV feed was malformed: {error_message}"
            );
            return Ok((fetched_feeds, source_statuses));
        }

        return Err(anyhow!(
            "no EPG feed could be ingested: {}",
            built_in_error.unwrap_or_else(|| "All configured EPG sources failed.".to_string())
        ));
    }

    Ok((fetched_feeds, source_statuses))
}

async fn fetch_external_epg_source(
    client: reqwest::Client,
    source: EpgSourceRecord,
) -> ExternalEpgFetchResult {
    let started_at = Instant::now();
    match url::Url::parse(&source.url) {
        Ok(url) => match xmltv::fetch_xmltv(&client, &url).await {
            Ok(feed) => {
                info!(
                    source_kind = %source.source_kind,
                    source = %source.url,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    programme_count = feed.programmes.len(),
                    channel_count = feed.channels.len(),
                    "fetched external XMLTV feed"
                );
                ExternalEpgFetchResult::Success(FetchedEpgFeed {
                    source_id: Some(source.id),
                    source_kind: source.source_kind,
                    source_label: source.url,
                    priority: source.priority,
                    feed,
                })
            }
            Err(error) => {
                error!(
                    source = %source.url,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "failed to fetch external EPG source: {error:?}"
                );
                ExternalEpgFetchResult::Failure(EpgSourceSyncStatus {
                    source_id: source.id,
                    last_sync_error: Some(error.to_string()),
                    last_program_count: None,
                    last_matched_count: None,
                    mark_synced: false,
                })
            }
        },
        Err(error) => ExternalEpgFetchResult::Failure(EpgSourceSyncStatus {
            source_id: source.id,
            last_sync_error: Some(format!("Invalid EPG source URL: {error}")),
            last_program_count: None,
            last_matched_count: None,
            mark_synced: false,
        }),
    }
}

async fn run_sync_job(
    state: AppState,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "UPDATE sync_jobs SET status = 'running', started_at = NOW(), current_phase = 'starting', phase_message = 'Preparing sync' WHERE id = $1",
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"UPDATE provider_profiles SET status = 'syncing', last_sync_error = NULL, updated_at = NOW() WHERE id = $1"#,
    )
    .bind(profile_id)
    .execute(&state.pool)
    .await?;
    let job_type = sqlx::query_scalar::<_, String>("SELECT job_type FROM sync_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&state.pool)
        .await?;

    let profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    let decrypted_password = decrypt_secret(&state.config.encryption_key, &profile.password_encrypted)
        .map_err(|_| anyhow!("Stored provider password could not be decrypted. Re-enter your provider password and save the profile again."))?;

    let credentials = XtreamCredentials {
        base_url: profile.base_url.clone(),
        username: profile.username.clone(),
        password: decrypted_password,
        output_format: profile.output_format.clone(),
    };

    update_sync_job_phase(
        &state.pool,
        job_id,
        "validating",
        0,
        &job_type,
        "Validating provider",
    )
    .await?;
    let validation_started_at = Instant::now();
    info!("sync job {job_id}: validating provider");
    let validation = xtreme::validate_profile(&state.provider_http_client, &credentials).await?;
    info!(
        job_id = %job_id,
        elapsed_ms = validation_started_at.elapsed().as_millis() as u64,
        "validated provider for sync job"
    );
    if !validation.valid {
        return Err(anyhow!("provider validation failed during sync"));
    }

    let existing_channel_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM channels WHERE user_id = $1 AND profile_id = $2",
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_one(&state.pool)
    .await?;
    let refresh_channels = should_refresh_channels(&job_type, existing_channel_count);
    let sync_epg = should_sync_epg(&job_type);

    let (categories, channels) = if refresh_channels {
        let provider_fetch_started_at = Instant::now();
        update_sync_job_phase(
            &state.pool,
            job_id,
            "fetching-categories",
            1,
            &job_type,
            "Fetching live categories",
        )
        .await?;
        let categories_future = async {
            let started_at = Instant::now();
            info!("sync job {job_id}: fetching categories");
            let categories =
                xtreme::fetch_categories(&state.provider_http_client, &credentials).await?;
            info!(
                job_id = %job_id,
                category_count = categories.len(),
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "fetched live categories"
            );
            Ok::<Vec<XtreamCategory>, anyhow::Error>(categories)
        };
        update_sync_job_phase(
            &state.pool,
            job_id,
            "fetching-channels",
            2,
            &job_type,
            "Fetching live channels",
        )
        .await?;
        let channels_future = async {
            let started_at = Instant::now();
            info!("sync job {job_id}: fetching live streams");
            let channels =
                xtreme::fetch_live_streams(&state.provider_http_client, &credentials).await?;
            info!(
                job_id = %job_id,
                channel_count = channels.len(),
                elapsed_ms = started_at.elapsed().as_millis() as u64,
                "fetched live streams"
            );
            Ok::<Vec<XtreamChannel>, anyhow::Error>(channels)
        };
        let (categories, channels) = tokio::try_join!(categories_future, channels_future)?;
        info!(
            job_id = %job_id,
            category_count = categories.len(),
            channel_count = channels.len(),
            elapsed_ms = provider_fetch_started_at.elapsed().as_millis() as u64,
            "fetched provider channel catalog"
        );
        (Some(categories), Some(channels))
    } else {
        (None, None)
    };

    let search_refresh_scope = if sync_epg {
        let epg_sources = sqlx::query_as::<_, EpgSourceRecord>(
            r#"
            SELECT
              id, url, priority, source_kind
            FROM epg_sources
            WHERE profile_id = $1 AND enabled = TRUE
            ORDER BY priority ASC, created_at ASC
            "#,
        )
        .bind(profile_id)
        .fetch_all(&state.pool)
        .await?;
        let epg_fetch_completed_phases = if refresh_channels { 3 } else { 1 };
        update_sync_job_phase(
            &state.pool,
            job_id,
            "fetching-epg",
            epg_fetch_completed_phases,
            &job_type,
            "Fetching EPG feeds",
        )
        .await?;
        let epg_fetch_started_at = Instant::now();
        let (fetched_feeds, mut source_statuses) =
            fetch_epg_feeds(&state.provider_http_client, &credentials, &epg_sources).await?;
        info!(
            job_id = %job_id,
            feed_count = fetched_feeds.len(),
            elapsed_ms = epg_fetch_started_at.elapsed().as_millis() as u64,
            "fetched EPG feeds"
        );

        let epg_match_completed_phases = if refresh_channels { 4 } else { 2 };
        update_sync_job_phase(
            &state.pool,
            job_id,
            "matching-epg",
            epg_match_completed_phases,
            &job_type,
            "Matching guide data",
        )
        .await?;
        info!("sync job {job_id}: persisting sync data");
        let persist_started_at = Instant::now();
        let persisted_statuses = if refresh_channels {
            persistence::persist_full_sync_data(
                &state.pool,
                user_id,
                profile_id,
                job_id,
                &job_type,
                categories.as_deref().unwrap_or(&[]),
                channels.as_deref().unwrap_or(&[]),
                &fetched_feeds,
            )
            .await?
        } else {
            persistence::persist_epg_sync_data(
                &state.pool,
                user_id,
                profile_id,
                job_id,
                &job_type,
                &fetched_feeds,
            )
            .await?
        };
        source_statuses.extend(persisted_statuses);
        persistence::update_epg_source_statuses(&state.pool, &source_statuses).await?;
        info!(
            job_id = %job_id,
            elapsed_ms = persist_started_at.elapsed().as_millis() as u64,
            "finished persisting sync data"
        );
        SearchRefreshScope::FullProfile { profile_id }
    } else {
        update_sync_job_phase(
            &state.pool,
            job_id,
            "saving-channels",
            3,
            &job_type,
            "Saving channel catalog",
        )
        .await?;
        let persist_started_at = Instant::now();
        let channel_delta = persistence::persist_channel_sync_data(
            &state.pool,
            user_id,
            profile_id,
            job_id,
            categories.as_deref().unwrap_or(&[]),
            channels.as_deref().unwrap_or(&[]),
        )
        .await?;
        info!(
            job_id = %job_id,
            elapsed_ms = persist_started_at.elapsed().as_millis() as u64,
            "finished persisting channel catalog"
        );
        SearchRefreshScope::ChannelDelta {
            profile_id,
            changed_remote_stream_ids: channel_delta.changed_remote_stream_ids,
            removed_channel_ids: channel_delta.removed_channel_ids,
            removed_program_ids: channel_delta.removed_program_ids,
        }
    };

    sqlx::query(
        r#"
        UPDATE sync_jobs
        SET
          status = 'succeeded',
          finished_at = NOW(),
          current_phase = 'finished',
          completed_phases = total_phases,
          phase_message = 'Sync complete',
          error_message = NULL
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        UPDATE provider_profiles
        SET status = 'valid', last_sync_at = NOW(), last_sync_error = NULL, last_validated_at = NOW(), updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(profile_id)
    .execute(&state.pool)
    .await?;
    invalidate_channel_visibility_cache(&state, user_id, Some(profile_id));
    spawn_search_refresh(state.clone(), user_id, job_id, search_refresh_scope);

    Ok(())
}

fn spawn_search_refresh(
    state: AppState,
    user_id: Uuid,
    job_id: Uuid,
    scope: SearchRefreshScope,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let refresh_started_at = Instant::now();
        info!("sync job {job_id}: refreshing search indexes in background");

        if state.meili.is_some() {
            state.meili_bootstrapping_users.insert(user_id);
        }

        if let Err(error) = search::indexing::refresh_search_metadata(&state, user_id).await {
            warn!("sync job {job_id}: failed to refresh PostgreSQL search metadata: {error:?}");
            return;
        }

        let mut rebuild_postgres_fallback = state.meili.is_none();
        if let Some(meili) = &state.meili {
            info!("sync job {job_id}: refreshing Meilisearch indexes in background");
            let meili_refresh = match &scope {
                SearchRefreshScope::FullProfile { profile_id } => {
                    search::indexing::rebuild_meili_indexes(
                        &state,
                        meili,
                        user_id,
                        Some(*profile_id),
                    )
                    .await
                }
                SearchRefreshScope::ChannelDelta {
                    profile_id,
                    changed_remote_stream_ids,
                    removed_channel_ids,
                    removed_program_ids,
                } => {
                    search::indexing::refresh_meili_channels_delta(
                        &state,
                        meili,
                        user_id,
                        *profile_id,
                        changed_remote_stream_ids,
                        removed_channel_ids,
                        removed_program_ids,
                    )
                    .await
                }
            };

            let user_readiness = match meili_refresh {
                Ok(()) => {
                    search::indexing::inspect_meili_readiness_for_user(
                        meili,
                        &state.pool,
                        user_id,
                        true,
                    )
                    .await
                }
                Err(error) => {
                    warn!("sync job {job_id}: failed to rebuild Meilisearch indexes: {error:?}");
                    Err(error)
                }
            };

            match user_readiness {
                Ok(MeiliReadiness::Ready) => {
                    state.meili_bootstrapping_users.remove(&user_id);
                    rebuild_postgres_fallback = false;
                    if let Err(error) = search::indexing::refresh_meili_readiness(&state).await {
                        warn!(
                            "sync job {job_id}: failed to refresh Meilisearch readiness: {error:?}"
                        );
                    } else {
                        info!("sync job {job_id}: finished Meilisearch background refresh");
                    }
                }
                Ok(MeiliReadiness::Bootstrapping) => {
                    rebuild_postgres_fallback = true;
                    warn!(
                        "sync job {job_id}: Meilisearch rebuild finished but the user index is still incomplete"
                    );
                    if let Err(error) = search::indexing::refresh_meili_readiness(&state).await {
                        warn!(
                            "sync job {job_id}: failed to refresh Meilisearch readiness: {error:?}"
                        );
                    }
                }
                Ok(MeiliReadiness::Disabled) => {
                    rebuild_postgres_fallback = true;
                }
                Err(error) => {
                    rebuild_postgres_fallback = true;
                    warn!(
                        "sync job {job_id}: failed to inspect Meilisearch readiness for user after refresh: {error:?}"
                    );
                    if let Err(error) = search::indexing::refresh_meili_readiness(&state).await {
                        warn!(
                            "sync job {job_id}: failed to refresh Meilisearch readiness: {error:?}"
                        );
                    }
                }
            }
        } else {
            info!("sync job {job_id}: Meilisearch is disabled; skipping Meilisearch refresh");
        }

        if rebuild_postgres_fallback {
            info!(
                "sync job {job_id}: rebuilding PostgreSQL fallback search documents in background"
            );
            if let Err(error) =
                search::indexing::rebuild_postgres_search_documents(&state, user_id).await
            {
                warn!(
                    "sync job {job_id}: failed to rebuild PostgreSQL search documents: {error:?}"
                );
                return;
            }
        } else {
            info!(
                "sync job {job_id}: skipping PostgreSQL fallback search rebuild because Meilisearch is healthy"
            );
        }

        if state.meili.is_some()
            && state.meili_bootstrapping_users.contains(&user_id)
            && !matches!(
                *state.meili_readiness.read().await,
                MeiliReadiness::Disabled
            )
        {
            warn!(
                "sync job {job_id}: keeping user on PostgreSQL fallback until a later successful Meilisearch refresh"
            );
        }

        info!(
            job_id = %job_id,
            elapsed_ms = refresh_started_at.elapsed().as_millis() as u64,
            "finished background search refresh"
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_sync_jobs_refresh_channels_without_epg() {
        assert!(should_refresh_channels("channels", 5));
        assert!(!should_sync_epg("channels"));
        assert!(should_refresh_channels("epg", 0));
        assert!(should_sync_epg("epg"));
    }
}
