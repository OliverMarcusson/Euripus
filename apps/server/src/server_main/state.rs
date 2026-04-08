use super::receiver::ReceiverEventPayload;
use super::search::lexicon::SearchLexicon;
use super::*;

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) pool: PgPool,
    pub(super) config: Arc<Config>,
    pub(super) provider_http_client: reqwest::Client,
    pub(super) relay_http_client: reqwest::Client,
    pub(super) meili: Option<Arc<MeilisearchClient>>,
    pub(super) meili_readiness: Arc<RwLock<MeiliReadiness>>,
    pub(super) meili_schema_ready: Arc<RwLock<bool>>,
    pub(super) meili_bootstrapping_users: Arc<DashSet<Uuid>>,
    pub(super) search_lexicons: Arc<DashMap<Uuid, Arc<SearchLexicon>>>,
    pub(super) session_cache: Arc<DashMap<(Uuid, Uuid), Instant>>,
    pub(super) relay_profile_cache: Arc<DashMap<(Uuid, Uuid), Instant>>,
    pub(super) receiver_channels: Arc<DashMap<Uuid, broadcast::Sender<ReceiverEventPayload>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MeiliReadiness {
    Disabled,
    Bootstrapping,
    Ready,
}

impl MeiliReadiness {
    pub(super) fn search_status(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Bootstrapping => "indexing",
            Self::Ready => "ready",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SearchIndexCounts {
    pub(super) postgres_channel_documents: i64,
    pub(super) postgres_program_documents: i64,
    pub(super) meili_channel_documents: i64,
    pub(super) meili_program_documents: i64,
}

pub(super) struct MeiliSetup {
    pub(super) client: Option<Arc<MeilisearchClient>>,
    pub(super) readiness: MeiliReadiness,
    pub(super) schema_ready: bool,
}
