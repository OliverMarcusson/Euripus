use self::queries::{search_channels_postgres, search_programs_postgres};
use super::*;

pub(super) mod ai_ppv;
pub(super) mod indexing;
pub(super) mod lexicon;
pub(super) mod queries;
pub(super) mod rules;

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/search/filter-options", get(get_search_filter_options))
        .route("/search/channels", get(search_channels))
        .route("/search/programs", get(search_programs))
        .merge(ai_ppv::router())
}

async fn get_search_filter_options(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<SearchFilterOptionsResponse> {
    require_auth(&state, &headers).await?;
    let groups = rules::load_pattern_groups(&state.pool).await?;
    Ok(Json(build_search_filter_options(&groups)))
}

async fn search_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> ApiResult<ChannelSearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let quality_channels_only = query.quality_channels_only.unwrap_or(false);
    let (term, offset, limit, parsed) = parse_search_pagination(query)?;
    let visibility = load_channel_visibility_map(&state, auth.user_id, None).await?;
    let mut visible_channel_ids = visible_channel_ids_from_map(&visibility);
    if quality_channels_only {
        visible_channel_ids =
            super::guide::quality_channel_ids(&state.pool, auth.user_id, visible_channel_ids)
                .await?;
    }
    Ok(Json(
        search_channels_postgres(
            &state,
            &headers,
            auth.user_id,
            &term,
            offset,
            limit,
            &parsed,
            &visible_channel_ids,
        )
        .await?,
    ))
}

async fn search_programs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> ApiResult<ProgramSearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let quality_channels_only = query.quality_channels_only.unwrap_or(false);
    let (term, offset, limit, parsed) = parse_search_pagination(query)?;
    let visibility = load_channel_visibility_map(&state, auth.user_id, None).await?;
    let mut visible_channel_ids = visible_channel_ids_from_map(&visibility);
    if quality_channels_only {
        visible_channel_ids =
            super::guide::quality_channel_ids(&state.pool, auth.user_id, visible_channel_ids)
                .await?;
    }
    Ok(Json(
        search_programs_postgres(
            &state.pool,
            auth.user_id,
            &term,
            offset,
            limit,
            &parsed,
            &visible_channel_ids,
        )
        .await?,
    ))
}

fn parse_search_pagination(
    query: SearchQuery,
) -> Result<(String, i64, i64, lexicon::ParsedSearch), AppError> {
    let term = query.q.trim().to_string();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(SEARCH_DEFAULT_LIMIT);
    let parsed = lexicon::parse_search_query(&term);
    let has_structured_filters = !parsed.countries.is_empty()
        || !parsed.providers.is_empty()
        || parsed.ppv.is_some()
        || parsed.vip.is_some()
        || parsed.require_epg;

    if parsed.search.len() < 2 && !has_structured_filters {
        return Err(AppError::BadRequest(
            "Search query must be at least 2 characters".to_string(),
        ));
    }

    if offset < 0 {
        return Err(AppError::BadRequest(
            "Search offset must be zero or greater".to_string(),
        ));
    }

    if limit <= 0 {
        return Err(AppError::BadRequest(
            "Search limit must be greater than zero".to_string(),
        ));
    }

    Ok((term, offset, limit.min(SEARCH_MAX_LIMIT), parsed))
}

fn next_page_offset(offset: i64, limit: i64, total_count: i64) -> Option<i64> {
    let next_offset = offset + limit;
    (next_offset < total_count).then_some(next_offset)
}

fn build_search_filter_options(
    groups: &[rules::LoadedAdminPatternGroup],
) -> SearchFilterOptionsResponse {
    let countries = collect_filter_options(groups, AdminSearchPatternKind::Country);
    let providers = collect_provider_filter_options(groups, &countries);

    SearchFilterOptionsResponse {
        countries,
        providers,
    }
}

fn collect_filter_options(
    groups: &[rules::LoadedAdminPatternGroup],
    kind: AdminSearchPatternKind,
) -> Vec<String> {
    groups
        .iter()
        .filter(|group| group.enabled && group.kind == kind && !group.normalized_value.is_empty())
        .map(|group| group.normalized_value.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_provider_filter_options(
    groups: &[rules::LoadedAdminPatternGroup],
    countries: &[String],
) -> Vec<SearchFilterProviderOptionResponse> {
    let enabled_countries = countries.iter().cloned().collect::<HashSet<_>>();
    let mut providers_by_value =
        std::collections::BTreeMap::<String, std::collections::BTreeSet<String>>::new();

    for group in groups.iter().filter(|group| {
        group.enabled
            && group.kind == AdminSearchPatternKind::Provider
            && !group.normalized_value.is_empty()
    }) {
        let entry = providers_by_value
            .entry(group.normalized_value.clone())
            .or_default();
        for country_code in &group.country_codes {
            if enabled_countries.contains(country_code) {
                entry.insert(country_code.clone());
            }
        }
    }

    providers_by_value
        .into_iter()
        .map(
            |(value, country_codes)| SearchFilterProviderOptionResponse {
                value,
                country_codes: country_codes.into_iter().collect(),
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_group(
        kind: AdminSearchPatternKind,
        value: &str,
        enabled: bool,
        country_codes: &[&str],
    ) -> rules::LoadedAdminPatternGroup {
        rules::LoadedAdminPatternGroup {
            id: Uuid::new_v4(),
            kind,
            value: value.to_string(),
            normalized_value: value.to_ascii_lowercase(),
            match_target: AdminSearchMatchTarget::ChannelOrCategory,
            match_mode: AdminSearchMatchMode::Contains,
            priority: 0,
            enabled,
            country_codes: country_codes
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
            patterns: Vec::new(),
        }
    }

    #[test]
    fn build_search_filter_options_returns_enabled_country_and_provider_values() {
        let response = build_search_filter_options(&[
            sample_group(AdminSearchPatternKind::Provider, "viaplay", true, &["se"]),
            sample_group(AdminSearchPatternKind::Country, "se", true, &[]),
            sample_group(AdminSearchPatternKind::Provider, "viaplay", true, &["uk"]),
            sample_group(AdminSearchPatternKind::Country, "us", false, &[]),
            sample_group(AdminSearchPatternKind::Flag, "ppv", true, &[]),
            sample_group(
                AdminSearchPatternKind::Provider,
                "tv4play",
                true,
                &["se", "us"],
            ),
        ]);

        assert_eq!(response.countries, vec!["se".to_string()]);
        assert_eq!(
            response.providers,
            vec![
                SearchFilterProviderOptionResponse {
                    value: "tv4play".to_string(),
                    country_codes: vec!["se".to_string()],
                },
                SearchFilterProviderOptionResponse {
                    value: "viaplay".to_string(),
                    country_codes: vec!["se".to_string()],
                },
            ]
        );
    }
}
