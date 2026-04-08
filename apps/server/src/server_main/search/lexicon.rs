use super::*;

#[derive(Debug, Clone, Default)]
pub(in crate::server_main) struct SearchLexicon {
    pub(in crate::server_main) known_prefixes: HashSet<String>,
    pub(in crate::server_main) country_prefixes: HashSet<String>,
    pub(in crate::server_main) region_prefixes: HashSet<String>,
    pub(in crate::server_main) provider_aliases: Vec<ProviderAlias>,
    pub(in crate::server_main) provider_labels: HashMap<String, Vec<String>>,
    pub(in crate::server_main) typo_disabled_words: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::server_main) struct ProviderAlias {
    pub(in crate::server_main) alias: String,
    pub(in crate::server_main) normalized_alias: String,
    pub(in crate::server_main) alias_tokens: Vec<String>,
    pub(in crate::server_main) key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::server_main) struct ParsedSearch {
    pub(in crate::server_main) search: String,
    pub(in crate::server_main) filter: Option<String>,
    pub(in crate::server_main) countries: Vec<String>,
    pub(in crate::server_main) regions: Vec<String>,
    pub(in crate::server_main) providers: Vec<String>,
}

#[derive(Debug, FromRow)]
pub(in crate::server_main) struct SearchLexiconRow {
    pub(in crate::server_main) category_name: Option<String>,
    pub(in crate::server_main) channel_name: String,
}

pub(in crate::server_main) async fn get_search_lexicon(
    state: &AppState,
    user_id: Uuid,
) -> Result<Arc<SearchLexicon>> {
    if let Some(existing) = state.search_lexicons.get(&user_id) {
        return Ok(existing.clone());
    }

    let lexicon = Arc::new(load_search_lexicon(&state.pool, Some(user_id)).await?);
    state.search_lexicons.insert(user_id, lexicon.clone());
    Ok(lexicon)
}

pub(in crate::server_main) async fn refresh_search_lexicon(
    state: &AppState,
    user_id: Uuid,
) -> Result<Arc<SearchLexicon>> {
    let lexicon = Arc::new(load_search_lexicon(&state.pool, Some(user_id)).await?);
    state.search_lexicons.insert(user_id, lexicon.clone());
    Ok(lexicon)
}

pub(in crate::server_main) async fn load_search_lexicon(
    pool: &PgPool,
    user_id: Option<Uuid>,
) -> Result<SearchLexicon> {
    let rows = match user_id {
        Some(user_id) => {
            sqlx::query_as::<_, SearchLexiconRow>(
                r#"
                SELECT cc.name AS category_name, c.name AS channel_name
                FROM channels c
                LEFT JOIN channel_categories cc ON cc.id = c.category_id
                WHERE c.user_id = $1
                "#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, SearchLexiconRow>(
                r#"
                SELECT cc.name AS category_name, c.name AS channel_name
                FROM channels c
                LEFT JOIN channel_categories cc ON cc.id = c.category_id
                "#,
            )
            .fetch_all(pool)
            .await?
        }
    };

    let mut known_prefixes = HashSet::new();
    let mut country_prefixes = HashSet::new();
    let mut region_prefixes = HashSet::new();
    let mut label_candidates: HashMap<String, HashSet<String>> = HashMap::new();

    for row in &rows {
        if let Some(category_name) = row.category_name.as_deref() {
            if let Some(prefix) = extract_catalog_prefix(category_name) {
                known_prefixes.insert(prefix.clone());
                if is_country_prefix(&prefix) {
                    country_prefixes.insert(prefix);
                } else {
                    region_prefixes.insert(prefix);
                }
            }
        }

        for candidate in collect_provider_candidates(&row.channel_name, &known_prefixes) {
            let tokens = tokenize_normalized(&candidate);
            if tokens.is_empty() {
                continue;
            }
            let key = provider_key_from_tokens(&tokens);
            label_candidates
                .entry(key)
                .or_default()
                .insert(candidate.clone());
        }
        if let Some(category_name) = row.category_name.as_deref() {
            for candidate in collect_provider_candidates(category_name, &known_prefixes) {
                let tokens = tokenize_normalized(&candidate);
                if tokens.is_empty() {
                    continue;
                }
                let key = provider_key_from_tokens(&tokens);
                label_candidates
                    .entry(key)
                    .or_default()
                    .insert(candidate.clone());
            }
        }
    }

    let mut aliases = Vec::new();
    let mut finalized_labels = HashMap::new();
    for (key, labels) in label_candidates {
        let mut normalized_labels = labels
            .into_iter()
            .filter_map(|label| {
                let normalized_alias = normalize_search_text(&label);
                let alias_tokens = tokenize_normalized(&label);
                if alias_tokens.is_empty() || normalized_alias.len() < 3 {
                    return None;
                }
                Some((label, normalized_alias, alias_tokens))
            })
            .collect::<Vec<_>>();

        normalized_labels.sort_by(|left, right| {
            left.2
                .len()
                .cmp(&right.2.len())
                .then(left.0.len().cmp(&right.0.len()))
                .then(left.0.cmp(&right.0))
        });
        normalized_labels.dedup_by(|left, right| left.1 == right.1);
        if normalized_labels.is_empty() {
            continue;
        }

        let labels = normalized_labels
            .iter()
            .map(|(label, _, _)| label.clone())
            .collect::<Vec<_>>();
        finalized_labels.insert(key.clone(), labels);

        for (alias, normalized_alias, alias_tokens) in normalized_labels {
            aliases.push(ProviderAlias {
                alias,
                normalized_alias,
                alias_tokens,
                key: key.clone(),
            });
        }
    }

    aliases.sort_by(|left, right| {
        right
            .alias_tokens
            .len()
            .cmp(&left.alias_tokens.len())
            .then(right.alias.len().cmp(&left.alias.len()))
            .then(left.alias.cmp(&right.alias))
    });

    let mut typo_disabled_words = known_prefixes
        .iter()
        .map(|prefix| prefix.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    for word in short_search_abbreviations() {
        typo_disabled_words.insert(word.to_string());
    }

    Ok(SearchLexicon {
        known_prefixes,
        country_prefixes,
        region_prefixes,
        provider_aliases: aliases,
        provider_labels: finalized_labels,
        typo_disabled_words,
    })
}

pub(in crate::server_main) fn build_meili_synonyms(
    _lexicon: &SearchLexicon,
) -> HashMap<String, Vec<String>> {
    let mut synonyms = HashMap::new();

    for group in [
        &["se", "swe", "sweden"][..],
        &["uk", "gb", "britain", "great britain", "united kingdom"][..],
        &["ucl", "champions league"][..],
        &["epl", "premier league"][..],
        &["f1", "formula 1"][..],
        &["nba", "national basketball association"][..],
        &["nfl", "national football league"][..],
        &["nhl", "national hockey league"][..],
        &["mlb", "major league baseball"][..],
        &["pga", "pga tour"][..],
        &["atp", "atp tour"][..],
        &["wta", "wta tour"][..],
        &["ufc", "ultimate fighting championship"][..],
        &["paramount plus", "paramount+"][..],
        &["disney plus", "disney+"][..],
        &["tsn plus", "tsn+"][..],
        &["play plus", "play+"][..],
        &["via play", "viaplay"][..],
    ] {
        add_synonym_group(&mut synonyms, group);
    }

    synonyms
}

fn add_synonym_group(synonyms: &mut HashMap<String, Vec<String>>, group: &[&str]) {
    let normalized = group
        .iter()
        .map(|value| normalize_search_text(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    for term in &normalized {
        let mut others = normalized
            .iter()
            .filter(|candidate| *candidate != term)
            .cloned()
            .collect::<Vec<_>>();
        others.sort();
        others.dedup();
        if !others.is_empty() {
            synonyms.insert(term.clone(), others);
        }
    }
}

pub(in crate::server_main) fn parse_search_query(
    query: &str,
    lexicon: &SearchLexicon,
) -> ParsedSearch {
    let trimmed = query.trim();
    let mut remaining = trimmed.to_string();
    let mut countries = Vec::new();
    let mut regions = Vec::new();
    let mut providers = Vec::new();

    if let Some(colon_index) = remaining.find(':') {
        let prefix_candidate = normalize_prefix(&remaining[..colon_index]);
        if !prefix_candidate.is_empty()
            && lexicon.known_prefixes.contains(&prefix_candidate)
            && !remaining[..colon_index].contains(' ')
        {
            if lexicon.country_prefixes.contains(&prefix_candidate) {
                countries.push(prefix_candidate.clone());
            } else {
                regions.push(prefix_candidate.clone());
            }
            remaining = remaining[colon_index + 1..].trim().to_string();
        }
    }

    let original_tokens = remaining
        .split_whitespace()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let normalized_tokens = original_tokens
        .iter()
        .map(|token| normalize_search_text(token))
        .collect::<Vec<_>>();
    let mut consumed = vec![false; original_tokens.len()];

    for alias in lexicon
        .provider_aliases
        .iter()
        .filter(|alias| is_high_confidence_provider_alias(alias))
    {
        if alias.alias_tokens.is_empty() || original_tokens.is_empty() {
            continue;
        }

        let mut matched = false;
        'search: for start in 0..normalized_tokens.len() {
            for end in start + 1..=normalized_tokens.len().min(start + 3) {
                if consumed[start..end].iter().any(|used| *used) {
                    continue;
                }
                let candidate = normalized_tokens[start..end]
                    .iter()
                    .filter(|token| !token.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if candidate == alias.normalized_alias {
                    if !providers.iter().any(|provider| provider == &alias.key) {
                        providers.push(alias.key.clone());
                    }
                    consumed[start..end].fill(true);
                    matched = true;
                    break 'search;
                }
            }
        }

        if matched && consumed.iter().all(|token| *token) {
            break;
        }
    }

    let search = original_tokens
        .into_iter()
        .zip(consumed)
        .filter_map(|(token, used)| (!used).then_some(token))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    let mut clauses = Vec::new();
    if !countries.is_empty() {
        clauses.push(build_meili_filter_clause("country_code", &countries));
    }
    if !regions.is_empty() {
        clauses.push(build_meili_filter_clause("region_code", &regions));
    }
    if !providers.is_empty() {
        clauses.push(build_meili_filter_clause("provider_key", &providers));
    }

    ParsedSearch {
        search,
        filter: (!clauses.is_empty()).then(|| clauses.join(" AND ")),
        countries,
        regions,
        providers,
    }
}

fn is_high_confidence_provider_alias(alias: &ProviderAlias) -> bool {
    if alias.alias_tokens.is_empty() {
        return false;
    }

    if alias.alias_tokens.len() >= 2 {
        return true;
    }

    let token = alias.alias_tokens[0].as_str();
    token.len() >= 5 && token.chars().all(|ch| ch.is_ascii_alphabetic())
}

pub(in crate::server_main) fn build_meili_filter_clause(
    attribute: &str,
    values: &[String],
) -> String {
    if values.len() == 1 {
        format!(r#"{attribute} = "{}""#, values[0])
    } else {
        let joined = values
            .iter()
            .map(|value| format!(r#"{attribute} = "{value}""#))
            .collect::<Vec<_>>()
            .join(" OR ");
        format!("({joined})")
    }
}

pub(in crate::server_main) fn build_meili_search_filter(
    user_id: Uuid,
    parsed_filter: Option<&str>,
) -> String {
    match parsed_filter {
        Some(filter) if !filter.is_empty() => format!(r#"user_id = "{user_id}" AND {filter}"#),
        _ => format!(r#"user_id = "{user_id}""#),
    }
}

pub(in crate::server_main) fn meili_channel_primary_limit(offset: i64, limit: i64) -> usize {
    ((offset + limit).max(limit))
        .min(MEILI_MAX_TOTAL_HITS as i64)
        .max(0) as usize
}

pub(in crate::server_main) fn extract_significant_search_terms(query: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "and", "at", "for", "from", "in", "of", "on", "the", "to", "vs", "with",
    ];

    let mut seen = HashSet::new();
    query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter_map(|part| {
            let token = part.trim().to_ascii_lowercase();
            if token.len() < 3
                || STOP_WORDS.contains(&token.as_str())
                || !seen.insert(token.clone())
            {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

pub(in crate::server_main) fn short_search_abbreviations() -> &'static [&'static str] {
    &[
        "f1", "nba", "nfl", "nhl", "mlb", "ucl", "epl", "pga", "atp", "wta", "ufc",
    ]
}

pub(in crate::server_main) fn extract_catalog_prefix(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('|') {
        let mut parts = trimmed
            .split('|')
            .filter(|segment| !segment.trim().is_empty());
        return parts
            .next()
            .map(normalize_prefix)
            .filter(|prefix| !prefix.is_empty());
    }

    if let Some((prefix, _)) = trimmed.split_once('|') {
        let prefix = normalize_prefix(prefix);
        if !prefix.is_empty() {
            return Some(prefix);
        }
    }

    if let Some((prefix, _)) = trimmed.split_once(':') {
        let prefix = normalize_prefix(prefix);
        if (2..=4).contains(&prefix.len()) && prefix.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return Some(prefix);
        }
    }

    None
}

pub(in crate::server_main) fn normalize_prefix(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

fn is_country_prefix(prefix: &str) -> bool {
    prefix.len() == 2 && prefix.chars().all(|ch| ch.is_ascii_alphabetic())
}

pub(in crate::server_main) fn normalize_search_text(value: &str) -> String {
    let mut normalized = String::new();
    for ch in value.chars() {
        match ch {
            '+' => normalized.push_str(" plus "),
            '&' | '/' | ':' | '|' | '@' | '-' | '_' | '.' | ',' | '(' | ')' | '[' | ']' | '{'
            | '}' | '\'' | '"' | '!' | '?' | '#' | '*' | ';' => normalized.push(' '),
            _ if ch.is_alphanumeric() || ch.is_whitespace() => normalized.push(ch),
            _ => normalized.push(' '),
        }
    }

    normalized
        .split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(in crate::server_main) fn tokenize_normalized(value: &str) -> Vec<String> {
    normalize_search_text(value)
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

pub(in crate::server_main) fn collect_provider_candidates(
    value: &str,
    known_prefixes: &HashSet<String>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    let mut segments = value
        .split(|ch| matches!(ch, '|' | ':'))
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        segments.push(value.trim());
    }

    for segment in segments {
        let tokens = tokenize_normalized(segment)
            .into_iter()
            .filter(|token| !provider_noise_tokens().contains(&token.as_str()))
            .collect::<Vec<_>>();
        if tokens.is_empty() {
            continue;
        }
        if known_prefixes.contains(&tokens[0].to_ascii_uppercase()) {
            continue;
        }
        if provider_disallowed_roots().contains(&tokens[0].as_str()) {
            continue;
        }

        for len in 1..=tokens.len().min(3) {
            let alias = tokens[..len].join(" ");
            if alias.len() < 3 || !seen.insert(alias.clone()) {
                continue;
            }
            candidates.push(alias);
        }
    }

    candidates
}

pub(in crate::server_main) fn provider_key_from_tokens(tokens: &[String]) -> String {
    if tokens.is_empty() {
        return String::new();
    }
    if tokens.len() >= 2 && matches!(tokens[1].as_str(), "plus" | "play") {
        return format!("{}{}", tokens[0], tokens[1]);
    }
    tokens[0].clone()
}

fn provider_noise_tokens() -> &'static [&'static str] {
    &[
        "hd",
        "uhd",
        "sd",
        "fhd",
        "hevc",
        "raw",
        "vip",
        "ppv",
        "live",
        "event",
        "events",
        "exclusive",
        "now",
        "only",
        "channel",
        "channels",
        "fps",
        "world",
        "and",
    ]
}

fn provider_disallowed_roots() -> &'static [&'static str] {
    &[
        "news",
        "sports",
        "sport",
        "movies",
        "movie",
        "cinema",
        "general",
        "documentary",
        "music",
        "kids",
        "series",
        "live",
        "event",
        "events",
        "next",
        "ended",
        "no",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_search_lexicon() -> SearchLexicon {
        SearchLexicon {
            known_prefixes: ["SE", "UK", "ASIA", "4K"]
                .into_iter()
                .map(String::from)
                .collect(),
            country_prefixes: ["SE", "UK"].into_iter().map(String::from).collect(),
            region_prefixes: ["ASIA", "4K"].into_iter().map(String::from).collect(),
            provider_aliases: vec![
                ProviderAlias {
                    alias: "sky sports".to_string(),
                    normalized_alias: "sky sports".to_string(),
                    alias_tokens: vec!["sky".to_string(), "sports".to_string()],
                    key: "sky".to_string(),
                },
                ProviderAlias {
                    alias: "viaplay".to_string(),
                    normalized_alias: "viaplay".to_string(),
                    alias_tokens: vec!["viaplay".to_string()],
                    key: "viaplay".to_string(),
                },
                ProviderAlias {
                    alias: "tv3".to_string(),
                    normalized_alias: "tv3".to_string(),
                    alias_tokens: vec!["tv3".to_string()],
                    key: "tv3".to_string(),
                },
            ],
            provider_labels: HashMap::from([
                (
                    "sky".to_string(),
                    vec!["sky".to_string(), "sky sports".to_string()],
                ),
                ("viaplay".to_string(), vec!["viaplay".to_string()]),
                ("tv3".to_string(), vec!["tv3".to_string()]),
            ]),
            typo_disabled_words: short_search_abbreviations()
                .iter()
                .map(|value| value.to_string())
                .collect(),
        }
    }

    #[test]
    fn extract_significant_search_terms_drops_stop_words() {
        assert_eq!(
            extract_significant_search_terms("viaplay the masters"),
            vec!["viaplay".to_string(), "masters".to_string()]
        );
    }

    #[test]
    fn collect_provider_candidates_extracts_expected_aliases() {
        let known_prefixes = ["SE", "AR", "4K"]
            .into_iter()
            .map(String::from)
            .collect::<HashSet<_>>();
        let aliases = collect_provider_candidates(
            "Bromley vs Shrewsbury @ Apr 7 20:55 :Viaplay SE 07",
            &known_prefixes,
        );
        assert!(aliases.iter().any(|alias| alias == "viaplay"));
        assert!(aliases.iter().any(|alias| alias == "viaplay se"));
    }

    #[test]
    fn collect_provider_candidates_keeps_compound_provider_labels() {
        let known_prefixes = ["4K"].into_iter().map(String::from).collect::<HashSet<_>>();
        let aliases = collect_provider_candidates("4K: SKY SPORTS F1 UHD", &known_prefixes);
        assert!(aliases.iter().any(|alias| alias == "sky"));
        assert!(aliases.iter().any(|alias| alias == "sky sports"));
    }

    #[test]
    fn parse_search_query_supports_filter_only_country_prefix() {
        let parsed = parse_search_query("se:", &sample_search_lexicon());
        assert_eq!(parsed.search, "");
        assert_eq!(parsed.countries, vec!["SE".to_string()]);
        assert_eq!(parsed.filter.as_deref(), Some(r#"country_code = "SE""#));
    }

    #[test]
    fn parse_search_query_extracts_country_and_provider_filters() {
        let parsed = parse_search_query("se: viaplay", &sample_search_lexicon());
        assert_eq!(parsed.search, "");
        assert_eq!(parsed.providers, vec!["viaplay".to_string()]);
        assert_eq!(
            parsed.filter.as_deref(),
            Some(r#"country_code = "SE" AND provider_key = "viaplay""#)
        );
    }

    #[test]
    fn parse_search_query_extracts_provider_from_tail_query() {
        let parsed = parse_search_query("the masters viaplay", &sample_search_lexicon());
        assert_eq!(parsed.search, "the masters");
        assert_eq!(parsed.providers, vec!["viaplay".to_string()]);
        assert_eq!(
            parsed.filter.as_deref(),
            Some(r#"provider_key = "viaplay""#)
        );
    }

    #[test]
    fn parse_search_query_supports_provider_only_alias() {
        let parsed = parse_search_query("sky sports", &sample_search_lexicon());
        assert_eq!(parsed.search, "");
        assert_eq!(parsed.providers, vec!["sky".to_string()]);
        assert_eq!(parsed.filter.as_deref(), Some(r#"provider_key = "sky""#));
    }

    #[test]
    fn parse_search_query_keeps_channel_like_aliases_as_free_text() {
        let parsed = parse_search_query("se tv3", &sample_search_lexicon());
        assert_eq!(parsed.search, "se tv3");
        assert!(parsed.providers.is_empty());
        assert!(parsed.filter.is_none());
    }

    #[test]
    fn parse_search_query_leaves_broad_category_as_free_text() {
        let parsed = parse_search_query("sports", &sample_search_lexicon());
        assert_eq!(parsed.search, "sports");
        assert!(parsed.filter.is_none());
    }

    #[test]
    fn parse_search_query_ignores_unknown_or_too_short_prefixes() {
        let malformed = parse_search_query("zz: viaplay", &sample_search_lexicon());
        assert_eq!(malformed.search, "zz:");
        assert_eq!(malformed.providers, vec!["viaplay".to_string()]);

        let too_short = parse_search_query("s: viaplay", &sample_search_lexicon());
        assert_eq!(too_short.search, "s:");
        assert_eq!(too_short.providers, vec!["viaplay".to_string()]);
    }

    #[test]
    fn build_meili_synonyms_keeps_curated_groups_small() {
        let synonyms = build_meili_synonyms(&sample_search_lexicon());
        assert_eq!(synonyms.get("viaplay"), Some(&vec!["via play".to_string()]));
        assert_eq!(synonyms.get("f1"), Some(&vec!["formula 1".to_string()]));
        assert!(!synonyms.contains_key("sky sports"));
    }

    #[test]
    fn meili_channel_primary_limit_covers_the_requested_page() {
        assert_eq!(meili_channel_primary_limit(120, 30), 150);
        assert_eq!(meili_channel_primary_limit(0, 30), 30);
    }
}
