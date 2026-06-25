use super::*;

const AI_PPV_DEFAULT_LIMIT: usize = 12;
const AI_PPV_MAX_LIMIT: usize = 30;
const AI_PPV_CANDIDATE_LIMIT: i64 = 35;
const AI_PPV_MIN_CONFIDENCE: f64 = 0.18;
const OPENROUTER_CHAT_COMPLETIONS_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENROUTER_TIMEOUT_SECONDS: u64 = 25;
const OPENROUTER_MAX_TOKENS: u16 = 1600;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/search/ppv/ai", post(search_ppv_ai))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiPpvSearchRequest {
    query: String,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiPpvSearchResponse {
    query: String,
    backend: String,
    items: Vec<AiPpvSearchResult>,
    message: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct AiPpvSearchResult {
    channel: ChannelResponse,
    program: Option<ProgramResponse>,
    confidence: f64,
    reason: String,
    matched_terms: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
struct AiPpvCandidateRow {
    channel_id: Uuid,
    profile_id: Uuid,
    channel_name: String,
    logo_url: Option<String>,
    category_name: Option<String>,
    remote_stream_id: i32,
    epg_channel_id: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    stream_extension: Option<String>,
    is_favorite: bool,
    is_ppv: bool,
    is_ppv_favorite: bool,
    search_country_code: Option<String>,
    search_provider_name: Option<String>,
    program_id: Option<Uuid>,
    program_channel_id: Option<Uuid>,
    program_channel_name: Option<String>,
    program_title: Option<String>,
    program_description: Option<String>,
    program_start_at: Option<DateTime<Utc>>,
    program_end_at: Option<DateTime<Utc>>,
    program_can_catchup: Option<bool>,
    local_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiPpvPromptCandidate {
    id: String,
    channel_title: String,
    category: Option<String>,
    provider: Option<String>,
    country: Option<String>,
    is_ppv: bool,
    program_title: Option<String>,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct AiPpvCandidate {
    id: String,
    channel: ChannelResponse,
    program: Option<ProgramResponse>,
    prompt: AiPpvPromptCandidate,
    local_score: f64,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChatResponse {
    choices: Vec<OpenRouterChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
}

#[derive(Debug, Deserialize)]
struct OpenRouterMessage {
    content: String,
}

#[derive(Debug, Serialize)]
struct OpenRouterChatRequest {
    model: String,
    messages: Vec<OpenRouterChatMessage>,
    temperature: f32,
    max_tokens: u16,
    response_format: OpenRouterResponseFormat,
    plugins: Vec<OpenRouterPlugin>,
}

#[derive(Debug, Serialize)]
struct OpenRouterChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Serialize)]
struct OpenRouterResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<OpenRouterJsonSchema>,
}

#[derive(Debug, Serialize)]
struct OpenRouterJsonSchema {
    name: &'static str,
    strict: bool,
    schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct OpenRouterPlugin {
    id: &'static str,
}

#[derive(Debug, Deserialize)]
struct AiPpvModelResponse {
    matches: Vec<AiPpvModelMatch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiPpvModelMatch {
    id: String,
    confidence: f64,
    reason: String,
    #[serde(default)]
    matched_terms: Vec<String>,
}

async fn search_ppv_ai(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AiPpvSearchRequest>,
) -> ApiResult<AiPpvSearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let query = payload.query.trim().to_string();
    let limit = payload
        .limit
        .unwrap_or(AI_PPV_DEFAULT_LIMIT)
        .clamp(1, AI_PPV_MAX_LIMIT);

    if query.chars().count() < 3 {
        return Err(AppError::BadRequest(
            "AI PPV search query must be at least 3 characters".to_string(),
        ));
    }

    let visibility = load_channel_visibility_map(&state, auth.user_id, None).await?;
    let visible_channel_ids = visible_channel_ids_from_map(&visibility);
    let mut candidates = load_ai_ppv_candidates(&state, auth.user_id, &query, &visible_channel_ids)
        .await
        .map_err(AppError::from)?;
    rewrite_candidate_logo_urls(&state, &headers, auth.user_id, &mut candidates)?;

    if candidates.is_empty() {
        return Ok(Json(AiPpvSearchResponse {
            query,
            backend: "local_fallback".to_string(),
            items: Vec::new(),
            message: Some("No visible PPV candidates were found for this account.".to_string()),
        }));
    }

    match rerank_candidates_with_openrouter(&state, &query, &candidates, limit).await {
        Ok(items) if !items.is_empty() => Ok(Json(AiPpvSearchResponse {
            query,
            backend: "openrouter".to_string(),
            items,
            message: None,
        })),
        Ok(_) => Ok(Json(AiPpvSearchResponse {
            query: query.clone(),
            backend: "local_fallback".to_string(),
            items: fallback_ai_ppv_results(&query, &candidates, limit),
            message: Some(
                "AI did not return confident PPV matches; showing local matches.".to_string(),
            ),
        })),
        Err(error) => {
            warn!("OpenRouter PPV search failed, using local fallback: {error:?}");
            Ok(Json(AiPpvSearchResponse {
                query: query.clone(),
                backend: "local_fallback".to_string(),
                items: fallback_ai_ppv_results(&query, &candidates, limit),
                message: Some(
                    "AI PPV search is unavailable right now; showing local matches.".to_string(),
                ),
            }))
        }
    }
}

async fn load_ai_ppv_candidates(
    state: &AppState,
    user_id: Uuid,
    query: &str,
    visible_channel_ids: &[Uuid],
) -> Result<Vec<AiPpvCandidate>> {
    if visible_channel_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, AiPpvCandidateRow>(
        r#"
        SELECT
          c.id AS channel_id,
          c.profile_id,
          c.name AS channel_name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite,
          (c.search_is_ppv OR COALESCE(p.program_is_ppv, FALSE)) AS is_ppv,
          EXISTS(
            SELECT 1 FROM favorite_ppv_channels fpc
            WHERE fpc.user_id = c.user_id AND fpc.channel_id = c.id
          ) AS is_ppv_favorite,
          c.search_country_code,
          c.search_provider_name,
          p.id AS program_id,
          p.channel_id AS program_channel_id,
          p.channel_name AS program_channel_name,
          p.title AS program_title,
          p.description AS program_description,
          p.start_at AS program_start_at,
          p.end_at AS program_end_at,
          p.can_catchup AS program_can_catchup,
          GREATEST(
            similarity(lower(concat_ws(' ', c.name, cc.name, c.search_provider_name, c.search_country_code)), lower($2)),
            COALESCE(p.program_score, 0)
          )::DOUBLE PRECISION AS local_score
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        LEFT JOIN LATERAL (
          SELECT
            p.id,
            p.channel_id,
            p.channel_name,
            p.title,
            p.description,
            p.start_at,
            p.end_at,
            p.can_catchup,
            p.search_is_ppv AS program_is_ppv,
            similarity(lower(concat_ws(' ', p.title, p.channel_name, p.search_provider_name, p.search_country_code)), lower($2)) AS program_score
          FROM programs p
          WHERE p.user_id = c.user_id
            AND p.channel_id = c.id
            AND (c.search_is_ppv = TRUE OR p.search_is_ppv = TRUE)
            AND p.end_at > NOW() - ($4 * INTERVAL '1 hour')
            AND p.start_at < NOW() + ($5 * INTERVAL '1 day')
          ORDER BY
            similarity(lower(concat_ws(' ', p.title, p.channel_name, p.search_provider_name, p.search_country_code)), lower($2)) DESC,
            CASE
              WHEN p.start_at <= NOW() AND p.end_at >= NOW() THEN 0
              WHEN p.start_at > NOW() THEN 1
              ELSE 2
            END,
            p.start_at ASC,
            p.title ASC
          LIMIT 1
        ) p ON TRUE
        WHERE c.user_id = $1
          AND c.id = ANY($3)
          AND (
            c.search_is_ppv = TRUE
            OR EXISTS(
              SELECT 1
              FROM programs ppv_program
              WHERE ppv_program.user_id = c.user_id
                AND ppv_program.channel_id = c.id
                AND ppv_program.search_is_ppv = TRUE
                AND ppv_program.end_at > NOW() - ($4 * INTERVAL '1 hour')
                AND ppv_program.start_at < NOW() + ($5 * INTERVAL '1 day')
            )
          )
        ORDER BY
          local_score DESC,
          CASE
            WHEN p.start_at <= NOW() AND p.end_at >= NOW() THEN 0
            WHEN p.start_at > NOW() THEN 1
            ELSE 2
          END,
          c.name ASC
        LIMIT $6
        "#,
    )
    .bind(user_id)
    .bind(query)
    .bind(visible_channel_ids)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .bind(AI_PPV_CANDIDATE_LIMIT)
    .fetch_all(&state.pool)
    .await?;

    Ok(rows.into_iter().map(candidate_from_row).collect())
}

fn candidate_from_row(row: AiPpvCandidateRow) -> AiPpvCandidate {
    let program = match (row.program_id, row.program_start_at, row.program_end_at) {
        (Some(id), Some(start_at), Some(end_at)) => Some(ProgramResponse {
            id,
            channel_id: row.program_channel_id,
            channel_name: row.program_channel_name.clone(),
            title: row.program_title.clone().unwrap_or_default(),
            description: row.program_description,
            start_at,
            end_at,
            can_catchup: row.program_can_catchup.unwrap_or(false),
        }),
        _ => None,
    };
    let channel = ChannelResponse {
        id: row.channel_id,
        profile_id: row.profile_id,
        name: row.channel_name.clone(),
        logo_url: row.logo_url,
        category_name: row.category_name.clone(),
        remote_stream_id: row.remote_stream_id,
        epg_channel_id: row.epg_channel_id,
        has_epg: program.is_some(),
        has_catchup: row.has_catchup,
        archive_duration_hours: row.archive_duration_hours,
        stream_extension: row.stream_extension,
        is_favorite: row.is_favorite,
        is_ppv: row.is_ppv,
        is_ppv_favorite: row.is_ppv_favorite,
    };
    let id = row.channel_id.to_string();
    let prompt = AiPpvPromptCandidate {
        id: id.clone(),
        channel_title: row.channel_name,
        category: row.category_name,
        provider: row.search_provider_name,
        country: row.search_country_code,
        is_ppv: row.is_ppv,
        program_title: row.program_title,
        starts_at: row.program_start_at,
        ends_at: row.program_end_at,
    };

    AiPpvCandidate {
        id,
        channel,
        program,
        prompt,
        local_score: row.local_score.unwrap_or(0.0),
    }
}

fn rewrite_candidate_logo_urls(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    candidates: &mut [AiPpvCandidate],
) -> Result<(), AppError> {
    let request_base_url = request_base_url(&state.config, headers)?;
    for candidate in candidates {
        candidate.channel.logo_url = rewrite_channel_logo_url(
            state,
            &request_base_url,
            user_id,
            candidate.channel.profile_id,
            candidate.channel.logo_url.take(),
        )?;
    }
    Ok(())
}

async fn rerank_candidates_with_openrouter(
    state: &AppState,
    query: &str,
    candidates: &[AiPpvCandidate],
    limit: usize,
) -> Result<Vec<AiPpvSearchResult>, AppError> {
    let Some(api_key) = state.config.openrouter_api_key.as_deref() else {
        return Err(AppError::ServiceUnavailable(
            "OpenRouter is not configured".to_string(),
        ));
    };

    let prompt_candidates = candidates
        .iter()
        .map(|candidate| candidate.prompt.clone())
        .collect::<Vec<_>>();
    let user_prompt = build_ai_ppv_prompt(query, &prompt_candidates, limit)?;
    let request = OpenRouterChatRequest {
        model: state.config.openrouter_model.clone(),
        messages: vec![
            OpenRouterChatMessage {
                role: "system",
                content: "You match user-described PPV sports/events to provider channel candidates. Return only one complete strict JSON object. Do not use markdown or prose.".to_string(),
            },
            OpenRouterChatMessage {
                role: "user",
                content: user_prompt,
            },
        ],
        temperature: 0.0,
        max_tokens: OPENROUTER_MAX_TOKENS,
        response_format: OpenRouterResponseFormat {
            kind: "json_schema",
            json_schema: Some(ai_ppv_response_json_schema()),
        },
        plugins: vec![OpenRouterPlugin {
            id: "response-healing",
        }],
    };

    let response = state
        .provider_http_client
        .post(OPENROUTER_CHAT_COMPLETIONS_URL)
        .bearer_auth(api_key)
        .header("HTTP-Referer", "https://euripus.local")
        .header("X-Title", "Euripus AI PPV Search")
        .timeout(Duration::from_secs(OPENROUTER_TIMEOUT_SECONDS))
        .json(&request)
        .send()
        .await
        .map_err(|error| AppError::BadGateway(format!("OpenRouter request failed: {error}")))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::BadGateway(format!(
            "OpenRouter request failed with status {status}: {}",
            body.trim()
        )));
    }

    let payload = response
        .json::<OpenRouterChatResponse>()
        .await
        .map_err(|error| {
            AppError::BadGateway(format!("OpenRouter returned invalid JSON: {error}"))
        })?;
    let content = payload
        .choices
        .first()
        .map(|choice| choice.message.content.as_str())
        .ok_or_else(|| AppError::BadGateway("OpenRouter returned no choices".to_string()))?;

    parse_ai_ppv_model_response(content, candidates, limit)
}

fn ai_ppv_response_json_schema() -> OpenRouterJsonSchema {
    OpenRouterJsonSchema {
        name: "ai_ppv_matches",
        strict: true,
        schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "matches": {
                    "type": "array",
                    "description": "Likely matching PPV candidate IDs ordered from best to weakest.",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Candidate ID copied exactly from the provided candidates."
                            },
                            "confidence": {
                                "type": "number",
                                "minimum": 0,
                                "maximum": 1,
                                "description": "Confidence that this candidate matches the user's event description."
                            },
                            "reason": {
                                "type": "string",
                                "description": "Short user-facing reason for the match."
                            },
                            "matchedTerms": {
                                "type": "array",
                                "description": "Short terms from the query or candidate that explain the match.",
                                "items": { "type": "string" }
                            }
                        },
                        "required": ["id", "confidence", "reason", "matchedTerms"]
                    }
                }
            },
            "required": ["matches"]
        }),
    }
}

fn build_ai_ppv_prompt(
    query: &str,
    candidates: &[AiPpvPromptCandidate],
    limit: usize,
) -> Result<String, AppError> {
    let candidates_json =
        serde_json::to_string(candidates).map_err(|error| AppError::Internal(anyhow!(error)))?;
    Ok(format!(
        "User event description: {query}\n\
         Return up to {limit} likely matches from the candidate list.\n\
         Use semantic event matching, team/country aliases, competition names, and provider naming variants.\n\
         Prefer live or soon-upcoming PPV events when ambiguous.\n\
         Return JSON exactly like {{\"matches\":[{{\"id\":\"candidate-id\",\"confidence\":0.82,\"reason\":\"short reason\",\"matchedTerms\":[\"term\"]}}]}}.\n\
         Only use IDs from candidates. Do not invent channels.\n\
         Candidates: {candidates_json}"
    ))
}

fn parse_ai_ppv_model_response(
    content: &str,
    candidates: &[AiPpvCandidate],
    limit: usize,
) -> Result<Vec<AiPpvSearchResult>, AppError> {
    let json = extract_json_object(content).ok_or_else(|| {
        AppError::BadGateway("OpenRouter response did not contain a JSON object".to_string())
    })?;
    let payload = serde_json::from_str::<AiPpvModelResponse>(json).map_err(|error| {
        AppError::BadGateway(format!("OpenRouter match JSON was invalid: {error}"))
    })?;
    let candidates_by_id = candidates
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut items = Vec::new();

    for matched in payload.matches {
        if !matched.confidence.is_finite()
            || !(AI_PPV_MIN_CONFIDENCE..=1.0).contains(&matched.confidence)
            || !seen.insert(matched.id.clone())
        {
            continue;
        }
        let Some(candidate) = candidates_by_id.get(matched.id.as_str()) else {
            continue;
        };
        items.push(AiPpvSearchResult {
            channel: candidate.channel.clone(),
            program: candidate.program.clone(),
            confidence: matched.confidence,
            reason: clamp_text(&matched.reason, 180),
            matched_terms: matched
                .matched_terms
                .into_iter()
                .map(|term| clamp_text(&term, 40))
                .filter(|term| !term.trim().is_empty())
                .take(8)
                .collect(),
        });
    }

    items.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items.truncate(limit);
    Ok(items)
}

fn fallback_ai_ppv_results(
    query: &str,
    candidates: &[AiPpvCandidate],
    limit: usize,
) -> Vec<AiPpvSearchResult> {
    let query_terms = normalize_ai_query_terms(query);
    candidates
        .iter()
        .take(limit)
        .map(|candidate| AiPpvSearchResult {
            channel: candidate.channel.clone(),
            program: candidate.program.clone(),
            confidence: fallback_confidence(candidate.local_score),
            reason: "Local PPV metadata match".to_string(),
            matched_terms: matched_local_terms(candidate, &query_terms),
        })
        .collect()
}

fn fallback_confidence(local_score: f64) -> f64 {
    (0.35 + local_score.clamp(0.0, 1.0) * 0.45).min(0.8)
}

fn normalize_ai_query_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| term.len() >= 3)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn matched_local_terms(candidate: &AiPpvCandidate, query_terms: &[String]) -> Vec<String> {
    let haystack = format!(
        "{} {} {} {}",
        candidate.prompt.channel_title,
        candidate.prompt.category.as_deref().unwrap_or_default(),
        candidate.prompt.provider.as_deref().unwrap_or_default(),
        candidate
            .prompt
            .program_title
            .as_deref()
            .unwrap_or_default()
    )
    .to_ascii_lowercase();

    query_terms
        .iter()
        .filter(|term| haystack.contains(term.as_str()))
        .take(8)
        .cloned()
        .collect()
}

fn extract_json_object(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    (start <= end).then_some(&content[start..=end])
}

fn clamp_text(value: &str, max_chars: usize) -> String {
    value.trim().chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_candidate(id: Uuid, channel_name: &str) -> AiPpvCandidate {
        let id_string = id.to_string();
        let channel = ChannelResponse {
            id,
            profile_id: Uuid::from_u128(2),
            name: channel_name.to_string(),
            logo_url: Some("https://example.com/logo.png".to_string()),
            category_name: Some("SE| VIAPLAY PPV".to_string()),
            remote_stream_id: 5,
            epg_channel_id: None,
            has_epg: false,
            has_catchup: false,
            archive_duration_hours: None,
            stream_extension: Some("m3u8".to_string()),
            is_favorite: false,
            is_ppv: true,
            is_ppv_favorite: false,
        };

        AiPpvCandidate {
            id: id_string.clone(),
            channel,
            program: None,
            prompt: AiPpvPromptCandidate {
                id: id_string,
                channel_title: channel_name.to_string(),
                category: Some("SE| VIAPLAY PPV".to_string()),
                provider: Some("viaplay".to_string()),
                country: Some("se".to_string()),
                is_ppv: true,
                program_title: None,
                starts_at: None,
                ends_at: None,
            },
            local_score: 0.42,
        }
    }

    #[test]
    fn prompt_payload_omits_stream_urls_and_remote_ids() {
        let candidate = sample_candidate(Uuid::from_u128(10), "Sweden vs Japan");
        let prompt = build_ai_ppv_prompt("sweden japan", &[candidate.prompt], 5)
            .expect("prompt should build");

        assert!(prompt.contains("Sweden vs Japan"));
        assert!(!prompt.contains("remoteStreamId"));
        assert!(!prompt.contains("logoUrl"));
        assert!(!prompt.contains("https://example.com"));
    }

    #[test]
    fn parser_rejects_unknown_ids_and_invalid_confidence_values() {
        let known_id = Uuid::from_u128(11);
        let candidate = sample_candidate(known_id, "Sweden vs Japan");
        let content = format!(
            r#"{{
              "matches": [
                {{"id":"{}","confidence":0.91,"reason":"teams match","matchedTerms":["sweden","japan"]}},
                {{"id":"{}","confidence":1.8,"reason":"bad confidence","matchedTerms":[]}},
                {{"id":"missing","confidence":0.93,"reason":"unknown","matchedTerms":[]}}
              ]
            }}"#,
            known_id, known_id
        );

        let items =
            parse_ai_ppv_model_response(&content, &[candidate], 10).expect("valid model response");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].channel.id, known_id);
        assert_eq!(items[0].matched_terms, vec!["sweden", "japan"]);
    }

    #[test]
    fn parser_accepts_json_wrapped_in_text() {
        let known_id = Uuid::from_u128(12);
        let candidate = sample_candidate(known_id, "Cup Final");
        let content = format!(
            "Here is JSON: {{\"matches\":[{{\"id\":\"{known_id}\",\"confidence\":0.5,\"reason\":\"title\",\"matchedTerms\":[]}}]}}"
        );

        let items = parse_ai_ppv_model_response(&content, &[candidate], 10)
            .expect("wrapped json should parse");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].reason, "title");
    }
}
