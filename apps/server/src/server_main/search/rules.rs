use super::*;

#[derive(Debug, Clone)]
pub(in crate::server_main) struct LoadedAdminPattern {
    pub(in crate::server_main) id: Uuid,
    pub(in crate::server_main) pattern: String,
    pub(in crate::server_main) normalized_pattern: String,
}

#[derive(Debug, Clone)]
pub(in crate::server_main) struct LoadedAdminPatternGroup {
    pub(in crate::server_main) id: Uuid,
    pub(in crate::server_main) kind: AdminSearchPatternKind,
    pub(in crate::server_main) value: String,
    pub(in crate::server_main) normalized_value: String,
    pub(in crate::server_main) match_target: AdminSearchMatchTarget,
    pub(in crate::server_main) match_mode: AdminSearchMatchMode,
    pub(in crate::server_main) priority: i32,
    pub(in crate::server_main) enabled: bool,
    pub(in crate::server_main) country_codes: Vec<String>,
    pub(in crate::server_main) patterns: Vec<LoadedAdminPattern>,
}

#[derive(Debug, Clone, Default)]
pub(in crate::server_main) struct EvaluatedAdminMetadata {
    pub(in crate::server_main) country_code: Option<String>,
    pub(in crate::server_main) provider_name: Option<String>,
    pub(in crate::server_main) is_ppv: bool,
    pub(in crate::server_main) is_vip: bool,
    pub(in crate::server_main) force_has_epg: bool,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::server_main) struct AdminSearchEvaluationInput<'a> {
    pub(in crate::server_main) channel_name: Option<&'a str>,
    pub(in crate::server_main) category_name: Option<&'a str>,
    pub(in crate::server_main) program_title: Option<&'a str>,
}

#[derive(Debug, FromRow)]
struct AdminPatternGroupRow {
    id: Uuid,
    kind: String,
    value: String,
    normalized_value: String,
    match_target: String,
    match_mode: String,
    priority: i32,
    enabled: bool,
}

#[derive(Debug, FromRow)]
struct AdminPatternRow {
    id: Uuid,
    group_id: Uuid,
    pattern: String,
    normalized_pattern: String,
}

#[derive(Debug, FromRow)]
struct AdminProviderCountryRow {
    group_id: Uuid,
    country_code: String,
}

pub(in crate::server_main) async fn load_pattern_groups(
    pool: &PgPool,
) -> Result<Vec<LoadedAdminPatternGroup>> {
    let groups = sqlx::query_as::<_, AdminPatternGroupRow>(
        r#"
        SELECT
          id,
          kind,
          value,
          normalized_value,
          match_target,
          match_mode,
          priority,
          enabled
        FROM admin_search_pattern_groups
        ORDER BY kind ASC, priority DESC, value ASC, created_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let patterns = sqlx::query_as::<_, AdminPatternRow>(
        r#"
        SELECT id, group_id, pattern, normalized_pattern
        FROM admin_search_patterns
        ORDER BY created_at ASC, pattern ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let provider_countries = sqlx::query_as::<_, AdminProviderCountryRow>(
        r#"
        SELECT group_id, country_code
        FROM admin_search_provider_countries
        ORDER BY created_at ASC, country_code ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut patterns_by_group = HashMap::<Uuid, Vec<LoadedAdminPattern>>::new();
    for pattern in patterns {
        patterns_by_group
            .entry(pattern.group_id)
            .or_default()
            .push(LoadedAdminPattern {
                id: pattern.id,
                pattern: pattern.pattern,
                normalized_pattern: pattern.normalized_pattern,
            });
    }

    let mut countries_by_group = HashMap::<Uuid, Vec<String>>::new();
    for country in provider_countries {
        countries_by_group
            .entry(country.group_id)
            .or_default()
            .push(country.country_code);
    }

    groups
        .into_iter()
        .map(|group| {
            Ok(LoadedAdminPatternGroup {
                id: group.id,
                kind: parse_pattern_kind(&group.kind)?,
                value: group.value,
                normalized_value: group.normalized_value,
                match_target: parse_match_target(&group.match_target)?,
                match_mode: parse_match_mode(&group.match_mode)?,
                priority: group.priority,
                enabled: group.enabled,
                country_codes: countries_by_group.remove(&group.id).unwrap_or_default(),
                patterns: patterns_by_group.remove(&group.id).unwrap_or_default(),
            })
        })
        .collect()
}

pub(in crate::server_main) async fn load_compiled_rules(
    pool: &PgPool,
) -> Result<Vec<LoadedAdminPatternGroup>> {
    load_pattern_groups(pool).await
}

pub(in crate::server_main) fn parse_patterns_text(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .filter_map(|item| {
            let dedupe_key = item.to_ascii_lowercase();
            seen.insert(dedupe_key).then(|| item.to_string())
        })
        .collect()
}

pub(in crate::server_main) fn parse_country_codes_text(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.to_ascii_lowercase())
        .filter_map(|item| seen.insert(item.clone()).then_some(item))
        .collect()
}

pub(in crate::server_main) fn normalize_rule_value(
    kind: AdminSearchPatternKind,
    value: &str,
) -> String {
    match kind {
        AdminSearchPatternKind::Country => value.trim().to_ascii_lowercase(),
        AdminSearchPatternKind::Provider => value.trim().to_ascii_lowercase(),
        AdminSearchPatternKind::Flag => value.trim().to_ascii_lowercase(),
    }
}

pub(in crate::server_main) fn normalize_rule_pattern(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(in crate::server_main) fn evaluate_patterns(
    groups: &[LoadedAdminPatternGroup],
    input: AdminSearchEvaluationInput<'_>,
) -> EvaluatedAdminMetadata {
    let mut best_country: Option<(i32, usize, String)> = None;
    let mut best_provider: Option<(i32, usize, String)> = None;
    let mut metadata = EvaluatedAdminMetadata::default();

    for group in groups.iter().filter(|group| group.enabled) {
        let matched_specificity = match_group(group, input);
        let Some(specificity) = matched_specificity else {
            continue;
        };

        match group.kind {
            AdminSearchPatternKind::Country => {
                if should_replace_best(best_country.as_ref(), group.priority, specificity) {
                    best_country =
                        Some((group.priority, specificity, group.normalized_value.clone()));
                }
            }
            AdminSearchPatternKind::Provider => {
                if should_replace_best(best_provider.as_ref(), group.priority, specificity) {
                    best_provider =
                        Some((group.priority, specificity, group.normalized_value.clone()));
                }
            }
            AdminSearchPatternKind::Flag => match group.normalized_value.as_str() {
                "ppv" => metadata.is_ppv = true,
                "vip" => metadata.is_vip = true,
                "force_epg" => metadata.force_has_epg = true,
                _ => {}
            },
        }
    }

    metadata.country_code = best_country.map(|(_, _, value)| value);
    metadata.provider_name = best_provider.map(|(_, _, value)| value);
    metadata
}

fn should_replace_best(
    current: Option<&(i32, usize, String)>,
    priority: i32,
    specificity: usize,
) -> bool {
    current
        .map(|(best_priority, best_specificity, _)| {
            priority > *best_priority
                || (priority == *best_priority && specificity > *best_specificity)
        })
        .unwrap_or(true)
}

fn match_group(
    group: &LoadedAdminPatternGroup,
    input: AdminSearchEvaluationInput<'_>,
) -> Option<usize> {
    let texts = texts_for_target(group.match_target, input);
    let mut best_specificity = None;

    for text in texts {
        let candidate = text.trim().to_ascii_lowercase();
        if candidate.is_empty() {
            continue;
        }

        for pattern in &group.patterns {
            if pattern.normalized_pattern.is_empty() {
                continue;
            }

            let matched = match group.match_mode {
                AdminSearchMatchMode::Prefix => candidate.starts_with(&pattern.normalized_pattern),
                AdminSearchMatchMode::Contains => candidate.contains(&pattern.normalized_pattern),
                AdminSearchMatchMode::Exact => candidate == pattern.normalized_pattern,
            };

            if matched {
                let specificity = pattern.normalized_pattern.len();
                best_specificity = Some(
                    best_specificity.map_or(specificity, |current: usize| current.max(specificity)),
                );
            }
        }
    }

    best_specificity
}

fn texts_for_target<'a>(
    target: AdminSearchMatchTarget,
    input: AdminSearchEvaluationInput<'a>,
) -> Vec<&'a str> {
    match target {
        AdminSearchMatchTarget::ChannelName => input.channel_name.into_iter().collect(),
        AdminSearchMatchTarget::CategoryName => input.category_name.into_iter().collect(),
        AdminSearchMatchTarget::ProgramTitle => input.program_title.into_iter().collect(),
        AdminSearchMatchTarget::ChannelOrCategory => input
            .channel_name
            .into_iter()
            .chain(input.category_name)
            .collect(),
        AdminSearchMatchTarget::AnyText => input
            .channel_name
            .into_iter()
            .chain(input.category_name)
            .chain(input.program_title)
            .collect(),
    }
}

fn parse_pattern_kind(value: &str) -> Result<AdminSearchPatternKind> {
    match value {
        "country" => Ok(AdminSearchPatternKind::Country),
        "provider" => Ok(AdminSearchPatternKind::Provider),
        "flag" => Ok(AdminSearchPatternKind::Flag),
        _ => Err(anyhow!("unsupported admin search pattern kind: {value}")),
    }
}

fn parse_match_target(value: &str) -> Result<AdminSearchMatchTarget> {
    match value {
        "channel_name" => Ok(AdminSearchMatchTarget::ChannelName),
        "category_name" => Ok(AdminSearchMatchTarget::CategoryName),
        "program_title" => Ok(AdminSearchMatchTarget::ProgramTitle),
        "channel_or_category" => Ok(AdminSearchMatchTarget::ChannelOrCategory),
        "any_text" => Ok(AdminSearchMatchTarget::AnyText),
        _ => Err(anyhow!("unsupported admin search match target: {value}")),
    }
}

fn parse_match_mode(value: &str) -> Result<AdminSearchMatchMode> {
    match value {
        "prefix" => Ok(AdminSearchMatchMode::Prefix),
        "contains" => Ok(AdminSearchMatchMode::Contains),
        "exact" => Ok(AdminSearchMatchMode::Exact),
        _ => Err(anyhow!("unsupported admin search match mode: {value}")),
    }
}

#[cfg(test)]
mod tests {
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
}
