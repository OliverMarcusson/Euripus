use super::*;

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/sports/live", get(get_live_events))
        .route("/sports/today", get(get_today_events))
        .route("/sports/upcoming", get(get_upcoming_events))
        .route("/sports/events/{id}", get(get_event_detail))
        .route("/sports/competitions/{slug}", get(get_competition))
        .route("/sports/providers", get(get_providers))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsUpcomingQuery {
    pub(super) hours: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsEventListResponse {
    pub(super) count: usize,
    pub(super) events: Vec<SportsEventResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsCompetitionResponse {
    pub(super) competition: String,
    pub(super) events: Vec<SportsEventResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsProviderCatalogResponse {
    pub(super) count: usize,
    pub(super) providers: Vec<SportsProviderResponse>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsEventResponse {
    pub(super) id: String,
    pub(super) sport: String,
    pub(super) competition: String,
    pub(super) title: String,
    pub(super) start_time: String,
    pub(super) end_time: Option<String>,
    pub(super) status: String,
    pub(super) venue: Option<String>,
    pub(super) round_label: Option<String>,
    pub(super) participants: Option<SportsParticipantsResponse>,
    pub(super) source: Option<String>,
    pub(super) source_url: Option<String>,
    pub(super) watch: SportsWatchResponse,
    pub(super) search_metadata: SportsSearchMetadataResponse,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsParticipantsResponse {
    pub(super) home: Option<String>,
    pub(super) away: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsWatchResponse {
    pub(super) recommended_market: Option<String>,
    pub(super) recommended_provider: Option<String>,
    pub(super) availabilities: Vec<SportsAvailabilityResponse>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsAvailabilityResponse {
    pub(super) market: Option<String>,
    pub(super) provider_family: Option<String>,
    pub(super) provider_label: String,
    pub(super) channel_name: Option<String>,
    pub(super) watch_type: Option<String>,
    pub(super) confidence: Option<f64>,
    pub(super) source: Option<String>,
    pub(super) search_hints: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsSearchMetadataResponse {
    pub(super) queries: Vec<String>,
    pub(super) keywords: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SportsProviderResponse {
    pub(super) family: String,
    pub(super) market: String,
    pub(super) aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsEventListResponse {
    count: usize,
    events: Vec<UpstreamSportsEvent>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsCompetitionResponse {
    competition: String,
    events: Vec<UpstreamSportsEvent>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsProviderCatalogResponse {
    count: usize,
    providers: Vec<UpstreamSportsProvider>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsEvent {
    id: String,
    sport: String,
    competition: String,
    title: String,
    start_time: String,
    end_time: Option<String>,
    status: String,
    venue: Option<String>,
    round_label: Option<String>,
    participants: Option<UpstreamSportsParticipants>,
    source: Option<String>,
    source_url: Option<String>,
    watch: Option<UpstreamSportsWatch>,
    search_metadata: Option<UpstreamSportsSearchMetadata>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsParticipants {
    home: Option<String>,
    away: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsWatch {
    recommended_market: Option<String>,
    recommended_provider: Option<String>,
    #[serde(default)]
    availabilities: Vec<UpstreamSportsAvailability>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsAvailability {
    market: Option<String>,
    provider_family: Option<String>,
    provider_label: String,
    channel_name: Option<String>,
    watch_type: Option<String>,
    confidence: Option<f64>,
    source: Option<String>,
    #[serde(default)]
    search_hints: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsSearchMetadata {
    #[serde(default)]
    queries: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSportsProvider {
    family: String,
    market: String,
    #[serde(default)]
    aliases: Vec<String>,
}

async fn get_live_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<SportsEventListResponse> {
    require_auth(&state, &headers).await?;
    let payload =
        fetch_upstream_json::<UpstreamSportsEventListResponse>(&state, "v1/events/live").await?;
    Ok(Json(SportsEventListResponse {
        count: payload.count,
        events: payload.events.into_iter().map(Into::into).collect(),
    }))
}

async fn get_today_events(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<SportsEventListResponse> {
    require_auth(&state, &headers).await?;
    let payload =
        fetch_upstream_json::<UpstreamSportsEventListResponse>(&state, "v1/events/today").await?;
    Ok(Json(SportsEventListResponse {
        count: payload.count,
        events: payload.events.into_iter().map(Into::into).collect(),
    }))
}

async fn get_upcoming_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SportsUpcomingQuery>,
) -> ApiResult<SportsEventListResponse> {
    require_auth(&state, &headers).await?;
    let hours = query.hours.unwrap_or(72).clamp(1, 336);
    let payload = fetch_upstream_json::<UpstreamSportsEventListResponse>(
        &state,
        &format!("v1/events/upcoming?hours={hours}"),
    )
    .await?;
    Ok(Json(SportsEventListResponse {
        count: payload.count,
        events: payload.events.into_iter().map(Into::into).collect(),
    }))
}

async fn get_event_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<SportsEventResponse> {
    require_auth(&state, &headers).await?;
    let payload =
        fetch_upstream_json::<UpstreamSportsEvent>(&state, &format!("v1/events/{id}")).await?;
    Ok(Json(payload.into()))
}

async fn get_competition(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> ApiResult<SportsCompetitionResponse> {
    require_auth(&state, &headers).await?;
    let payload = fetch_upstream_json::<UpstreamSportsCompetitionResponse>(
        &state,
        &format!("v1/competitions/{slug}"),
    )
    .await?;
    Ok(Json(SportsCompetitionResponse {
        competition: payload.competition,
        events: payload.events.into_iter().map(Into::into).collect(),
    }))
}

async fn get_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<SportsProviderCatalogResponse> {
    require_auth(&state, &headers).await?;
    let payload =
        fetch_upstream_json::<UpstreamSportsProviderCatalogResponse>(&state, "v1/providers")
            .await?;
    Ok(Json(SportsProviderCatalogResponse {
        count: payload.count,
        providers: payload.providers.into_iter().map(Into::into).collect(),
    }))
}

async fn fetch_upstream_json<T>(state: &AppState, path: &str) -> Result<T, AppError>
where
    T: serde::de::DeserializeOwned,
{
    let url = sports_url(state, path)?;
    let response = state
        .provider_http_client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| {
            AppError::BadGateway(format!(
                "Sports API request failed for {}: {error}",
                url.path()
            ))
        })?;

    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Err(AppError::NotFound("Sports resource not found".to_string()));
    }

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let body = body.trim();
        let message = if body.is_empty() {
            format!("Sports API request failed with status {status}")
        } else {
            format!("Sports API request failed with status {status}: {body}")
        };
        return Err(AppError::BadGateway(message));
    }

    response.json::<T>().await.map_err(|error| {
        AppError::BadGateway(format!("Sports API returned an invalid payload: {error}"))
    })
}

fn sports_url(state: &AppState, path: &str) -> Result<Url, AppError> {
    let Some(base_url) = state.config.sports_api_base_url.as_ref() else {
        return Err(AppError::ServiceUnavailable(
            "Sports is not configured on this Euripus server".to_string(),
        ));
    };

    base_url.join(path).map_err(|error| {
        AppError::Internal(anyhow!(
            "failed to build Sports API URL for path {path}: {error}"
        ))
    })
}

impl From<UpstreamSportsEvent> for SportsEventResponse {
    fn from(value: UpstreamSportsEvent) -> Self {
        Self {
            id: value.id,
            sport: value.sport,
            competition: value.competition,
            title: value.title,
            start_time: value.start_time,
            end_time: value.end_time,
            status: value.status,
            venue: value.venue,
            round_label: value.round_label,
            participants: value.participants.map(Into::into),
            source: value.source,
            source_url: value.source_url,
            watch: value
                .watch
                .map(Into::into)
                .unwrap_or_else(|| SportsWatchResponse {
                    recommended_market: None,
                    recommended_provider: None,
                    availabilities: Vec::new(),
                }),
            search_metadata: value.search_metadata.map(Into::into).unwrap_or_else(|| {
                SportsSearchMetadataResponse {
                    queries: Vec::new(),
                    keywords: Vec::new(),
                }
            }),
        }
    }
}

impl From<UpstreamSportsParticipants> for SportsParticipantsResponse {
    fn from(value: UpstreamSportsParticipants) -> Self {
        Self {
            home: value.home,
            away: value.away,
        }
    }
}

impl From<UpstreamSportsWatch> for SportsWatchResponse {
    fn from(value: UpstreamSportsWatch) -> Self {
        Self {
            recommended_market: value.recommended_market,
            recommended_provider: value.recommended_provider,
            availabilities: value.availabilities.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<UpstreamSportsAvailability> for SportsAvailabilityResponse {
    fn from(value: UpstreamSportsAvailability) -> Self {
        Self {
            market: value.market,
            provider_family: value.provider_family,
            provider_label: value.provider_label,
            channel_name: value.channel_name,
            watch_type: value.watch_type,
            confidence: value.confidence,
            source: value.source,
            search_hints: value.search_hints,
        }
    }
}

impl From<UpstreamSportsSearchMetadata> for SportsSearchMetadataResponse {
    fn from(value: UpstreamSportsSearchMetadata) -> Self {
        Self {
            queries: value.queries,
            keywords: value.keywords,
        }
    }
}

impl From<UpstreamSportsProvider> for SportsProviderResponse {
    fn from(value: UpstreamSportsProvider) -> Self {
        Self {
            family: value.family,
            market: value.market,
            aliases: value.aliases,
        }
    }
}
