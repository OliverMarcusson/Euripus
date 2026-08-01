use super::*;

mod matching;
mod tmdb;

use matching::{
    DiscoverTitleRow, count_chart_titles, load_chart_titles, release_year, tmdb_image_url,
};

const TMDB_API_BASE_URL: &str = "https://api.themoviedb.org/3/";
const TMDB_IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p";
const DISCOVER_REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60 * 6);
const DISCOVER_DEFAULT_LIMIT: i64 = 40;
const DISCOVER_MAX_LIMIT: i64 = 100;

const MEDIA_TYPE_MOVIE: &str = "movie";
const MEDIA_TYPE_SERIES: &str = "series";
const CHART_TRENDING: &str = "trending";
const CHART_POPULAR: &str = "popular";
const CHART_TOP_RATED: &str = "top_rated";
const COUNTRY_MODE_GLOBAL: &str = "global";
const COUNTRY_MODE_AVAILABLE_IN: &str = "available_in";
const COUNTRY_MODE_FROM: &str = "from";

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/discover/charts", get(get_charts))
        .route("/discover/titles", get(list_titles))
}

/// Starts the periodic TMDB chart refresh. Returns `None` when no API key is configured,
/// in which case Discover serves whatever is already cached and reports itself disabled.
pub(super) fn spawn_refresh_worker(state: AppState) -> Option<JoinHandle<()>> {
    tmdb::spawn_refresh_worker(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverQuery {
    chart: Option<String>,
    #[serde(rename = "type")]
    media_type: Option<String>,
    country_mode: Option<String>,
    country: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverChartsResponse {
    /// False when `APP_TMDB_API_KEY` is unset. The client renders an explanatory empty
    /// state rather than a broken grid.
    enabled: bool,
    countries: Vec<String>,
    /// Which (chart, country mode) pairs actually have data. TMDB cannot serve every
    /// combination — there is no country-scoped trending chart, for one — so the client
    /// builds its filter controls from this instead of hardcoding the matrix.
    charts: Vec<DiscoverChartOption>,
    last_refreshed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverChartOption {
    chart: String,
    country_mode: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverTitleResponse {
    rank: i32,
    tmdb_id: i64,
    media_type: String,
    title: String,
    original_title: String,
    origin_countries: Vec<String>,
    overview: Option<String>,
    poster_url: Option<String>,
    backdrop_url: Option<String>,
    release_date: Option<String>,
    release_year: Option<i32>,
    vote_average: Option<f64>,
    vote_count: Option<i32>,
    /// `None` means no title in the user's providers matched, so the client greys the card
    /// out and disables playback.
    on_demand_title_id: Option<Uuid>,
    provider_label: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverPageResponse {
    items: Vec<DiscoverTitleResponse>,
    total_count: i64,
    next_offset: Option<i64>,
    available_count: i64,
}

impl From<DiscoverTitleRow> for DiscoverTitleResponse {
    fn from(row: DiscoverTitleRow) -> Self {
        Self {
            rank: row.rank,
            tmdb_id: row.tmdb_id,
            media_type: row.media_type,
            title: row.title,
            original_title: row.original_title,
            origin_countries: row.origin_countries,
            overview: row.overview,
            poster_url: tmdb_image_url(row.poster_path.as_deref(), "w500"),
            backdrop_url: tmdb_image_url(row.backdrop_path.as_deref(), "w780"),
            release_date: row.release_date,
            release_year: row.release_year,
            vote_average: row.vote_average,
            vote_count: row.vote_count,
            on_demand_title_id: row.on_demand_title_id,
            provider_label: row.provider_label,
        }
    }
}

async fn get_charts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<DiscoverChartsResponse> {
    require_auth(&state, &headers).await?;
    let enabled = state.config.tmdb_api_key.is_some();
    let last_refreshed_at =
        sqlx::query_scalar::<_, Option<DateTime<Utc>>>("SELECT MAX(refreshed_at) FROM tmdb_titles")
            .fetch_one(&state.pool)
            .await?;

    let mut charts = Vec::new();
    let mut seen = HashSet::new();
    for definition in tmdb::chart_definitions(&state.config.tmdb_countries) {
        if seen.insert((definition.chart, definition.country_mode)) {
            charts.push(DiscoverChartOption {
                chart: definition.chart.to_string(),
                country_mode: definition.country_mode.to_string(),
            });
        }
    }

    Ok(Json(DiscoverChartsResponse {
        enabled,
        countries: state.config.tmdb_countries.clone(),
        charts,
        last_refreshed_at,
    }))
}

async fn list_titles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DiscoverQuery>,
) -> ApiResult<DiscoverPageResponse> {
    let auth = require_auth(&state, &headers).await?;
    let chart = normalize_chart(query.chart.as_deref())?;
    let media_type = normalize_media_type(query.media_type.as_deref())?;
    let (country_mode, country_code) = normalize_country(
        &state.config.tmdb_countries,
        query.country_mode.as_deref(),
        query.country.as_deref(),
    )?;
    let offset = query.offset.unwrap_or(0).max(0);
    let limit = query
        .limit
        .unwrap_or(DISCOVER_DEFAULT_LIMIT)
        .clamp(1, DISCOVER_MAX_LIMIT);

    let total_count =
        count_chart_titles(&state.pool, chart, media_type, country_mode, &country_code).await?;
    let rows = load_chart_titles(
        &state.pool,
        auth.user_id,
        chart,
        media_type,
        country_mode,
        &country_code,
        offset,
        limit,
    )
    .await?;

    let available_count = rows
        .iter()
        .filter(|row| row.on_demand_title_id.is_some())
        .count() as i64;
    let next_offset = (offset + limit < total_count).then_some(offset + limit);

    Ok(Json(DiscoverPageResponse {
        items: rows.into_iter().map(DiscoverTitleResponse::from).collect(),
        total_count,
        next_offset,
        available_count,
    }))
}

fn normalize_chart(raw: Option<&str>) -> Result<&'static str, AppError> {
    match raw.unwrap_or(CHART_TRENDING) {
        CHART_TRENDING => Ok(CHART_TRENDING),
        CHART_POPULAR => Ok(CHART_POPULAR),
        CHART_TOP_RATED => Ok(CHART_TOP_RATED),
        _ => Err(AppError::BadRequest(
            "Discover chart must be 'trending', 'popular' or 'top_rated'.".to_string(),
        )),
    }
}

fn normalize_media_type(raw: Option<&str>) -> Result<&'static str, AppError> {
    match raw.unwrap_or(MEDIA_TYPE_MOVIE) {
        MEDIA_TYPE_MOVIE => Ok(MEDIA_TYPE_MOVIE),
        MEDIA_TYPE_SERIES => Ok(MEDIA_TYPE_SERIES),
        _ => Err(AppError::BadRequest(
            "Discover type must be 'movie' or 'series'.".to_string(),
        )),
    }
}

/// Pairs the country mode with its code and rejects mismatches, mirroring the CHECK
/// constraint on `tmdb_chart_entries`. Unconfigured countries are rejected rather than
/// silently returning an empty chart, since nothing ever refreshes them.
fn normalize_country(
    configured: &[String],
    raw_mode: Option<&str>,
    raw_country: Option<&str>,
) -> Result<(&'static str, String), AppError> {
    let country = raw_country
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_uppercase());

    match raw_mode.unwrap_or(COUNTRY_MODE_GLOBAL) {
        COUNTRY_MODE_GLOBAL => Ok((COUNTRY_MODE_GLOBAL, String::new())),
        mode @ (COUNTRY_MODE_AVAILABLE_IN | COUNTRY_MODE_FROM) => {
            let country = country.ok_or_else(|| {
                AppError::BadRequest(
                    "A country code is required unless countryMode is 'global'.".to_string(),
                )
            })?;
            if !configured.contains(&country) {
                return Err(AppError::BadRequest(format!(
                    "Discover is not configured for country {country}."
                )));
            }
            let mode = if mode == COUNTRY_MODE_AVAILABLE_IN {
                COUNTRY_MODE_AVAILABLE_IN
            } else {
                COUNTRY_MODE_FROM
            };
            Ok((mode, country))
        }
        _ => Err(AppError::BadRequest(
            "Discover countryMode must be 'global', 'available_in' or 'from'.".to_string(),
        )),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
