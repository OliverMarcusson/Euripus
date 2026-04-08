use super::*;

pub(super) async fn ensure_no_active_sync(pool: &PgPool, profile_id: Uuid) -> Result<(), AppError> {
    let active_job_count = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM sync_jobs WHERE profile_id = $1 AND status IN ('queued', 'running')"#,
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await?;

    if active_job_count > 0 {
        return Err(AppError::BadRequest(
            "A sync is already queued or running for this provider.".to_string(),
        ));
    }

    Ok(())
}

pub(super) async fn insert_sync_job(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_type: &str,
    trigger: &str,
) -> Result<SyncJobResponse> {
    let total_phases = total_phases_for_job(job_type);

    let job = sqlx::query_as::<_, SyncJobResponse>(
        r#"
        INSERT INTO sync_jobs (
          user_id,
          profile_id,
          status,
          job_type,
          trigger,
          current_phase,
          completed_phases,
          total_phases,
          phase_message
        )
        VALUES ($1, $2, 'queued', $3, $4, 'queued', 0, $5, 'Waiting to start')
        RETURNING
          id,
          status,
          job_type,
          trigger,
          created_at,
          started_at,
          finished_at,
          current_phase,
          completed_phases,
          total_phases,
          phase_message,
          error_message
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .bind(job_type)
    .bind(trigger)
    .bind(total_phases)
    .fetch_one(pool)
    .await?;

    Ok(job)
}

pub(super) fn total_phases_for_job(job_type: &str) -> i32 {
    match job_type {
        "channels" => CHANNEL_SYNC_TOTAL_PHASES,
        "epg" => EPG_SYNC_TOTAL_PHASES,
        _ => FULL_SYNC_TOTAL_PHASES,
    }
}

pub(super) async fn update_sync_job_phase(
    pool: &PgPool,
    job_id: Uuid,
    phase: &str,
    completed_phases: i32,
    job_type: &str,
    phase_message: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE sync_jobs
        SET
          current_phase = $2,
          completed_phases = $3,
          total_phases = $4,
          phase_message = $5
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(phase)
    .bind(completed_phases)
    .bind(total_phases_for_job(job_type))
    .bind(phase_message)
    .execute(pool)
    .await?;

    Ok(())
}

pub(super) async fn persist_full_sync_data(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
    job_type: &str,
    categories: &[XtreamCategory],
    channels: &[XtreamChannel],
    feeds: &[FetchedEpgFeed],
) -> Result<Vec<EpgSourceSyncStatus>> {
    let persist_started_at = Instant::now();
    let mut transaction = pool.begin().await?;
    bulk_upsert_categories(&mut transaction, user_id, profile_id, categories).await?;
    bulk_upsert_channels(&mut transaction, user_id, profile_id, channels).await?;
    info!(
        job_id = %job_id,
        category_count = categories.len(),
        channel_count = channels.len(),
        elapsed_ms = persist_started_at.elapsed().as_millis() as u64,
        "persisted provider categories and channels"
    );
    let persisted_channels = load_persisted_channels(&mut transaction, user_id, profile_id).await?;
    let channel_lookup = build_channel_lookup_index(&persisted_channels);
    let programme_resolution_started_at = Instant::now();
    let (programmes, source_statuses) = resolve_epg_programmes(feeds, &channel_lookup);
    info!(
        job_id = %job_id,
        programme_count = programmes.len(),
        elapsed_ms = programme_resolution_started_at.elapsed().as_millis() as u64,
        "resolved EPG programmes against persisted channels"
    );

    update_sync_job_phase(
        pool,
        job_id,
        "saving-programs",
        5,
        job_type,
        "Saving guide entries",
    )
    .await?;
    let programme_write_started_at = Instant::now();
    sqlx::query("DELETE FROM programs WHERE user_id = $1 AND profile_id = $2")
        .bind(user_id)
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;
    bulk_insert_programmes(&mut transaction, user_id, profile_id, &programmes).await?;
    transaction.commit().await?;
    info!(
        job_id = %job_id,
        programme_count = programmes.len(),
        elapsed_ms = programme_write_started_at.elapsed().as_millis() as u64,
        "persisted guide programmes"
    );
    Ok(source_statuses)
}

pub(super) async fn persist_channel_sync_data(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
    categories: &[XtreamCategory],
    channels: &[XtreamChannel],
) -> Result<ChannelSyncDelta> {
    let persist_started_at = Instant::now();
    let mut transaction = pool.begin().await?;
    let existing_categories =
        load_persisted_categories(&mut transaction, user_id, profile_id).await?;
    let existing_channels =
        load_persisted_channels_for_sync(&mut transaction, user_id, profile_id).await?;
    let deduped_categories = dedupe_categories(categories);
    let deduped_channels = dedupe_channels(channels);
    let changed_category_remote_ids =
        changed_category_remote_ids(&existing_categories, &deduped_categories);
    let channel_delta = determine_channel_sync_delta(
        &existing_channels,
        &deduped_channels,
        &changed_category_remote_ids,
    );

    bulk_upsert_categories(&mut transaction, user_id, profile_id, categories).await?;
    bulk_upsert_channels(&mut transaction, user_id, profile_id, channels).await?;
    delete_stale_channels(
        &mut transaction,
        user_id,
        profile_id,
        &deduped_channels
            .iter()
            .map(|channel| channel.remote_stream_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    transaction.commit().await?;
    info!(
        job_id = %job_id,
        category_count = categories.len(),
        channel_count = channels.len(),
        elapsed_ms = persist_started_at.elapsed().as_millis() as u64,
        "persisted provider categories and channels"
    );
    Ok(channel_delta)
}

pub(super) async fn persist_epg_sync_data(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
    job_type: &str,
    feeds: &[FetchedEpgFeed],
) -> Result<Vec<EpgSourceSyncStatus>> {
    let persist_started_at = Instant::now();
    let mut transaction = pool.begin().await?;
    let persisted_channels = load_persisted_channels(&mut transaction, user_id, profile_id).await?;
    let channel_lookup = build_channel_lookup_index(&persisted_channels);
    let programme_resolution_started_at = Instant::now();
    let (programmes, source_statuses) = resolve_epg_programmes(feeds, &channel_lookup);
    info!(
        job_id = %job_id,
        programme_count = programmes.len(),
        elapsed_ms = programme_resolution_started_at.elapsed().as_millis() as u64,
        "resolved EPG programmes against persisted channels"
    );

    update_sync_job_phase(
        pool,
        job_id,
        "saving-programs",
        3,
        job_type,
        "Saving guide entries",
    )
    .await?;
    let programme_write_started_at = Instant::now();
    sqlx::query("DELETE FROM programs WHERE user_id = $1 AND profile_id = $2")
        .bind(user_id)
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;
    bulk_insert_programmes(&mut transaction, user_id, profile_id, &programmes).await?;
    transaction.commit().await?;
    info!(
        job_id = %job_id,
        programme_count = programmes.len(),
        persisted_channel_count = persisted_channels.len(),
        total_elapsed_ms = persist_started_at.elapsed().as_millis() as u64,
        write_elapsed_ms = programme_write_started_at.elapsed().as_millis() as u64,
        "persisted guide programmes"
    );
    Ok(source_statuses)
}

pub(super) async fn update_epg_source_statuses(
    pool: &PgPool,
    statuses: &[EpgSourceSyncStatus],
) -> Result<()> {
    for status in statuses {
        sqlx::query(
            r#"
            UPDATE epg_sources
            SET
              last_sync_at = CASE WHEN $2 THEN NOW() ELSE last_sync_at END,
              last_sync_error = $3,
              last_program_count = $4,
              last_matched_count = $5,
              updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(status.source_id)
        .bind(status.mark_synced)
        .bind(&status.last_sync_error)
        .bind(status.last_program_count)
        .bind(status.last_matched_count)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn bulk_upsert_categories(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    categories: &[XtreamCategory],
) -> Result<()> {
    let deduped_categories = categories
        .iter()
        .cloned()
        .fold(HashMap::new(), |mut categories_by_remote_id, category| {
            categories_by_remote_id.insert(category.remote_category_id.clone(), category);
            categories_by_remote_id
        })
        .into_values()
        .collect::<Vec<_>>();

    for chunk in deduped_categories.chunks(SYNC_BATCH_SIZE) {
        let remote_category_ids = chunk
            .iter()
            .map(|category| category.remote_category_id.clone())
            .collect::<Vec<_>>();
        let names = chunk
            .iter()
            .map(|category| category.name.clone())
            .collect::<Vec<_>>();

        sqlx::query(
            r#"
            WITH input AS (
              SELECT *
              FROM UNNEST($3::text[], $4::text[]) AS input(remote_category_id, name)
            )
            INSERT INTO channel_categories (user_id, profile_id, remote_category_id, name)
            SELECT $1, $2, input.remote_category_id, input.name
            FROM input
            ON CONFLICT (user_id, profile_id, remote_category_id)
            DO UPDATE SET name = EXCLUDED.name
            WHERE channel_categories.name IS DISTINCT FROM EXCLUDED.name
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(&remote_category_ids)
        .bind(&names)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn bulk_upsert_channels(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    channels: &[XtreamChannel],
) -> Result<()> {
    let deduped_channels = channels
        .iter()
        .cloned()
        .fold(HashMap::new(), |mut channels_by_stream_id, channel| {
            channels_by_stream_id.insert(channel.remote_stream_id, channel);
            channels_by_stream_id
        })
        .into_values()
        .collect::<Vec<_>>();

    for chunk in deduped_channels.chunks(SYNC_BATCH_SIZE) {
        let remote_stream_ids = chunk
            .iter()
            .map(|channel| channel.remote_stream_id)
            .collect::<Vec<_>>();
        let names = chunk
            .iter()
            .map(|channel| channel.name.clone())
            .collect::<Vec<_>>();
        let logo_urls = chunk
            .iter()
            .map(|channel| channel.logo_url.clone())
            .collect::<Vec<_>>();
        let category_remote_ids = chunk
            .iter()
            .map(|channel| channel.category_id.clone())
            .collect::<Vec<_>>();
        let has_catchup = chunk
            .iter()
            .map(|channel| channel.has_catchup)
            .collect::<Vec<_>>();
        let archive_duration_hours = chunk
            .iter()
            .map(|channel| channel.archive_duration_hours)
            .collect::<Vec<_>>();
        let stream_extensions = chunk
            .iter()
            .map(|channel| channel.stream_extension.clone())
            .collect::<Vec<_>>();
        let epg_channel_ids = chunk
            .iter()
            .map(|channel| channel.epg_channel_id.clone())
            .collect::<Vec<_>>();

        sqlx::query(
            r#"
            WITH input AS (
              SELECT *
              FROM UNNEST(
                $3::int4[],
                $4::text[],
                $5::text[],
                $6::text[],
                $7::bool[],
                $8::int4[],
                $9::text[],
                $10::text[]
              ) AS input(
                remote_stream_id,
                name,
                logo_url,
                category_remote_id,
                has_catchup,
                archive_duration_hours,
                stream_extension,
                epg_channel_id
              )
            )
            INSERT INTO channels (
              user_id,
              profile_id,
              category_id,
              remote_stream_id,
              epg_channel_id,
              name,
              logo_url,
              has_catchup,
              archive_duration_hours,
              stream_extension,
              updated_at
            )
            SELECT
              $1,
              $2,
              cc.id,
              input.remote_stream_id,
              input.epg_channel_id,
              input.name,
              input.logo_url,
              input.has_catchup,
              input.archive_duration_hours,
              input.stream_extension,
              NOW()
            FROM input
            LEFT JOIN channel_categories cc
              ON cc.user_id = $1
             AND cc.profile_id = $2
             AND cc.remote_category_id = input.category_remote_id
            ON CONFLICT (user_id, profile_id, remote_stream_id)
            DO UPDATE SET
              category_id = EXCLUDED.category_id,
              epg_channel_id = EXCLUDED.epg_channel_id,
              name = EXCLUDED.name,
              logo_url = EXCLUDED.logo_url,
              has_catchup = EXCLUDED.has_catchup,
              archive_duration_hours = EXCLUDED.archive_duration_hours,
              stream_extension = EXCLUDED.stream_extension,
              updated_at = NOW()
            WHERE channels.category_id IS DISTINCT FROM EXCLUDED.category_id
               OR channels.epg_channel_id IS DISTINCT FROM EXCLUDED.epg_channel_id
               OR channels.name IS DISTINCT FROM EXCLUDED.name
               OR channels.logo_url IS DISTINCT FROM EXCLUDED.logo_url
               OR channels.has_catchup IS DISTINCT FROM EXCLUDED.has_catchup
               OR channels.archive_duration_hours IS DISTINCT FROM EXCLUDED.archive_duration_hours
               OR channels.stream_extension IS DISTINCT FROM EXCLUDED.stream_extension
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(&remote_stream_ids)
        .bind(&names)
        .bind(&logo_urls)
        .bind(&category_remote_ids)
        .bind(&has_catchup)
        .bind(&archive_duration_hours)
        .bind(&stream_extensions)
        .bind(&epg_channel_ids)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn load_persisted_channels(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<Vec<PersistedChannelRecord>> {
    let channels = sqlx::query_as::<_, PersistedChannelRecord>(
        r#"
        SELECT
          id,
          name,
          remote_stream_id,
          epg_channel_id,
          has_catchup
        FROM channels
        WHERE user_id = $1 AND profile_id = $2
        ORDER BY updated_at DESC, id DESC
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_all(&mut **transaction)
    .await?;

    Ok(channels)
}

async fn load_persisted_categories(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<Vec<PersistedCategoryRecord>> {
    sqlx::query_as::<_, PersistedCategoryRecord>(
        r#"
        SELECT remote_category_id, name
        FROM channel_categories
        WHERE user_id = $1 AND profile_id = $2
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(Into::into)
}

async fn load_persisted_channels_for_sync(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<Vec<PersistedChannelSyncRow>> {
    sqlx::query_as::<_, PersistedChannelSyncRow>(
        r#"
        SELECT
          c.id,
          c.remote_stream_id,
          cc.remote_category_id AS category_remote_id,
          c.name,
          c.logo_url,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1 AND c.profile_id = $2
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(Into::into)
}

fn dedupe_categories(categories: &[XtreamCategory]) -> Vec<XtreamCategory> {
    categories
        .iter()
        .cloned()
        .fold(HashMap::new(), |mut categories_by_remote_id, category| {
            categories_by_remote_id.insert(category.remote_category_id.clone(), category);
            categories_by_remote_id
        })
        .into_values()
        .collect::<Vec<_>>()
}

fn dedupe_channels(channels: &[XtreamChannel]) -> Vec<XtreamChannel> {
    channels
        .iter()
        .cloned()
        .fold(HashMap::new(), |mut channels_by_stream_id, channel| {
            channels_by_stream_id.insert(channel.remote_stream_id, channel);
            channels_by_stream_id
        })
        .into_values()
        .collect::<Vec<_>>()
}

fn changed_category_remote_ids(
    existing_categories: &[PersistedCategoryRecord],
    incoming_categories: &[XtreamCategory],
) -> HashSet<String> {
    let existing_by_remote_id = existing_categories
        .iter()
        .map(|category| (category.remote_category_id.as_str(), category))
        .collect::<HashMap<_, _>>();

    incoming_categories
        .iter()
        .filter(|category| {
            existing_by_remote_id
                .get(category.remote_category_id.as_str())
                .is_none_or(|existing| existing.name != category.name)
        })
        .map(|category| category.remote_category_id.clone())
        .collect::<HashSet<_>>()
}

fn determine_channel_sync_delta(
    existing_channels: &[PersistedChannelSyncRow],
    incoming_channels: &[XtreamChannel],
    changed_category_remote_ids: &HashSet<String>,
) -> ChannelSyncDelta {
    let existing_by_remote_stream_id = existing_channels
        .iter()
        .map(|channel| (channel.remote_stream_id, channel))
        .collect::<HashMap<_, _>>();
    let incoming_remote_stream_ids = incoming_channels
        .iter()
        .map(|channel| channel.remote_stream_id)
        .collect::<HashSet<_>>();

    let changed_remote_stream_ids = incoming_channels
        .iter()
        .filter(|incoming| {
            existing_by_remote_stream_id
                .get(&incoming.remote_stream_id)
                .is_none_or(|existing| {
                    existing.name != incoming.name
                        || existing.logo_url != incoming.logo_url
                        || existing.category_remote_id != incoming.category_id
                        || existing.epg_channel_id != incoming.epg_channel_id
                        || existing.has_catchup != incoming.has_catchup
                        || existing.archive_duration_hours != incoming.archive_duration_hours
                        || existing.stream_extension != incoming.stream_extension
                        || incoming.category_id.as_ref().is_some_and(|category_id| {
                            changed_category_remote_ids.contains(category_id)
                        })
                })
        })
        .map(|channel| channel.remote_stream_id)
        .collect::<Vec<_>>();

    let removed_channel_ids = existing_channels
        .iter()
        .filter(|channel| !incoming_remote_stream_ids.contains(&channel.remote_stream_id))
        .map(|channel| channel.id)
        .collect::<Vec<_>>();

    ChannelSyncDelta {
        changed_remote_stream_ids,
        removed_channel_ids,
    }
}

async fn delete_stale_channels(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    active_remote_stream_ids: &[i32],
) -> Result<()> {
    if active_remote_stream_ids.is_empty() {
        sqlx::query("DELETE FROM channels WHERE user_id = $1 AND profile_id = $2")
            .bind(user_id)
            .bind(profile_id)
            .execute(&mut **transaction)
            .await?;
        return Ok(());
    }

    sqlx::query(
        r#"
        DELETE FROM channels
        WHERE user_id = $1
          AND profile_id = $2
          AND NOT (remote_stream_id = ANY($3))
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .bind(active_remote_stream_ids)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

fn build_channel_lookup_index(channels: &[PersistedChannelRecord]) -> ChannelLookupIndex {
    let mut lookup = ChannelLookupIndex::default();
    let mut ambiguous_simplified_names = HashSet::new();

    for channel in channels {
        let resolution = ChannelResolution {
            channel_id: channel.id,
            channel_name: channel.name.clone(),
            has_catchup: channel.has_catchup,
        };
        if let Some(epg_channel_id) = channel
            .epg_channel_id
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            lookup
                .epg_channel_ids
                .entry(epg_channel_id.clone())
                .or_insert_with(|| resolution.clone());
        }
        lookup
            .remote_stream_ids
            .entry(channel.remote_stream_id.to_string())
            .or_insert_with(|| resolution.clone());
        let normalized_name = normalize_channel_name(&channel.name);
        if !normalized_name.is_empty() {
            lookup
                .normalized_names
                .entry(normalized_name)
                .or_insert_with(|| resolution.clone());
        }
        let simplified_name = simplify_channel_name(&channel.name);
        if !simplified_name.is_empty() {
            insert_unique_channel_alias(
                &mut lookup.simplified_names,
                &mut ambiguous_simplified_names,
                simplified_name,
                resolution,
            );
        }
    }

    lookup
}

fn normalize_channel_name(value: &str) -> String {
    channel_name_tokens(value).join("")
}

fn simplify_channel_name(value: &str) -> String {
    channel_name_tokens(value)
        .into_iter()
        .filter(|token| !is_channel_noise_token(token))
        .collect::<Vec<_>>()
        .join("")
}

fn channel_name_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in normalize_channel_text(value)
        .chars()
        .flat_map(|character| character.to_lowercase())
    {
        if character.is_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    trim_channel_region_tokens(tokens)
}

fn normalize_channel_text(value: &str) -> String {
    value
        .replace("áµá´´á´°", "UHD")
        .replace("á¶ á´´á´°", "FHD")
        .replace("á´´á´°", "HD")
        .replace("Ë¢á´°", "SD")
        .replace("â´á´·", "4K")
}

fn trim_channel_region_tokens(mut tokens: Vec<String>) -> Vec<String> {
    while tokens
        .first()
        .map(|token| is_channel_region_token(token))
        .unwrap_or(false)
    {
        tokens.remove(0);
    }

    while tokens
        .last()
        .map(|token| is_channel_region_token(token))
        .unwrap_or(false)
    {
        tokens.pop();
    }

    tokens
}

fn is_channel_region_token(token: &str) -> bool {
    matches!(token, "se" | "swe" | "sweden")
}

fn is_channel_noise_token(token: &str) -> bool {
    matches!(
        token,
        "hd" | "uhd" | "fhd" | "sd" | "4k" | "text" | "multi" | "sub" | "audio" | "dub" | "dubbed"
    )
}

fn insert_unique_channel_alias(
    aliases: &mut HashMap<String, ChannelResolution>,
    ambiguous_aliases: &mut HashSet<String>,
    alias: String,
    resolution: ChannelResolution,
) {
    if ambiguous_aliases.contains(&alias) {
        return;
    }

    match aliases.get(&alias) {
        None => {
            aliases.insert(alias, resolution);
        }
        Some(existing) if existing.channel_id == resolution.channel_id => {}
        Some(_) => {
            aliases.remove(&alias);
            ambiguous_aliases.insert(alias);
        }
    }
}

fn resolve_channel_for_programme(
    programme: &XmltvProgramme,
    channels: &HashMap<String, XmltvChannel>,
    lookup: &ChannelLookupIndex,
) -> Option<ChannelResolution> {
    if let Some(channel) = lookup.epg_channel_ids.get(&programme.channel_key) {
        return Some(channel.clone());
    }

    if let Some(channel) = lookup.remote_stream_ids.get(&programme.channel_key) {
        return Some(channel.clone());
    }

    let display_names = channels
        .get(&programme.channel_key)
        .map(|channel| channel.display_names.as_slice())
        .unwrap_or(&[]);
    for display_name in display_names {
        let normalized_name = normalize_channel_name(display_name);
        if normalized_name.is_empty() {
            continue;
        }

        if let Some(channel) = lookup.normalized_names.get(&normalized_name) {
            return Some(channel.clone());
        }

        let simplified_name = simplify_channel_name(display_name);
        if simplified_name.is_empty() {
            continue;
        }

        if let Some(channel) = lookup.simplified_names.get(&simplified_name) {
            return Some(channel.clone());
        }
    }

    None
}

fn resolve_epg_programmes(
    feeds: &[FetchedEpgFeed],
    lookup: &ChannelLookupIndex,
) -> (Vec<ResolvedProgramme>, Vec<EpgSourceSyncStatus>) {
    let mut selected_slots = HashSet::new();
    let mut resolved_programmes = Vec::new();
    let mut source_statuses = Vec::new();

    for feed in feeds {
        let mut matched_count = 0i32;
        for programme in &feed.feed.programmes {
            let Some(channel) =
                resolve_channel_for_programme(programme, &feed.feed.channels, lookup)
            else {
                continue;
            };
            matched_count += 1;

            let slot_key = (
                channel.channel_id,
                programme.start_at.timestamp(),
                programme.end_at.timestamp(),
            );
            if !selected_slots.insert(slot_key) {
                continue;
            }

            resolved_programmes.push(ResolvedProgramme {
                channel_id: channel.channel_id,
                channel_name: channel.channel_name,
                title: programme.title.clone(),
                description: programme.description.clone(),
                start_at: programme.start_at,
                end_at: programme.end_at,
                can_catchup: channel.has_catchup,
            });
        }

        info!(
            source_kind = %feed.source_kind,
            source = %feed.source_label,
            programme_count = feed.feed.programmes.len(),
            matched_count,
            "resolved EPG feed against channel catalog"
        );

        if let Some(source_id) = feed.source_id {
            source_statuses.push(EpgSourceSyncStatus {
                source_id,
                last_sync_error: None,
                last_program_count: Some(feed.feed.programmes.len() as i32),
                last_matched_count: Some(matched_count),
                mark_synced: true,
            });
        }
    }

    resolved_programmes.sort_by_key(|programme| {
        (
            programme.channel_name.clone(),
            programme.start_at.timestamp(),
            programme.end_at.timestamp(),
        )
    });

    (resolved_programmes, source_statuses)
}

async fn bulk_insert_programmes(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    programmes: &[ResolvedProgramme],
) -> Result<()> {
    for chunk in programmes.chunks(SYNC_BATCH_SIZE) {
        let channel_ids = chunk
            .iter()
            .map(|programme| programme.channel_id)
            .collect::<Vec<_>>();
        let channel_names = chunk
            .iter()
            .map(|programme| programme.channel_name.clone())
            .collect::<Vec<_>>();
        let titles = chunk
            .iter()
            .map(|programme| programme.title.clone())
            .collect::<Vec<_>>();
        let descriptions = chunk
            .iter()
            .map(|programme| programme.description.clone())
            .collect::<Vec<_>>();
        let start_times = chunk
            .iter()
            .map(|programme| programme.start_at)
            .collect::<Vec<_>>();
        let end_times = chunk
            .iter()
            .map(|programme| programme.end_at)
            .collect::<Vec<_>>();
        let can_catchup = chunk
            .iter()
            .map(|programme| programme.can_catchup)
            .collect::<Vec<_>>();

        sqlx::query(
            r#"
            WITH input AS (
              SELECT *
              FROM UNNEST(
                $3::uuid[],
                $4::text[],
                $5::text[],
                $6::text[],
                $7::timestamptz[],
                $8::timestamptz[],
                $9::bool[]
              ) AS input(channel_id, channel_name, title, description, start_at, end_at, can_catchup)
            )
            INSERT INTO programs (
              user_id,
              profile_id,
              channel_id,
              channel_name,
              title,
              description,
              start_at,
              end_at,
              can_catchup
            )
            SELECT
              $1,
              $2,
              input.channel_id,
              input.channel_name,
              input.title,
              input.description,
              input.start_at,
              input.end_at,
              input.can_catchup
            FROM input
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(&channel_ids)
        .bind(&channel_names)
        .bind(&titles)
        .bind(&descriptions)
        .bind(&start_times)
        .bind(&end_times)
        .bind(&can_catchup)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_phases_for_job_supports_channel_syncs() {
        assert_eq!(total_phases_for_job("channels"), CHANNEL_SYNC_TOTAL_PHASES);
        assert_eq!(total_phases_for_job("epg"), EPG_SYNC_TOTAL_PHASES);
        assert_eq!(total_phases_for_job("full"), FULL_SYNC_TOTAL_PHASES);
    }

    fn sample_persisted_channel_sync_row(
        remote_stream_id: i32,
        category_remote_id: Option<&str>,
        name: &str,
    ) -> PersistedChannelSyncRow {
        PersistedChannelSyncRow {
            id: Uuid::from_u128(remote_stream_id as u128),
            remote_stream_id,
            category_remote_id: category_remote_id.map(str::to_string),
            name: name.to_string(),
            logo_url: Some(format!("https://example.com/{remote_stream_id}.png")),
            epg_channel_id: Some(format!("epg-{remote_stream_id}")),
            has_catchup: false,
            archive_duration_hours: None,
            stream_extension: Some("m3u8".to_string()),
        }
    }

    fn sample_xtream_channel(
        remote_stream_id: i32,
        category_id: Option<&str>,
        name: &str,
    ) -> XtreamChannel {
        XtreamChannel {
            remote_stream_id,
            name: name.to_string(),
            logo_url: Some(format!("https://example.com/{remote_stream_id}.png")),
            category_id: category_id.map(str::to_string),
            epg_channel_id: Some(format!("epg-{remote_stream_id}")),
            has_catchup: false,
            archive_duration_hours: None,
            stream_extension: Some("m3u8".to_string()),
        }
    }

    #[test]
    fn channel_sync_delta_only_marks_changed_and_removed_channels() {
        let existing = vec![
            sample_persisted_channel_sync_row(1, Some("sports"), "Sports 1"),
            sample_persisted_channel_sync_row(2, Some("news"), "News 1"),
            sample_persisted_channel_sync_row(3, Some("kids"), "Kids 1"),
        ];
        let incoming = vec![
            sample_xtream_channel(1, Some("sports"), "Sports 1"),
            sample_xtream_channel(2, Some("news"), "News 2"),
            sample_xtream_channel(4, Some("movies"), "Movies 1"),
        ];

        let delta = determine_channel_sync_delta(&existing, &incoming, &HashSet::new());

        assert_eq!(delta.changed_remote_stream_ids, vec![2, 4]);
        assert_eq!(delta.removed_channel_ids, vec![Uuid::from_u128(3)]);
    }

    #[test]
    fn channel_sync_delta_marks_channels_when_their_category_changes() {
        let existing = vec![sample_persisted_channel_sync_row(
            1,
            Some("sports"),
            "Sports 1",
        )];
        let incoming = vec![sample_xtream_channel(1, Some("sports"), "Sports 1")];
        let changed_categories = HashSet::from(["sports".to_string()]);

        let delta = determine_channel_sync_delta(&existing, &incoming, &changed_categories);

        assert_eq!(delta.changed_remote_stream_ids, vec![1]);
        assert!(delta.removed_channel_ids.is_empty());
    }

    #[test]
    fn resolves_external_epg_programmes_by_xmltv_display_name() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(11),
            name: "TV4 HD".to_string(),
            remote_stream_id: 4,
            epg_channel_id: None,
            has_catchup: true,
        }]);
        let feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(12)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/tv.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::from([(
                    "external-tv4".to_string(),
                    XmltvChannel {
                        id: "external-tv4".to_string(),
                        display_names: vec!["TV4 HD".to_string()],
                    },
                )]),
                programmes: vec![XmltvProgramme {
                    channel_key: "external-tv4".to_string(),
                    title: "Morning Show".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(1),
                }],
            },
        };

        let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].channel_name, "TV4 HD");
        assert_eq!(programmes[0].title, "Morning Show");
        assert_eq!(statuses[0].last_matched_count, Some(1));
    }

    #[test]
    fn resolves_external_epg_programmes_with_region_and_quality_decorations() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(13),
            name: "|SE|TV4 á´´á´° SE".to_string(),
            remote_stream_id: 41,
            epg_channel_id: None,
            has_catchup: true,
        }]);
        let feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(14)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/tv4.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::from([(
                    "tv4.se".to_string(),
                    XmltvChannel {
                        id: "tv4.se".to_string(),
                        display_names: vec!["TV4 HD.se".to_string()],
                    },
                )]),
                programmes: vec![XmltvProgramme {
                    channel_key: "tv4.se".to_string(),
                    title: "Evening News".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(1),
                }],
            },
        };

        let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].channel_name, "|SE|TV4 á´´á´° SE");
        assert_eq!(programmes[0].title, "Evening News");
        assert_eq!(statuses[0].last_matched_count, Some(1));
    }

    #[test]
    fn resolves_external_epg_programmes_when_feed_uses_text_variant_names() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(15),
            name: "|SE|TV4 FAKTA".to_string(),
            remote_stream_id: 42,
            epg_channel_id: None,
            has_catchup: false,
        }]);
        let feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(16)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/tv4fakta.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::from([(
                    "tv4-fakta.se".to_string(),
                    XmltvChannel {
                        id: "tv4-fakta.se".to_string(),
                        display_names: vec!["TV4 Fakta - Text.se".to_string()],
                    },
                )]),
                programmes: vec![XmltvProgramme {
                    channel_key: "tv4-fakta.se".to_string(),
                    title: "Documentary Hour".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(1),
                }],
            },
        };

        let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].channel_name, "|SE|TV4 FAKTA");
        assert_eq!(programmes[0].title, "Documentary Hour");
        assert_eq!(statuses[0].last_matched_count, Some(1));
    }

    #[test]
    fn keeps_higher_priority_epg_source_when_timeslots_overlap() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(21),
            name: "Arena 1".to_string(),
            remote_stream_id: 1,
            epg_channel_id: Some("arena.1".to_string()),
            has_catchup: true,
        }]);
        let primary_feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(22)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/primary.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::new(),
                programmes: vec![XmltvProgramme {
                    channel_key: "arena.1".to_string(),
                    title: "Primary Listing".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(2),
                }],
            },
        };
        let fallback_feed = FetchedEpgFeed {
            source_id: None,
            source_kind: "xtream".to_string(),
            source_label: "https://provider.example.com/xmltv.php".to_string(),
            priority: 1,
            feed: XmltvFeed {
                channels: HashMap::new(),
                programmes: vec![XmltvProgramme {
                    channel_key: "arena.1".to_string(),
                    title: "Fallback Listing".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(2),
                }],
            },
        };

        let (programmes, statuses) =
            resolve_epg_programmes(&[primary_feed, fallback_feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].title, "Primary Listing");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].last_program_count, Some(1));
    }
}
