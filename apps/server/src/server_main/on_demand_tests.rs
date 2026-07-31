use super::is_preferred_catalog_title;

#[test]
fn accepts_language_and_platform_title_prefixes() {
    for title in [
        "EN - Slow Horses",
        "SE - Bron",
        "4K-EN - Foundation",
        "4K - SE: Tunna blå linjen",
        "uhd_en | Silo",
        "FHD:EN- The Bear",
        "NF - The Crown",
        "4K-AMZ - Reacher",
        "A+ - Severance",
        "4K-D+ - Andor",
        "MAX - The Last of Us",
        "P+ - Star Trek",
        "NF-DO - Our Planet",
    ] {
        assert!(is_preferred_catalog_title(title), "rejected {title:?}");
    }
}

#[test]
fn rejects_other_or_embedded_language_markers() {
    for title in [
        "ES - La casa de papel",
        "AR-EN-S - Example",
        "IN-EN - Example",
        "ENGLISH SERIES",
        "4K-TOP - Example",
        "SC - Nordic collection",
        "The Last Enemy",
        "",
    ] {
        assert!(!is_preferred_catalog_title(title), "accepted {title:?}");
    }
}
