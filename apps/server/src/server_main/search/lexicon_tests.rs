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
