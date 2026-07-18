use super::*;

pub async fn run() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let config = Arc::new(Config::from_env()?);
    let pool = wait_for_postgres(&config.database_url).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;
    let recovery = recover_interrupted_syncs(&pool).await?;
    if recovery.recovered_jobs > 0 || recovery.recovered_profiles > 0 {
        warn!(
            "recovered {} interrupted sync job(s) and {} syncing provider profile(s)",
            recovery.recovered_jobs, recovery.recovered_profiles
        );
    }

    if config.daily_sync_hour_local > 23 {
        return Err(anyhow!(
            "APP_DAILY_SYNC_HOUR_LOCAL must be between 0 and 23"
        ));
    }

    let state = AppState {
        pool,
        config,
        provider_http_client: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?,
        relay_http_client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(RELAY_UPSTREAM_CONNECT_TIMEOUT_SECONDS))
            .read_timeout(Duration::from_secs(RELAY_UPSTREAM_READ_TIMEOUT_SECONDS))
            .build()?,
        user_database_locks: Arc::new(DashMap::new()),
        session_cache: Arc::new(DashMap::new()),
        relay_profile_cache: Arc::new(DashMap::new()),
        channel_visibility_cache: Arc::new(DashMap::new()),
        receiver_channels: Arc::new(DashMap::new()),
        cast_transcodes: Arc::new(Mutex::new(transcode::CastTranscodeManager::default())),
    };

    let periodic_state = state.clone();
    sync::spawn_periodic_sync_worker(periodic_state);
    transcode::spawn_reaper(state.clone());

    let bind_address: SocketAddr = state.config.bind_address;
    let cors = build_cors_layer(&state.config)?;
    let router_state = state.clone();
    if let Some(public_origin) = state.config.public_origin.as_ref() {
        info!("Browser public origin configured as {public_origin}");
    }
    let app = Router::new()
        .route("/health", get(health))
        .nest("/api", browser_api_router())
        .with_state(router_state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    info!("Euripus server listening on {bind_address}");
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    transcode::stop_all(&state).await;
    state.pool.close().await;
    Ok(())
}

pub(super) async fn wait_for_postgres(database_url: &str) -> Result<PgPool> {
    let startup_deadline = tokio::time::Instant::now() + DATABASE_STARTUP_TIMEOUT;
    let mut retry_delay = DATABASE_RETRY_DELAY_INITIAL;
    let mut attempt = 1;

    loop {
        let connect_future = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url);
        match tokio::time::timeout(DATABASE_CONNECT_TIMEOUT, connect_future).await {
            Ok(Ok(pool)) => {
                if attempt > 1 {
                    info!("connected to PostgreSQL on startup attempt {attempt}");
                }
                return Ok(pool);
            }
            Ok(Err(error)) => {
                if tokio::time::Instant::now() >= startup_deadline {
                    return Err(error)
                        .context("failed to connect to PostgreSQL before startup timeout");
                }
                warn!(
                    "PostgreSQL is not ready yet on startup attempt {attempt}: {error}. Retrying in {}s",
                    retry_delay.as_secs()
                );
            }
            Err(_) => {
                if tokio::time::Instant::now() >= startup_deadline {
                    return Err(anyhow!(
                        "timed out while connecting to PostgreSQL before startup timeout"
                    ));
                }
                warn!(
                    "PostgreSQL connection attempt {attempt} timed out after {}s. Retrying in {}s",
                    DATABASE_CONNECT_TIMEOUT.as_secs(),
                    retry_delay.as_secs()
                );
            }
        }

        tokio::time::sleep(retry_delay).await;
        retry_delay = std::cmp::min(retry_delay * 2, DATABASE_RETRY_DELAY_MAX);
        attempt += 1;
    }
}

pub(super) async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub(super) async fn shutdown_signal() {
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = signal::ctrl_c() => {}
        _ = terminate => {}
    }

    info!("shutdown signal received, draining server and closing PostgreSQL pool");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InterruptedSyncRecovery {
    recovered_jobs: u64,
    recovered_profiles: u64,
}

async fn recover_interrupted_syncs(pool: &PgPool) -> Result<InterruptedSyncRecovery> {
    let recovered_jobs = sqlx::query(
        r#"
        UPDATE sync_jobs
        SET
          status = 'failed',
          finished_at = NOW(),
          current_phase = 'failed',
          phase_message = $1,
          error_message = $1
        WHERE status IN ('queued', 'running')
        "#,
    )
    .bind(INTERRUPTED_SYNC_MESSAGE)
    .execute(pool)
    .await?
    .rows_affected();

    let recovered_profiles = sqlx::query(
        r#"
        UPDATE provider_profiles
        SET
          status = 'error',
          last_sync_error = $1,
          updated_at = NOW()
        WHERE status = 'syncing'
        "#,
    )
    .bind(INTERRUPTED_SYNC_MESSAGE)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(InterruptedSyncRecovery {
        recovered_jobs,
        recovered_profiles,
    })
}

pub(super) fn shared_api_router() -> Router<AppState> {
    Router::new()
        .route("/server/network", get(get_server_network_status))
        .merge(admin::browser_router())
        .merge(auth::shared_router())
        .merge(provider::shared_router())
        .merge(guide::shared_router())
        .merge(google_calendar::router())
        .merge(on_demand::shared_router())
        .merge(search::shared_router())
        .merge(sports::shared_router())
        .merge(playback::shared_router())
        .merge(receiver::shared_router())
        .merge(sync::shared_router())
}

pub(super) fn browser_api_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .merge(auth::browser_router())
        .merge(receiver::browser_router())
        .merge(relay::router())
        .merge(transcode::router())
        .merge(shared_api_router())
}

pub(super) fn build_cors_layer(config: &Config) -> Result<CorsLayer> {
    let allowed_origins = config
        .allowed_origins
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin).with_context(|| {
                format!("APP_ALLOWED_ORIGINS contains an invalid origin: {origin}")
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(CorsLayer::new()
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static(CSRF_HEADER_NAME),
        ])
        .allow_origin(AllowOrigin::list(allowed_origins)))
}
