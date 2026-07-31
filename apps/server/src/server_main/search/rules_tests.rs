use super::*;

fn sample_group(
    kind: AdminSearchPatternKind,
    value: &str,
    match_target: AdminSearchMatchTarget,
    match_mode: AdminSearchMatchMode,
    priority: i32,
    patterns: &[&str],
) -> LoadedAdminPatternGroup {
    LoadedAdminPatternGroup {
        id: Uuid::new_v4(),
        kind,
        value: value.to_string(),
        normalized_value: normalize_rule_value(kind, value),
        match_target,
        match_mode,
        priority,
        enabled: true,
        country_codes: Vec::new(),
        patterns: patterns
            .iter()
            .map(|pattern| LoadedAdminPattern {
                id: Uuid::new_v4(),
                pattern: (*pattern).to_string(),
                normalized_pattern: normalize_rule_pattern(pattern),
            })
            .collect(),
    }
}

#[test]
fn parse_patterns_text_splits_and_deduplicates() {
    let patterns = parse_patterns_text("SE:, SE|, se:");
    assert_eq!(patterns, vec!["SE:".to_string(), "SE|".to_string()]);
}

#[test]
fn parse_country_codes_text_normalizes_and_deduplicates() {
    let country_codes = parse_country_codes_text("SE, uk, se");
    assert_eq!(country_codes, vec!["se".to_string(), "uk".to_string()]);
}

#[test]
fn evaluate_patterns_supports_multiple_patterns_for_one_country() {
    let groups = vec![sample_group(
        AdminSearchPatternKind::Country,
        "SE",
        AdminSearchMatchTarget::ChannelOrCategory,
        AdminSearchMatchMode::Prefix,
        10,
        &["SE:", "SE|"],
    )];

    let first = evaluate_patterns(
        &groups,
        AdminSearchEvaluationInput {
            channel_name: Some("SE: TV4"),
            category_name: None,
            program_title: None,
        },
    );
    let second = evaluate_patterns(
        &groups,
        AdminSearchEvaluationInput {
            channel_name: None,
            category_name: Some("SE| Entertainment"),
            program_title: None,
        },
    );

    assert_eq!(first.country_code.as_deref(), Some("se"));
    assert_eq!(second.country_code.as_deref(), Some("se"));
}

#[test]
fn evaluate_patterns_sets_flag_rules_from_category_text() {
    let groups = vec![
        sample_group(
            AdminSearchPatternKind::Flag,
            "ppv",
            AdminSearchMatchTarget::ChannelOrCategory,
            AdminSearchMatchMode::Contains,
            5,
            &["PPV"],
        ),
        sample_group(
            AdminSearchPatternKind::Flag,
            "vip",
            AdminSearchMatchTarget::CategoryName,
            AdminSearchMatchMode::Contains,
            5,
            &["ⱽᴵᴾ"],
        ),
    ];

    let evaluated = evaluate_patterns(
        &groups,
        AdminSearchEvaluationInput {
            channel_name: Some("SE: VIAPLAY PPV 2"),
            category_name: Some("SE| PLAY+ ⱽᴵᴾ"),
            program_title: None,
        },
    );

    assert!(evaluated.is_ppv);
    assert!(evaluated.is_vip);
}

#[test]
fn evaluate_patterns_prefers_higher_priority_then_longer_match() {
    let groups = vec![
        sample_group(
            AdminSearchPatternKind::Provider,
            "play",
            AdminSearchMatchTarget::ChannelName,
            AdminSearchMatchMode::Contains,
            1,
            &["PLAY"],
        ),
        sample_group(
            AdminSearchPatternKind::Provider,
            "viaplay",
            AdminSearchMatchTarget::ChannelName,
            AdminSearchMatchMode::Contains,
            10,
            &["VIAPLAY"],
        ),
    ];

    let evaluated = evaluate_patterns(
        &groups,
        AdminSearchEvaluationInput {
            channel_name: Some("SE: VIAPLAY SPORT"),
            category_name: None,
            program_title: None,
        },
    );

    assert_eq!(evaluated.provider_name.as_deref(), Some("viaplay"));
}
