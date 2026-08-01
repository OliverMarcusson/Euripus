use super::*;

#[test]
fn chart_defaults_to_trending_and_rejects_unknown_values() {
    assert_eq!(normalize_chart(None).unwrap(), CHART_TRENDING);
    assert_eq!(normalize_chart(Some("popular")).unwrap(), CHART_POPULAR);
    assert_eq!(normalize_chart(Some("top_rated")).unwrap(), CHART_TOP_RATED);
    assert!(matches!(
        normalize_chart(Some("best")),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn media_type_defaults_to_movie_and_rejects_unknown_values() {
    assert_eq!(normalize_media_type(None).unwrap(), MEDIA_TYPE_MOVIE);
    assert_eq!(
        normalize_media_type(Some("series")).unwrap(),
        MEDIA_TYPE_SERIES
    );
    assert!(matches!(
        normalize_media_type(Some("tv")),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn global_country_mode_ignores_any_supplied_country() {
    let configured = vec!["SE".to_string()];
    assert_eq!(
        normalize_country(&configured, None, None).unwrap(),
        (COUNTRY_MODE_GLOBAL, String::new())
    );
    // The table's CHECK constraint requires an empty code for global charts, so a stray
    // country parameter must not leak through.
    assert_eq!(
        normalize_country(&configured, Some("global"), Some("SE")).unwrap(),
        (COUNTRY_MODE_GLOBAL, String::new())
    );
}

#[test]
fn country_modes_require_a_configured_country() {
    let configured = vec!["SE".to_string(), "US".to_string()];
    assert_eq!(
        normalize_country(&configured, Some("available_in"), Some("se")).unwrap(),
        (COUNTRY_MODE_AVAILABLE_IN, "SE".to_string())
    );
    assert_eq!(
        normalize_country(&configured, Some("from"), Some("US")).unwrap(),
        (COUNTRY_MODE_FROM, "US".to_string())
    );

    // Unconfigured countries are never refreshed, so an empty chart would look like a bug
    // to the user. Reject instead.
    assert!(matches!(
        normalize_country(&configured, Some("from"), Some("JP")),
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        normalize_country(&configured, Some("from"), None),
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        normalize_country(&configured, Some("from"), Some("   ")),
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        normalize_country(&configured, Some("nearby"), Some("SE")),
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn tmdb_image_urls_are_absolute_and_skip_missing_paths() {
    assert_eq!(
        tmdb_image_url(Some("/abc.jpg"), "w500").as_deref(),
        Some("https://image.tmdb.org/t/p/w500/abc.jpg")
    );
    assert_eq!(tmdb_image_url(None, "w500"), None);
    assert_eq!(tmdb_image_url(Some(""), "w500"), None);
}
