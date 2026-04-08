use super::*;

pub async fn run() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let config = Arc::new(Config::from_env()?);
    let pool = wait_for_postgres(&config.database_url).await?;

    repair_sqlx_migration_checksums(&pool).await?;

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

    let meili_setup = setup_meilisearch(&config, &pool).await;

    let state = AppState {
        pool,
        config,
        provider_http_client: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?,
        relay_http_client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()?,
        meili: meili_setup.client,
        meili_readiness: Arc::new(RwLock::new(meili_setup.readiness)),
        meili_schema_ready: Arc::new(RwLock::new(meili_setup.schema_ready)),
        meili_bootstrapping_users: Arc::new(DashSet::new()),
        search_lexicons: Arc::new(DashMap::new()),
        session_cache: Arc::new(DashMap::new()),
        relay_profile_cache: Arc::new(DashMap::new()),
        receiver_channels: Arc::new(DashMap::new()),
    };

    if state.meili.is_some() && !meili_setup.schema_ready {
        search::indexing::spawn_meili_startup_worker(state.clone());
    } else if matches!(meili_setup.readiness, MeiliReadiness::Bootstrapping) {
        search::indexing::spawn_meili_bootstrap_worker(state.clone());
    }

    let periodic_state = state.clone();
    sync::spawn_periodic_sync_worker(periodic_state);

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

pub(super) async fn setup_meilisearch(config: &Config, pool: &PgPool) -> MeiliSetup {
    let Some(url) = config.meilisearch_url.as_deref() else {
        return MeiliSetup {
            client: None,
            readiness: MeiliReadiness::Disabled,
            schema_ready: true,
        };
    };
    let client = match MeilisearchClient::new(url, config.meilisearch_api_key.as_deref()) {
        Ok(client) => client,
        Err(error) => {
            warn!(
                "failed to initialize Meilisearch client, falling back to PostgreSQL search: {error:?}"
            );
            return MeiliSetup {
                client: None,
                readiness: MeiliReadiness::Disabled,
                schema_ready: true,
            };
        }
    };

    let startup_result = tokio::time::timeout(MEILI_STARTUP_TIMEOUT, async {
        let schema_ready = search::indexing::inspect_meili_schema_readiness(&client)
            .await
            .unwrap_or_else(|error| {
            warn!(
                "failed to inspect Meilisearch schema version before setup; forcing bootstrap: {error:?}"
            );
            false
        });

        let strategy = ExponentialBackoff::from_millis(500).factor(2).take(4);
        let setup_result = Retry::spawn(strategy, || {
            let client = client.clone();
            let pool = pool.clone();
            async move { search::indexing::configure_meili_indexes(&client, &pool).await }
        })
        .await;

        match setup_result {
            Ok(()) => {
                let readiness = match search::indexing::inspect_meili_readiness(&client, pool, schema_ready).await {
                    Ok(readiness) => readiness,
                    Err(error) => {
                        warn!(
                            "failed to verify Meilisearch index readiness, falling back to PostgreSQL search: {error:?}"
                        );
                    return MeiliSetup {
                        client: Some(Arc::new(client.clone())),
                        readiness: MeiliReadiness::Bootstrapping,
                        schema_ready: false,
                    };
                }
            };
            match readiness {
                    MeiliReadiness::Ready => info!("Meilisearch configured successfully"),
                    MeiliReadiness::Bootstrapping => warn!(
                        "Meilisearch configured but requires bootstrap from PostgreSQL; search will use PostgreSQL until bootstrap completes"
                    ),
                    MeiliReadiness::Disabled => {}
                }
                MeiliSetup {
                    client: Some(Arc::new(client.clone())),
                    readiness,
                    schema_ready,
                }
            }
            Err(error) => {
                warn!(
                    "failed to configure Meilisearch during startup, continuing setup in the background: {error:?}"
                );
                MeiliSetup {
                    client: Some(Arc::new(client.clone())),
                    readiness: MeiliReadiness::Bootstrapping,
                    schema_ready: false,
                }
            }
        }
    })
    .await;

    match startup_result {
        Ok(setup) => setup,
        Err(_) => {
            warn!(
                "Meilisearch startup exceeded {}s, continuing setup in the background",
                MEILI_STARTUP_TIMEOUT.as_secs()
            );
            MeiliSetup {
                client: Some(Arc::new(client)),
                readiness: MeiliReadiness::Bootstrapping,
                schema_ready: false,
            }
        }
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

pub(super) async fn repair_sqlx_migration_checksums(pool: &PgPool) -> Result<()> {
    let migrations_table_exists =
        sqlx::query_scalar::<_, bool>("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await
            .context("failed to check for sqlx migrations table")?;

    if !migrations_table_exists {
        return Ok(());
    }

    let migrations_dir = FsPath::new("./migrations");
    if !migrations_dir.exists() {
        warn!(
            "sqlx migrations directory {} not found, skipping checksum repair",
            migrations_dir.display()
        );
        return Ok(());
    }

    let mut repaired_versions = Vec::new();
    for entry in fs::read_dir(migrations_dir).with_context(|| {
        format!(
            "failed to read migrations directory {}",
            migrations_dir.display()
        )
    })? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("sql") {
            continue;
        }

        let file_name = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => continue,
        };

        let version_str = match file_name.split('_').next() {
            Some(segment) => segment,
            None => continue,
        };

        let version = match version_str.parse::<i64>() {
            Ok(value) => value,
            Err(_) => continue,
        };

        let contents = fs::read(&path)
            .with_context(|| format!("failed to read migration file {}", path.display()))?;
        let checksum = Sha384::digest(&contents).to_vec();

        let updated_rows = sqlx::query(
            r#"
            UPDATE _sqlx_migrations
            SET checksum = $1
            WHERE version = $2 AND success = true AND checksum <> $1
            "#,
        )
        .bind(checksum)
        .bind(version)
        .execute(pool)
        .await
        .with_context(|| format!("failed to repair migration {version:04} checksum"))?
        .rows_affected();

        if updated_rows > 0 {
            repaired_versions.push(version);
        }
    }

    if !repaired_versions.is_empty() {
        repaired_versions.sort_unstable();
        warn!(
            "repaired sqlx migration checksum(s) for version(s): {:?}",
            repaired_versions
        );
    }

    Ok(())
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
        .merge(auth::shared_router())
        .merge(provider::shared_router())
        .merge(guide::shared_router())
        .merge(search::shared_router())
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
