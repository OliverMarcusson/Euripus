use super::*;

fn definition(
    chart: &'static str,
    media_type: &'static str,
    country_mode: &'static str,
    country: &str,
) -> ChartDefinition {
    ChartDefinition {
        chart,
        media_type,
        country_mode,
        country_code: country.to_string(),
    }
}

fn query_value(query: &[(String, String)], key: &str) -> Option<String> {
    query
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}

#[test]
fn trending_uses_the_dedicated_endpoint_per_media_type() {
    let (path, query) = definition(CHART_TRENDING, MEDIA_TYPE_MOVIE, COUNTRY_MODE_GLOBAL, "")
        .request_path_and_query(1);
    assert_eq!(path, "trending/movie/week");
    assert_eq!(query_value(&query, "page").as_deref(), Some("1"));

    let (path, _) = definition(CHART_TRENDING, MEDIA_TYPE_SERIES, COUNTRY_MODE_GLOBAL, "")
        .request_path_and_query(2);
    assert_eq!(path, "trending/tv/week");
}

#[test]
fn global_charts_use_the_plain_list_endpoints() {
    let (path, _) = definition(CHART_POPULAR, MEDIA_TYPE_MOVIE, COUNTRY_MODE_GLOBAL, "")
        .request_path_and_query(1);
    assert_eq!(path, "movie/popular");

    let (path, _) = definition(CHART_TOP_RATED, MEDIA_TYPE_SERIES, COUNTRY_MODE_GLOBAL, "")
        .request_path_and_query(1);
    assert_eq!(path, "tv/top_rated");
}

#[test]
fn available_in_always_pairs_watch_region_with_monetization_types() {
    // TMDB rejects watch_region on its own; losing this pairing would silently return an
    // unfiltered global chart labelled as a country chart.
    let (path, query) = definition(
        CHART_POPULAR,
        MEDIA_TYPE_MOVIE,
        COUNTRY_MODE_AVAILABLE_IN,
        "SE",
    )
    .request_path_and_query(1);
    assert_eq!(path, "discover/movie");
    assert_eq!(query_value(&query, "watch_region").as_deref(), Some("SE"));
    assert!(query_value(&query, "with_watch_monetization_types").is_some());
    assert_eq!(
        query_value(&query, "sort_by").as_deref(),
        Some("popularity.desc")
    );
}

#[test]
fn from_country_filters_by_origin_and_floors_top_rated_vote_counts() {
    let (path, query) = definition(CHART_POPULAR, MEDIA_TYPE_SERIES, COUNTRY_MODE_FROM, "SE")
        .request_path_and_query(1);
    assert_eq!(path, "discover/tv");
    assert_eq!(
        query_value(&query, "with_origin_country").as_deref(),
        Some("SE")
    );
    assert!(query_value(&query, "vote_count.gte").is_none());

    let (_, query) = definition(CHART_TOP_RATED, MEDIA_TYPE_MOVIE, COUNTRY_MODE_FROM, "SE")
        .request_path_and_query(1);
    assert_eq!(
        query_value(&query, "sort_by").as_deref(),
        Some("vote_average.desc")
    );
    // Without a vote floor, vote_average.desc returns obscure titles with three votes.
    assert_eq!(query_value(&query, "vote_count.gte").as_deref(), Some("50"));
}

#[test]
fn chart_definitions_cover_every_configured_country_without_duplicates() {
    let countries = vec!["SE".to_string(), "US".to_string()];
    let definitions = chart_definitions(&countries);

    let keys = definitions
        .iter()
        .map(|item| {
            (
                item.chart,
                item.media_type,
                item.country_mode,
                item.country_code.clone(),
            )
        })
        .collect::<HashSet<_>>();
    assert_eq!(keys.len(), definitions.len(), "definitions must be unique");

    // Global charts never carry a country code, matching the table's CHECK constraint.
    for definition in &definitions {
        assert_eq!(
            definition.country_mode == COUNTRY_MODE_GLOBAL,
            definition.country_code.is_empty()
        );
    }

    for country in &countries {
        for media_type in [MEDIA_TYPE_MOVIE, MEDIA_TYPE_SERIES] {
            assert!(definitions.iter().any(|item| {
                item.country_mode == COUNTRY_MODE_AVAILABLE_IN
                    && item.media_type == media_type
                    && &item.country_code == country
            }));
            assert!(definitions.iter().any(|item| {
                item.country_mode == COUNTRY_MODE_FROM
                    && item.media_type == media_type
                    && &item.country_code == country
            }));
        }
    }
}

#[test]
fn no_countries_configured_leaves_only_global_charts() {
    let definitions = chart_definitions(&[]);
    assert!(!definitions.is_empty());
    assert!(
        definitions
            .iter()
            .all(|item| item.country_mode == COUNTRY_MODE_GLOBAL)
    );
}

#[test]
fn release_year_reads_the_leading_year_of_tmdb_dates() {
    assert_eq!(release_year(Some("1999-03-31")), Some(1999));
    assert_eq!(release_year(Some("2024")), Some(2024));
    assert_eq!(release_year(None), None);
    assert_eq!(release_year(Some("")), None);
    assert_eq!(release_year(Some("not-a-date")), None);
}
