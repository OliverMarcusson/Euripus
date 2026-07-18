#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::server_main) struct ParsedSearch {
    pub(in crate::server_main) search: String,
    pub(in crate::server_main) countries: Vec<String>,
    pub(in crate::server_main) providers: Vec<String>,
    pub(in crate::server_main) ppv: Option<bool>,
    pub(in crate::server_main) vip: Option<bool>,
    pub(in crate::server_main) require_epg: bool,
}

pub(in crate::server_main) fn parse_search_query(query: &str) -> ParsedSearch {
    let mut countries = Vec::new();
    let mut providers = Vec::new();
    let mut ppv = None;
    let mut vip = None;
    let mut require_epg = false;
    let search = query
        .split_whitespace()
        .filter_map(|token| {
            let normalized_token = token.trim().to_ascii_lowercase();
            if normalized_token.is_empty() {
                return None;
            }
            if let Some(value) = normalized_token.strip_prefix("country:") {
                let normalized_value = value.trim().to_ascii_lowercase();
                if !normalized_value.is_empty()
                    && !countries.iter().any(|country| country == &normalized_value)
                {
                    countries.push(normalized_value);
                }
                return None;
            }
            if let Some(value) = normalized_token.strip_prefix("provider:") {
                let normalized_value = value.trim().to_ascii_lowercase();
                if !normalized_value.is_empty()
                    && !providers
                        .iter()
                        .any(|provider| provider == &normalized_value)
                {
                    providers.push(normalized_value);
                }
                return None;
            }
            if normalized_token == "ppv" {
                ppv = Some(true);
                return None;
            }
            if normalized_token == "!ppv" {
                ppv = Some(false);
                return None;
            }
            if normalized_token == "vip" {
                vip = Some(true);
                return None;
            }
            if normalized_token == "!vip" {
                vip = Some(false);
                return None;
            }
            if normalized_token == "epg" {
                require_epg = true;
                return None;
            }
            Some(token.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    ParsedSearch {
        search,
        countries,
        providers,
        ppv,
        vip,
        require_epg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_search_query_supports_filter_only_country_prefix() {
        let parsed = parse_search_query("country:se");
        assert_eq!(parsed.search, "");
        assert_eq!(parsed.countries, vec!["se".to_string()]);
    }

    #[test]
    fn parse_search_query_extracts_filters_and_free_text() {
        let parsed = parse_search_query("the masters country:se provider:viaplay !ppv vip epg");
        assert_eq!(parsed.search, "the masters");
        assert_eq!(parsed.countries, vec!["se".to_string()]);
        assert_eq!(parsed.providers, vec!["viaplay".to_string()]);
        assert_eq!(parsed.ppv, Some(false));
        assert_eq!(parsed.vip, Some(true));
        assert!(parsed.require_epg);
    }

    #[test]
    fn parse_search_query_keeps_free_text_without_operators() {
        let parsed = parse_search_query("se tv3");
        assert_eq!(parsed.search, "se tv3");
        assert!(parsed.providers.is_empty());
    }
}
