use super::*;

pub(super) fn spawn_periodic_sync_worker(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(PERIODIC_CHANNEL_SYNC_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(error) = queue_scheduled_channel_syncs(state.clone()).await {
                error!("periodic sync worker failed: {error:?}");
            }
        }
    })
}

async fn queue_scheduled_channel_syncs(state: AppState) -> Result<()> {
    let profiles = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          p.id, p.user_id, p.provider_type, p.label, p.base_url, p.username, p.password_encrypted, p.output_format, p.playback_mode,
          p.status, p.last_validated_at, p.last_sync_at, p.last_sync_error, p.created_at, p.updated_at
        FROM provider_profiles p
        JOIN users u ON u.id = p.user_id
        WHERE p.status = 'valid'
        ORDER BY
          p.user_id,
          CASE WHEN p.id = u.live_provider_id THEN 0 ELSE 1 END,
          p.updated_at DESC,
          p.created_at DESC,
          p.id
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    for profile in profiles {
        if has_recent_sync_job(&state.pool, profile.id, PERIODIC_CHANNEL_SYNC_INTERVAL).await? {
            continue;
        }

        let job = match insert_sync_job(
            &state.pool,
            profile.user_id,
            profile.id,
            "channels",
            "scheduled",
        )
        .await
        {
            Ok(job) => job,
            Err(AppError::BadRequest(_)) => continue,
            Err(other) => return Err(anyhow!("failed to queue scheduled sync: {other:?}")),
        };

        let profile_id = profile.id;
        let job_id = job.id;
        if let Err(error) =
            sync::spawn_sync_job(state.clone(), profile.user_id, profile_id, job_id).await
        {
            error!(
                %profile_id,
                %job_id,
                "scheduled sync task stopped unexpectedly: {error}"
            );
        }
    }

    Ok(())
}

async fn has_recent_sync_job(pool: &PgPool, profile_id: Uuid, interval: Duration) -> Result<bool> {
    let recent_job_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM sync_jobs
        WHERE profile_id = $1
          AND created_at >= NOW() - ($2 * INTERVAL '1 second')
        "#,
    )
    .bind(profile_id)
    .bind(interval.as_secs() as i64)
    .fetch_one(pool)
    .await?;

    Ok(recent_job_count > 0)
}
