use super::receiver::ReceiverEventPayload;
use super::*;

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) pool: PgPool,
    pub(super) config: Arc<Config>,
    pub(super) provider_http_client: reqwest::Client,
    pub(super) relay_http_client: reqwest::Client,
    pub(super) user_database_locks: Arc<DashMap<Uuid, Arc<Mutex<()>>>>,
    pub(super) session_cache: Arc<DashMap<(Uuid, Uuid), Instant>>,
    pub(super) relay_profile_cache: Arc<DashMap<(Uuid, Uuid), Instant>>,
    pub(super) channel_visibility_cache:
        Arc<DashMap<(Uuid, Option<Uuid>), CachedChannelVisibilityMap>>,
    pub(super) receiver_channels: Arc<DashMap<Uuid, broadcast::Sender<ReceiverEventPayload>>>,
    pub(super) cast_transcodes: Arc<Mutex<super::transcode::CastTranscodeManager>>,
}

impl AppState {
    pub(super) fn user_database_lock(&self, user_id: Uuid) -> Arc<Mutex<()>> {
        self.user_database_locks
            .entry(user_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[derive(Debug, Clone)]
pub(super) struct CachedChannelVisibilityMap {
    pub(super) values: Arc<HashMap<Uuid, ChannelVisibility>>,
    pub(super) expires_at: Instant,
}
