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
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE status = 'valid'
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    for profile in profiles {
        if has_recent_sync_job(&state.pool, profile.id, PERIODIC_CHANNEL_SYNC_INTERVAL).await? {
            continue;
        }

        match ensure_no_active_sync(&state.pool, profile.id).await {
            Ok(()) => {}
            Err(AppError::BadRequest(_)) => continue,
            Err(other) => return Err(anyhow!("failed to inspect active syncs: {other:?}")),
        }

        let job = insert_sync_job(
            &state.pool,
            profile.user_id,
            profile.id,
            "channels",
            "scheduled",
        )
        .await?;

        sync::spawn_sync_job(state.clone(), profile.user_id, profile.id, job.id);
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
