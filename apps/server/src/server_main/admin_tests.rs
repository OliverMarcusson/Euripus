use super::*;

#[test]
fn validates_matching_pi_regex_proposals() {
    validate_regex_proposal(
        ":Viaplay SE 17",
        &PiRegexProposal {
            regex: r"(?i)^:viaplay se \d+$".to_string(),
            explanation: "Matches numbered Viaplay SE placeholders.".to_string(),
        },
    )
    .expect("valid proposal");
}

#[test]
fn rejects_pi_regex_proposals_that_do_not_match_the_sample() {
    let error = validate_regex_proposal(
        ":Viaplay SE 17",
        &PiRegexProposal {
            regex: r"^other$".to_string(),
            explanation: "Wrong family.".to_string(),
        },
    )
    .expect_err("proposal should be rejected");
    assert!(matches!(error, AppError::BadRequest(_)));
}

#[test]
fn validate_import_pattern_groups_accepts_valid_batches_with_defaults() {
    let known_country_codes = HashSet::from(["se".to_string()]);
    let groups = validate_import_pattern_groups(
        vec![AdminPatternGroupImportItem {
            kind: "country".to_string(),
            value: "se".to_string(),
            match_target: "channel_or_category".to_string(),
            match_mode: "prefix".to_string(),
            priority: None,
            enabled: None,
            patterns: Some(vec!["SE:".to_string(), "SE|".to_string()]),
            patterns_text: None,
            country_codes: None,
        }],
        &known_country_codes,
    )
    .expect("expected valid import groups");

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].priority, 0);
    assert!(groups[0].enabled);
    assert_eq!(
        groups[0].patterns,
        vec!["SE:".to_string(), "SE|".to_string()]
    );
}

#[test]
fn validate_import_pattern_groups_rejects_items_without_patterns() {
    let known_country_codes = HashSet::new();
    let error = validate_import_pattern_groups(
        vec![AdminPatternGroupImportItem {
            kind: "flag".to_string(),
            value: "ppv".to_string(),
            match_target: "channel_or_category".to_string(),
            match_mode: "contains".to_string(),
            priority: Some(10),
            enabled: Some(true),
            patterns: Some(vec![]),
            patterns_text: None,
            country_codes: None,
        }],
        &known_country_codes,
    )
    .expect_err("expected invalid import to fail");

    match error {
        AppError::BadRequestDetailed { details, .. } => {
            let details = details
                .as_array()
                .expect("expected array of validation errors");
            assert_eq!(details.len(), 1);
            assert_eq!(details[0]["index"], 0);
            assert_eq!(details[0]["field"], "patterns");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn validate_import_pattern_groups_reports_invalid_enum_values() {
    let known_country_codes = HashSet::new();
    let error = validate_import_pattern_groups(
        vec![AdminPatternGroupImportItem {
            kind: "region".to_string(),
            value: "se".to_string(),
            match_target: "channel".to_string(),
            match_mode: "wildcard".to_string(),
            priority: Some(1),
            enabled: Some(true),
            patterns: Some(vec!["SE:".to_string()]),
            patterns_text: None,
            country_codes: None,
        }],
        &known_country_codes,
    )
    .expect_err("expected invalid enum values to fail");

    match error {
        AppError::BadRequestDetailed { details, .. } => {
            let details = details
                .as_array()
                .expect("expected array of validation errors");
            assert_eq!(details.len(), 3);
            assert!(details.iter().any(|detail| detail["field"] == "kind"));
            assert!(
                details
                    .iter()
                    .any(|detail| detail["field"] == "matchTarget")
            );
            assert!(details.iter().any(|detail| detail["field"] == "matchMode"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn normalize_import_patterns_supports_patterns_text_fallback() {
    let patterns = normalize_import_patterns(None, Some("SE:, SE|, se:".to_string()));
    assert_eq!(patterns, vec!["SE:".to_string(), "SE|".to_string()]);
}

#[test]
fn validate_import_pattern_groups_accepts_provider_country_codes() {
    let known_country_codes = HashSet::from(["se".to_string(), "uk".to_string()]);
    let groups = validate_import_pattern_groups(
        vec![AdminPatternGroupImportItem {
            kind: "provider".to_string(),
            value: "viaplay".to_string(),
            match_target: "channel_or_category".to_string(),
            match_mode: "contains".to_string(),
            priority: Some(10),
            enabled: Some(true),
            patterns: Some(vec!["VIAPLAY".to_string()]),
            patterns_text: None,
            country_codes: Some(vec!["SE".to_string(), "uk".to_string()]),
        }],
        &known_country_codes,
    )
    .expect("expected provider import groups");

    assert_eq!(
        groups[0].country_codes,
        vec!["se".to_string(), "uk".to_string()]
    );
}

#[test]
fn validate_import_pattern_groups_rejects_unknown_provider_country_codes() {
    let known_country_codes = HashSet::from(["se".to_string()]);
    let error = validate_import_pattern_groups(
        vec![AdminPatternGroupImportItem {
            kind: "provider".to_string(),
            value: "viaplay".to_string(),
            match_target: "channel_or_category".to_string(),
            match_mode: "contains".to_string(),
            priority: Some(10),
            enabled: Some(true),
            patterns: Some(vec!["VIAPLAY".to_string()]),
            patterns_text: None,
            country_codes: Some(vec!["uk".to_string()]),
        }],
        &known_country_codes,
    )
    .expect_err("expected invalid provider country code");

    match error {
        AppError::BadRequestDetailed { details, .. } => {
            let details = details
                .as_array()
                .expect("expected array of validation errors");
            assert!(
                details
                    .iter()
                    .any(|detail| detail["field"] == "countryCodes")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn validate_import_pattern_groups_accepts_provider_country_codes_defined_in_same_batch() {
    let known_country_codes = HashSet::new();
    let groups = validate_import_pattern_groups(
        vec![
            AdminPatternGroupImportItem {
                kind: "provider".to_string(),
                value: "viaplay".to_string(),
                match_target: "channel_or_category".to_string(),
                match_mode: "contains".to_string(),
                priority: Some(10),
                enabled: Some(true),
                patterns: Some(vec!["VIAPLAY".to_string()]),
                patterns_text: None,
                country_codes: Some(vec!["se".to_string(), "uk".to_string()]),
            },
            AdminPatternGroupImportItem {
                kind: "country".to_string(),
                value: "se".to_string(),
                match_target: "channel_or_category".to_string(),
                match_mode: "prefix".to_string(),
                priority: Some(10),
                enabled: Some(true),
                patterns: Some(vec!["SE:".to_string()]),
                patterns_text: None,
                country_codes: None,
            },
            AdminPatternGroupImportItem {
                kind: "country".to_string(),
                value: "uk".to_string(),
                match_target: "channel_or_category".to_string(),
                match_mode: "prefix".to_string(),
                priority: Some(10),
                enabled: Some(true),
                patterns: Some(vec!["UK:".to_string()]),
                patterns_text: None,
                country_codes: None,
            },
        ],
        &known_country_codes,
    )
    .expect("expected same-batch countries to satisfy provider country validation");

    assert_eq!(
        groups[0].country_codes,
        vec!["se".to_string(), "uk".to_string()]
    );
}
