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
