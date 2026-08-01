use super::*;

/// TMDB pages hold 20 results each, so this caps a chart at 100 entries.
const CHART_PAGES: u32 = 5;
/// Alternative titles cost one request per title. Capping the per-cycle backfill keeps a
/// refresh bounded; match quality improves over the first few cycles instead of stalling
/// the chart refresh behind hundreds of serial requests.
const ALTERNATIVE_TITLE_BACKFILL_LIMIT: i64 = 120;

#[derive(Debug, Deserialize)]
struct TmdbListResponse {
    #[serde(default)]
    results: Vec<TmdbListItem>,
}

/// One row of a chart. Movie and TV responses differ in which field carries the name and
/// the date, so both spellings are accepted and normalized on read.
#[derive(Debug, Deserialize)]
struct TmdbListItem {
    id: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    original_title: Option<String>,
    #[serde(default)]
    original_name: Option<String>,
    #[serde(default)]
    original_language: Option<String>,
    #[serde(default)]
    origin_country: Vec<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    backdrop_path: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    first_air_date: Option<String>,
    #[serde(default)]
    vote_average: Option<f64>,
    #[serde(default)]
    vote_count: Option<i32>,
    #[serde(default)]
    popularity: Option<f64>,
}

impl TmdbListItem {
    fn display_title(&self) -> Option<&str> {
        self.title.as_deref().or(self.name.as_deref())
    }

    fn original(&self) -> Option<&str> {
        self.original_title
            .as_deref()
            .or(self.original_name.as_deref())
    }

    fn date(&self) -> Option<&str> {
        self.release_date
            .as_deref()
            .or(self.first_air_date.as_deref())
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Deserialize)]
struct TmdbAlternativeTitlesResponse {
    #[serde(default)]
    titles: Vec<TmdbAlternativeTitle>,
    #[serde(default)]
    results: Vec<TmdbAlternativeTitle>,
}

#[derive(Debug, Deserialize)]
struct TmdbAlternativeTitle {
    #[serde(default)]
    title: String,
}

/// Every chart Discover can serve. Charts are refreshed for this whole matrix, so adding
/// a country to `APP_TMDB_COUNTRIES` costs four extra chart refreshes per cycle.
pub(super) fn chart_definitions(countries: &[String]) -> Vec<ChartDefinition> {
    let mut definitions = Vec::new();
    for media_type in [MEDIA_TYPE_MOVIE, MEDIA_TYPE_SERIES] {
        for chart in [CHART_TRENDING, CHART_POPULAR, CHART_TOP_RATED] {
            definitions.push(ChartDefinition {
                chart,
                media_type,
                country_mode: COUNTRY_MODE_GLOBAL,
                country_code: String::new(),
            });
        }
        for country in countries {
            // TMDB has no per-country popularity, so these two modes are the only honest
            // country dimensions available: licensed there, or made there.
            definitions.push(ChartDefinition {
                chart: CHART_POPULAR,
                media_type,
                country_mode: COUNTRY_MODE_AVAILABLE_IN,
                country_code: country.clone(),
            });
            definitions.push(ChartDefinition {
                chart: CHART_POPULAR,
                media_type,
                country_mode: COUNTRY_MODE_FROM,
                country_code: country.clone(),
            });
            definitions.push(ChartDefinition {
                chart: CHART_TOP_RATED,
                media_type,
                country_mode: COUNTRY_MODE_FROM,
                country_code: country.clone(),
            });
        }
    }
    definitions
}

#[derive(Debug, Clone)]
pub(super) struct ChartDefinition {
    pub(super) chart: &'static str,
    pub(super) media_type: &'static str,
    pub(super) country_mode: &'static str,
    pub(super) country_code: String,
}

impl ChartDefinition {
    /// Maps a chart onto a TMDB endpoint and query string.
    ///
    /// `watch_region` cannot stand alone — TMDB requires it alongside
    /// `with_watch_monetization_types` or `with_watch_providers` — so "available in" asks
    /// for every monetization type rather than naming individual providers.
    fn request_path_and_query(&self, page: u32) -> (String, Vec<(String, String)>) {
        let tmdb_media = if self.media_type == MEDIA_TYPE_MOVIE {
            "movie"
        } else {
            "tv"
        };
        let mut query = vec![("page".to_string(), page.to_string())];

        match (self.chart, self.country_mode) {
            (CHART_TRENDING, _) => (format!("trending/{tmdb_media}/week"), query),
            (chart, COUNTRY_MODE_GLOBAL) => {
                let endpoint = if chart == CHART_TOP_RATED {
                    "top_rated"
                } else {
                    "popular"
                };
                (format!("{tmdb_media}/{endpoint}"), query)
            }
            (chart, COUNTRY_MODE_AVAILABLE_IN) => {
                query.push(("watch_region".to_string(), self.country_code.clone()));
                query.push((
                    "with_watch_monetization_types".to_string(),
                    "flatrate|free|ads|rent|buy".to_string(),
                ));
                query.push(("sort_by".to_string(), sort_for_chart(chart).to_string()));
                (format!("discover/{tmdb_media}"), query)
            }
            (chart, _) => {
                query.push(("with_origin_country".to_string(), self.country_code.clone()));
                query.push(("sort_by".to_string(), sort_for_chart(chart).to_string()));
                // Top rated by raw average is dominated by titles with a handful of votes,
                // so require a floor the way TMDB's own top-rated chart does.
                if chart == CHART_TOP_RATED {
                    query.push(("vote_count.gte".to_string(), "50".to_string()));
                }
                (format!("discover/{tmdb_media}"), query)
            }
        }
    }
}

fn sort_for_chart(chart: &str) -> &'static str {
    if chart == CHART_TOP_RATED {
        "vote_average.desc"
    } else {
        "popularity.desc"
    }
}

pub(super) fn spawn_refresh_worker(state: AppState) -> Option<JoinHandle<()>> {
    if state.config.tmdb_api_key.is_none() {
        info!("APP_TMDB_API_KEY is not set, Discover charts are disabled");
        return None;
    }
    Some(tokio::spawn(async move {
        let mut interval = tokio::time::interval(DISCOVER_REFRESH_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(error) = refresh_all(&state).await {
                error!("Discover chart refresh failed: {error:?}");
            }
        }
    }))
}

async fn refresh_all(state: &AppState) -> Result<()> {
    let definitions = chart_definitions(&state.config.tmdb_countries);
    for definition in definitions {
        if let Err(error) = refresh_chart(state, &definition).await {
            // One failing chart should not abandon the rest of the matrix.
            error!(
                chart = definition.chart,
                media_type = definition.media_type,
                country_mode = definition.country_mode,
                country_code = definition.country_code,
                "failed to refresh Discover chart: {error:?}"
            );
        }
    }
    if let Err(error) = backfill_alternative_titles(state).await {
        error!("failed to backfill TMDB alternative titles: {error:?}");
    }
    Ok(())
}

async fn refresh_chart(state: &AppState, definition: &ChartDefinition) -> Result<()> {
    let mut items = Vec::new();
    for page in 1..=CHART_PAGES {
        let (path, query) = definition.request_path_and_query(page);
        let response: TmdbListResponse = request_tmdb(state, &path, &query).await?;
        if response.results.is_empty() {
            break;
        }
        items.extend(response.results);
    }
    if items.is_empty() {
        return Ok(());
    }

    let mut tx = state.pool.begin().await?;
    for item in &items {
        upsert_title(&mut tx, definition.media_type, item).await?;
    }

    // Replace the chart wholesale: ranks shift on every refresh, so merging would leave
    // stale positions behind.
    sqlx::query(
        r#"
        DELETE FROM tmdb_chart_entries
        WHERE chart = $1 AND media_type = $2 AND country_mode = $3 AND country_code = $4
        "#,
    )
    .bind(definition.chart)
    .bind(definition.media_type)
    .bind(definition.country_mode)
    .bind(&definition.country_code)
    .execute(&mut *tx)
    .await?;

    let mut seen = HashSet::new();
    let mut rank = 0_i32;
    for item in &items {
        // TMDB paginates over a moving popularity ranking, so the same id can arrive on
        // two pages. Deduplicate before assigning ranks or the primary key collides.
        if !seen.insert(item.id) {
            continue;
        }
        rank += 1;
        sqlx::query(
            r#"
            INSERT INTO tmdb_chart_entries
              (chart, media_type, country_mode, country_code, rank, tmdb_id, refreshed_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
        )
        .bind(definition.chart)
        .bind(definition.media_type)
        .bind(definition.country_mode)
        .bind(&definition.country_code)
        .bind(rank)
        .bind(item.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn upsert_title(
    tx: &mut Transaction<'_, Postgres>,
    media_type: &str,
    item: &TmdbListItem,
) -> Result<()> {
    let Some(title) = item.display_title().filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let original = item.original().unwrap_or(title);
    let date = item.date();

    // match_keys is built by the same SQL normalizer the on_demand_titles generated column
    // uses, so the two sides cannot drift. Alternative titles are fetched separately, so
    // the recompute reads whatever raw ones the row already carries rather than dropping
    // them.
    sqlx::query(
        r#"
        INSERT INTO tmdb_titles (
          media_type, tmdb_id, title, original_title, original_language, origin_countries,
          overview, poster_path, backdrop_path, release_date, release_year,
          vote_average, vote_count, popularity, match_keys, refreshed_at
        )
        SELECT
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
          discover_title_match_keys($3, $4, '{}'),
          NOW()
        ON CONFLICT (media_type, tmdb_id) DO UPDATE SET
          title = EXCLUDED.title,
          original_title = EXCLUDED.original_title,
          original_language = EXCLUDED.original_language,
          origin_countries = EXCLUDED.origin_countries,
          overview = EXCLUDED.overview,
          poster_path = EXCLUDED.poster_path,
          backdrop_path = EXCLUDED.backdrop_path,
          release_date = EXCLUDED.release_date,
          release_year = EXCLUDED.release_year,
          vote_average = EXCLUDED.vote_average,
          vote_count = EXCLUDED.vote_count,
          popularity = EXCLUDED.popularity,
          match_keys = discover_title_match_keys(
            EXCLUDED.title, EXCLUDED.original_title, tmdb_titles.alternative_titles
          ),
          refreshed_at = NOW()
        "#,
    )
    .bind(media_type)
    .bind(item.id)
    .bind(title)
    .bind(original)
    .bind(item.original_language.as_deref())
    .bind(&item.origin_country)
    .bind(item.overview.as_deref())
    .bind(item.poster_path.as_deref())
    .bind(item.backdrop_path.as_deref())
    .bind(date)
    .bind(release_year(date))
    .bind(item.vote_average)
    .bind(item.vote_count)
    .bind(item.popularity)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Adds localized release names to `match_keys` so a Swedish provider listing matches the
/// English TMDB title. Runs after the chart refresh and is bounded per cycle.
async fn backfill_alternative_titles(state: &AppState) -> Result<()> {
    let pending = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT media_type, tmdb_id FROM tmdb_titles
        WHERE alternative_titles_fetched_at IS NULL
        ORDER BY refreshed_at
        LIMIT $1
        "#,
    )
    .bind(ALTERNATIVE_TITLE_BACKFILL_LIMIT)
    .fetch_all(&state.pool)
    .await?;

    for (media_type, tmdb_id) in pending {
        let tmdb_media = if media_type == MEDIA_TYPE_MOVIE {
            "movie"
        } else {
            "tv"
        };
        let path = format!("{tmdb_media}/{tmdb_id}/alternative_titles");
        let titles = match request_tmdb::<TmdbAlternativeTitlesResponse>(state, &path, &[]).await {
            Ok(response) => {
                // Movies return `titles`, TV returns `results`.
                let mut all = response.titles;
                all.extend(response.results);
                all
            }
            Err(error) => {
                warn!(%tmdb_id, media_type, "failed to fetch alternative titles: {error}");
                continue;
            }
        };

        let names = titles
            .into_iter()
            .map(|entry| entry.title)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        // The raw names are stored alongside the derived keys so a future change to the
        // normalizer can recompute locally instead of re-querying TMDB for every title.
        //
        // The timestamp is stamped even when TMDB returned nothing, otherwise this title is
        // retried on every cycle forever and starves the rest of the queue.
        sqlx::query(
            r#"
            UPDATE tmdb_titles SET
              alternative_titles = $3::text[],
              match_keys = discover_title_match_keys(title, original_title, $3::text[]),
              alternative_titles_fetched_at = NOW()
            WHERE media_type = $1 AND tmdb_id = $2
            "#,
        )
        .bind(&media_type)
        .bind(tmdb_id)
        .bind(&names)
        .execute(&state.pool)
        .await?;
    }
    Ok(())
}

async fn request_tmdb<T: serde::de::DeserializeOwned>(
    state: &AppState,
    path: &str,
    query: &[(String, String)],
) -> Result<T> {
    let api_key = state
        .config
        .tmdb_api_key
        .as_deref()
        .ok_or_else(|| anyhow!("APP_TMDB_API_KEY is not configured"))?;
    let mut url = Url::parse(TMDB_API_BASE_URL)?.join(path)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("language", "en-US");
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
    }

    let request = state.provider_http_client.get(url.clone());
    // TMDB accepts either a v4 bearer token or a v3 api_key query parameter. Bearer tokens
    // are long and contain dots; v3 keys are 32 hex characters.
    let request = if api_key.contains('.') {
        request.bearer_auth(api_key)
    } else {
        request.query(&[("api_key", api_key)])
    };

    let response = request
        .send()
        .await
        .with_context(|| format!("failed to request TMDB {path}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("TMDB {path} returned {status}: {body}"));
    }
    response
        .json::<T>()
        .await
        .with_context(|| format!("failed to decode TMDB {path}"))
}

#[cfg(test)]
#[path = "tmdb_tests.rs"]
mod tests;
