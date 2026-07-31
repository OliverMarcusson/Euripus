use super::*;

fn sample_candidate(id: Uuid, channel_name: &str) -> AiPpvCandidate {
    let id_string = id.to_string();
    let channel = ChannelResponse {
        id,
        profile_id: Uuid::from_u128(2),
        name: channel_name.to_string(),
        logo_url: Some("https://example.com/logo.png".to_string()),
        category_name: Some("SE| VIAPLAY PPV".to_string()),
        remote_stream_id: 5,
        epg_channel_id: None,
        has_epg: false,
        has_catchup: false,
        archive_duration_hours: None,
        stream_extension: Some("m3u8".to_string()),
        is_favorite: false,
        is_ppv: true,
        is_ppv_favorite: false,
    };

    AiPpvCandidate {
        id: id_string.clone(),
        channel,
        program: None,
        prompt: AiPpvPromptCandidate {
            id: id_string,
            channel_title: channel_name.to_string(),
            category: Some("SE| VIAPLAY PPV".to_string()),
            provider: Some("viaplay".to_string()),
            country: Some("se".to_string()),
            is_ppv: true,
            program_title: None,
            starts_at: None,
            ends_at: None,
        },
        local_score: 0.42,
    }
}

#[test]
fn prompt_payload_omits_stream_urls_and_remote_ids() {
    let candidate = sample_candidate(Uuid::from_u128(10), "Sweden vs Japan");
    let prompt =
        build_ai_ppv_prompt("sweden japan", &[candidate.prompt], 5).expect("prompt should build");

    assert!(prompt.contains("Sweden vs Japan"));
    assert!(!prompt.contains("remoteStreamId"));
    assert!(!prompt.contains("logoUrl"));
    assert!(!prompt.contains("https://example.com"));
}

#[test]
fn parser_rejects_unknown_ids_and_invalid_confidence_values() {
    let known_id = Uuid::from_u128(11);
    let candidate = sample_candidate(known_id, "Sweden vs Japan");
    let content = format!(
        r#"{{
              "matches": [
                {{"id":"{}","confidence":0.91,"reason":"teams match","matchedTerms":["sweden","japan"]}},
                {{"id":"{}","confidence":1.8,"reason":"bad confidence","matchedTerms":[]}},
                {{"id":"missing","confidence":0.93,"reason":"unknown","matchedTerms":[]}}
              ]
            }}"#,
        known_id, known_id
    );

    let items =
        parse_ai_ppv_model_response(&content, &[candidate], 10).expect("valid model response");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].channel.id, known_id);
    assert_eq!(items[0].matched_terms, vec!["sweden", "japan"]);
}

#[test]
fn parser_accepts_json_wrapped_in_text() {
    let known_id = Uuid::from_u128(12);
    let candidate = sample_candidate(known_id, "Cup Final");
    let content = format!(
        "Here is JSON: {{\"matches\":[{{\"id\":\"{known_id}\",\"confidence\":0.5,\"reason\":\"title\",\"matchedTerms\":[]}}]}}"
    );

    let items =
        parse_ai_ppv_model_response(&content, &[candidate], 10).expect("wrapped json should parse");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].reason, "title");
}
