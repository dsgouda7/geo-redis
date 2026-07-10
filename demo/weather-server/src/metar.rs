use serde::Deserialize;
use serde_json::Value;

/// One live weather observation from a METAR station.
#[derive(Debug, Clone)]
pub struct WeatherObs {
    /// ICAO station identifier, e.g. "KLAX"
    pub icao_id:   String,
    /// Human-readable station name, e.g. "Los Angeles Intl"
    pub name:      String,
    pub lat:       f64,
    pub lon:       f64,
    /// Temperature °C (None if not reported)
    pub temp_c:    Option<f64>,
    /// Dew point °C
    pub dewp_c:    Option<f64>,
    /// Wind direction degrees true (0 = variable/calm)
    pub wdir:      Option<u16>,
    /// Wind speed knots
    pub wspd_kt:   Option<f64>,
    /// Present-weather string, e.g. "-RA", "TSRA", "FG"
    pub wx_string: String,
    /// Dominant cloud cover, e.g. "FEW", "SCT", "BKN", "OVC"
    pub clouds:    String,
    /// Flight category: VFR / MVFR / IFR / LIFR
    pub flt_cat:   String,
}

// ── API shapes ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MetarRecord {
    #[serde(rename = "icaoId")]
    icao_id:    Option<String>,
    name:       Option<String>,
    lat:        Option<f64>,
    lon:        Option<f64>,
    temp:       Option<f64>,
    dewp:       Option<f64>,
    /// wdir is usually a number but can be the string "VRB"
    wdir:       Option<Value>,
    wspd:       Option<f64>,
    #[serde(rename = "wxString")]
    wx_string:  Option<String>,
    /// clouds is an array of {cover, base} objects
    clouds:     Option<Vec<CloudLayer>>,
    #[serde(rename = "fltCat")]
    flt_cat:    Option<String>,
}

#[derive(Deserialize)]
struct CloudLayer {
    cover: Option<String>,
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Fetches live METAR observations from the Aviation Weather Center.
/// No API key required — entirely public data.
pub async fn fetch_stations(client: &reqwest::Client) -> anyhow::Result<Vec<WeatherObs>> {
    let resp: Vec<MetarRecord> = client
        .get("https://aviationweather.gov/api/data/metar")
        .query(&[("format", "json"), ("hours", "2"), ("bbox", "-90,-180,90,180")])
        .timeout(std::time::Duration::from_secs(30))
        .header("User-Agent", "georedis-weather-demo/1.0")
        .send()
        .await?
        .json()
        .await?;

    // Deduplicate by ICAO ID: the API returns multiple obs per station within
    // the time window; we keep only the first (most recent) for each station.
    let mut seen = std::collections::HashSet::new();
    let stations: Vec<WeatherObs> = resp
        .into_iter()
        .filter_map(parse_record)
        .filter(|s| seen.insert(s.icao_id.clone()))
        .collect();

    Ok(stations)
}

fn parse_record(r: MetarRecord) -> Option<WeatherObs> {
    let icao_id = r.icao_id?.trim().to_string();
    let lat     = r.lat?;
    let lon     = r.lon?;

    if icao_id.is_empty() { return None; }
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) { return None; }

    // wdir can be a number or the string "VRB" (variable)
    let wdir: Option<u16> = r.wdir.as_ref().and_then(|v| {
        if let Some(n) = v.as_u64() { Some(n as u16) }
        else { None } // treat VRB / unknown as None
    });

    // Take the most significant cloud layer cover description
    let clouds = r.clouds.as_deref()
        .and_then(|layers| {
            // Priority: OVC > BKN > SCT > FEW > CLR
            let order = ["OVC", "BKN", "SCT", "FEW", "CLR"];
            for target in order {
                if layers.iter().any(|l| l.cover.as_deref() == Some(target)) {
                    return Some(target.to_string());
                }
            }
            layers.first().and_then(|l| l.cover.clone())
        })
        .unwrap_or_default();

    Some(WeatherObs {
        icao_id,
        name:      r.name.unwrap_or_default().trim().to_string(),
        lat, lon,
        temp_c:    r.temp,
        dewp_c:    r.dewp,
        wdir,
        wspd_kt:   r.wspd,
        wx_string: r.wx_string.unwrap_or_default().trim().to_string(),
        clouds,
        flt_cat:   r.flt_cat.unwrap_or_default(),
    })
}

/// Map temperature to an "altitude" value (metres) that drives the existing
/// Leaflet UI altitude colour scale as a temperature heat-map:
///
///   > +35 °C  →    0 m  (red   — very hot)
///   +20 °C   → 1 800 m  (orange/yellow — warm)
///     0 °C   → 4 200 m  (green — temperate)
///   -20 °C   → 6 600 m  (cyan  — cold)
///   < -40 °C → ≥ 9 000 m (purple — very cold)
///
/// Formula: altitude_m = max(0, (35 − temp_c) × 120)
pub fn temp_to_altitude_m(temp_c: f64) -> f64 {
    ((35.0 - temp_c) * 120.0).max(0.0)
}
