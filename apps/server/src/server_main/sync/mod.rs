use super::*;

mod persistence;
mod runner;
mod scheduler;

#[derive(Debug, Clone)]
pub(super) struct ChannelSyncDelta {
    pub(super) changed_remote_stream_ids: Vec<i32>,
    pub(super) removed_channel_ids: Vec<Uuid>,
    pub(super) removed_program_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub(super) struct ChannelResolution {
    pub(super) channel_id: Uuid,
    pub(super) channel_name: String,
    pub(super) has_catchup: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ChannelLookupIndex {
    pub(super) epg_channel_ids: HashMap<String, ChannelResolution>,
    pub(super) remote_stream_ids: HashMap<String, ChannelResolution>,
    pub(super) normalized_names: HashMap<String, ChannelResolution>,
    pub(super) simplified_names: HashMap<String, ChannelResolution>,
}

#[derive(Debug, Clone)]
pub(super) struct FetchedEpgFeed {
    pub(super) source_id: Option<Uuid>,
    pub(super) source_kind: String,
    pub(super) source_label: String,
    pub(super) priority: i32,
    pub(super) feed: XmltvFeed,
}

#[derive(Debug, Clone)]
pub(super) struct EpgSourceSyncStatus {
    pub(super) source_id: Uuid,
    pub(super) last_sync_error: Option<String>,
    pub(super) last_program_count: Option<i32>,
    pub(super) last_matched_count: Option<i32>,
    pub(super) mark_synced: bool,
}

pub(super) enum ExternalEpgFetchResult {
    Success(FetchedEpgFeed),
    Failure(EpgSourceSyncStatus),
}

pub(super) enum EpgFetchResult {
    External(ExternalEpgFetchResult),
    BuiltIn(Result<FetchedEpgFeed>),
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedProgramme {
    pub(super) channel_id: Uuid,
    pub(super) channel_name: String,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) start_at: DateTime<Utc>,
    pub(super) end_at: DateTime<Utc>,
    pub(super) can_catchup: bool,
}

pub(super) fn spawn_periodic_sync_worker(state: AppState) -> JoinHandle<()> {
    scheduler::spawn_periodic_sync_worker(state)
}

pub(super) fn spawn_sync_job(
    state: AppState,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
) -> JoinHandle<()> {
    runner::spawn_sync_job(state, user_id, profile_id, job_id)
}

pub(super) async fn ensure_no_active_sync(pool: &PgPool, profile_id: Uuid) -> Result<(), AppError> {
    persistence::ensure_no_active_sync(pool, profile_id).await
}

pub(super) async fn insert_sync_job(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_type: &str,
    trigger: &str,
) -> std::result::Result<SyncJobResponse, AppError> {
    persistence::insert_sync_job(pool, user_id, profile_id, job_type, trigger).await
}

pub(super) async fn update_sync_job_phase(
    pool: &PgPool,
    job_id: Uuid,
    phase: &str,
    completed_phases: i32,
    job_type: &str,
    phase_message: &str,
) -> Result<()> {
    persistence::update_sync_job_phase(
        pool,
        job_id,
        phase,
        completed_phases,
        job_type,
        phase_message,
    )
    .await
}

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
}
