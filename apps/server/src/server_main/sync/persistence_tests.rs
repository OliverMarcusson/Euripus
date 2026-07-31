use super::*;

#[test]
fn total_phases_for_job_supports_channel_syncs() {
    assert_eq!(total_phases_for_job("channels"), CHANNEL_SYNC_TOTAL_PHASES);
    assert_eq!(total_phases_for_job("epg"), EPG_SYNC_TOTAL_PHASES);
    assert_eq!(total_phases_for_job("full"), FULL_SYNC_TOTAL_PHASES);
}

#[test]
fn resolves_external_epg_programmes_by_xmltv_display_name() {
    let now = Utc::now();
    let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
        id: Uuid::from_u128(11),
        name: "TV4 HD".to_string(),
        remote_stream_id: 4,
        epg_channel_id: None,
        has_catchup: true,
    }]);
    let feed = FetchedEpgFeed {
        source_id: Some(Uuid::from_u128(12)),
        source_kind: "external".to_string(),
        source_label: "https://example.com/tv.xml.gz".to_string(),
        priority: 0,
        feed: XmltvFeed {
            channels: HashMap::from([(
                "external-tv4".to_string(),
                XmltvChannel {
                    id: "external-tv4".to_string(),
                    display_names: vec!["TV4 HD".to_string()],
                },
            )]),
            programmes: vec![XmltvProgramme {
                channel_key: "external-tv4".to_string(),
                title: "Morning Show".to_string(),
                description: None,
                start_at: now,
                end_at: now + ChronoDuration::hours(1),
            }],
        },
    };

    let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

    assert_eq!(programmes.len(), 1);
    assert_eq!(programmes[0].channel_name, "TV4 HD");
    assert_eq!(programmes[0].title, "Morning Show");
    assert_eq!(statuses[0].last_matched_count, Some(1));
}

#[test]
fn resolves_external_epg_programmes_with_region_and_quality_decorations() {
    let now = Utc::now();
    let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
        id: Uuid::from_u128(13),
        name: "|SE|TV4 á´´á´° SE".to_string(),
        remote_stream_id: 41,
        epg_channel_id: None,
        has_catchup: true,
    }]);
    let feed = FetchedEpgFeed {
        source_id: Some(Uuid::from_u128(14)),
        source_kind: "external".to_string(),
        source_label: "https://example.com/tv4.xml.gz".to_string(),
        priority: 0,
        feed: XmltvFeed {
            channels: HashMap::from([(
                "tv4.se".to_string(),
                XmltvChannel {
                    id: "tv4.se".to_string(),
                    display_names: vec!["TV4 HD.se".to_string()],
                },
            )]),
            programmes: vec![XmltvProgramme {
                channel_key: "tv4.se".to_string(),
                title: "Evening News".to_string(),
                description: None,
                start_at: now,
                end_at: now + ChronoDuration::hours(1),
            }],
        },
    };

    let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

    assert_eq!(programmes.len(), 1);
    assert_eq!(programmes[0].channel_name, "|SE|TV4 á´´á´° SE");
    assert_eq!(programmes[0].title, "Evening News");
    assert_eq!(statuses[0].last_matched_count, Some(1));
}

#[test]
fn resolves_external_epg_programmes_when_feed_uses_text_variant_names() {
    let now = Utc::now();
    let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
        id: Uuid::from_u128(15),
        name: "|SE|TV4 FAKTA".to_string(),
        remote_stream_id: 42,
        epg_channel_id: None,
        has_catchup: false,
    }]);
    let feed = FetchedEpgFeed {
        source_id: Some(Uuid::from_u128(16)),
        source_kind: "external".to_string(),
        source_label: "https://example.com/tv4fakta.xml.gz".to_string(),
        priority: 0,
        feed: XmltvFeed {
            channels: HashMap::from([(
                "tv4-fakta.se".to_string(),
                XmltvChannel {
                    id: "tv4-fakta.se".to_string(),
                    display_names: vec!["TV4 Fakta - Text.se".to_string()],
                },
            )]),
            programmes: vec![XmltvProgramme {
                channel_key: "tv4-fakta.se".to_string(),
                title: "Documentary Hour".to_string(),
                description: None,
                start_at: now,
                end_at: now + ChronoDuration::hours(1),
            }],
        },
    };

    let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

    assert_eq!(programmes.len(), 1);
    assert_eq!(programmes[0].channel_name, "|SE|TV4 FAKTA");
    assert_eq!(programmes[0].title, "Documentary Hour");
    assert_eq!(statuses[0].last_matched_count, Some(1));
}

#[test]
fn keeps_higher_priority_epg_source_when_timeslots_overlap() {
    let now = Utc::now();
    let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
        id: Uuid::from_u128(21),
        name: "Arena 1".to_string(),
        remote_stream_id: 1,
        epg_channel_id: Some("arena.1".to_string()),
        has_catchup: true,
    }]);
    let primary_feed = FetchedEpgFeed {
        source_id: Some(Uuid::from_u128(22)),
        source_kind: "external".to_string(),
        source_label: "https://example.com/primary.xml.gz".to_string(),
        priority: 0,
        feed: XmltvFeed {
            channels: HashMap::new(),
            programmes: vec![XmltvProgramme {
                channel_key: "arena.1".to_string(),
                title: "Primary Listing".to_string(),
                description: None,
                start_at: now,
                end_at: now + ChronoDuration::hours(2),
            }],
        },
    };
    let fallback_feed = FetchedEpgFeed {
        source_id: None,
        source_kind: "xtream".to_string(),
        source_label: "https://provider.example.com/xmltv.php".to_string(),
        priority: 1,
        feed: XmltvFeed {
            channels: HashMap::new(),
            programmes: vec![XmltvProgramme {
                channel_key: "arena.1".to_string(),
                title: "Fallback Listing".to_string(),
                description: None,
                start_at: now,
                end_at: now + ChronoDuration::hours(2),
            }],
        },
    };

    let (programmes, statuses) = resolve_epg_programmes(&[primary_feed, fallback_feed], &lookup);

    assert_eq!(programmes.len(), 1);
    assert_eq!(programmes[0].title, "Primary Listing");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].last_program_count, Some(1));
}
