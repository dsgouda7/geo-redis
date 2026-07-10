use serde::Deserialize;

/// One current weather observation from Open-Meteo.
#[derive(Debug, Clone)]
pub struct WeatherObs {
    /// Synthetic ID: "lat_lon" (e.g. "48.0_2.0")
    pub id:        String,
    /// Human-readable label, e.g. "48°N 2°E"
    pub name:      String,
    pub lat:       f64,
    pub lon:       f64,
    /// Temperature in °C
    pub temp_c:    f64,
    /// Wind speed in knots
    pub wspd_kt:   f64,
    /// Wind direction degrees (0–360)
    pub wdir:      u16,
    /// WMO weather interpretation code
    pub wmo_code:  u8,
    /// Precipitation mm/h
    pub precip:    f64,
}

// ── WMO codes ─────────────────────────────────────────────────────────────

pub fn wmo_emoji(code: u8) -> &'static str {
    match code {
        0            => "☀️",
        1            => "🌤️",
        2            => "⛅",
        3            => "☁️",
        45 | 48      => "🌫️",
        51..=57      => "🌦️",
        61..=65      => "🌧️",
        66 | 67      => "🌨️",
        71..=77      => "❄️",
        80..=82      => "🌦️",
        85 | 86      => "🌨️",
        95           => "⛈️",
        96 | 99      => "⛈️",
        _            => "🌡️",
    }
}

pub fn wmo_label(code: u8) -> &'static str {
    match code {
        0        => "Clear sky",
        1        => "Mainly clear",
        2        => "Partly cloudy",
        3        => "Overcast",
        45       => "Fog",
        48       => "Icy fog",
        51       => "Light drizzle",
        53       => "Moderate drizzle",
        55       => "Heavy drizzle",
        61       => "Light rain",
        63       => "Moderate rain",
        65       => "Heavy rain",
        66 | 67  => "Freezing rain",
        71       => "Light snow",
        73       => "Moderate snow",
        75       => "Heavy snow",
        77       => "Snow grains",
        80       => "Light showers",
        81       => "Moderate showers",
        82       => "Heavy showers",
        85 | 86  => "Snow showers",
        95       => "Thunderstorm",
        96 | 99  => "Thunderstorm + hail",
        _        => "Unknown",
    }
}

/// Map temperature (°C) to an "altitude" value (metres) for the existing
/// Leaflet colour scale — cold = purple/blue, hot = red/orange:
///
///   +35°C →      0 m  (red)
///   +20°C →  1 800 m  (yellow)
///     0°C →  4 200 m  (green)
///   -20°C →  6 600 m  (cyan)
///  -40°C+ → ≥9 000 m  (purple)
pub fn temp_to_altitude_m(temp_c: f64) -> f64 {
    ((35.0 - temp_c) * 120.0).max(0.0)
}

// ── Open-Meteo API shapes ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct OmResponse {
    latitude:  f64,
    longitude: f64,
    current:   CurrentWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    temperature_2m:      f64,
    wind_speed_10m:      f64,
    wind_direction_10m:  f64,
    weather_code:        f64,  // Open-Meteo returns this as float
    precipitation:       f64,
}

// ── Global 10° grid ────────────────────────────────────────────────────────

fn global_grid() -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    let mut lat = -80.0_f64;
    while lat <= 80.01 {
        let mut lon = -170.0_f64;
        while lon <= 170.01 {
            pts.push((lat, lon));
            lon += 10.0;
        }
        lat += 10.0;
    }
    pts
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Fetches current weather for a global 10° grid (~595 points) from Open-Meteo.
/// No API key required.  Sends two batched requests (~300 points each) to
/// stay well within URL length limits.
pub async fn fetch_stations(client: &reqwest::Client) -> anyhow::Result<Vec<WeatherObs>> {
    let grid = global_grid();
    let mut all: Vec<WeatherObs> = Vec::with_capacity(grid.len());

    for chunk in grid.chunks(300) {
        let lats: Vec<String> = chunk.iter().map(|(la, _)| format!("{la}")).collect();
        let lons: Vec<String> = chunk.iter().map(|(_, lo)| format!("{lo}")).collect();

        let body = client
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude",  lats.join(",")),
                ("longitude", lons.join(",")),
                ("current",   "temperature_2m,wind_speed_10m,wind_direction_10m,weather_code,precipitation".to_string()),
                ("wind_speed_unit", "kn".to_string()),
            ])
            .timeout(std::time::Duration::from_secs(30))
            .header("User-Agent", "georedis-weather-demo/1.0")
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        // Open-Meteo returns an array for multiple points, or an error object.
        let records: Vec<OmResponse> = match body {
            serde_json::Value::Array(arr) => {
                serde_json::from_value(serde_json::Value::Array(arr))
                    .unwrap_or_default()
            }
            serde_json::Value::Object(ref obj) if obj.contains_key("error") => {
                anyhow::bail!("Open-Meteo API error: {}", body["reason"].as_str().unwrap_or("unknown"));
            }
            single => match serde_json::from_value::<OmResponse>(single) {
                Ok(r) => vec![r],
                Err(_) => vec![],
            },
        };

        for r in records {
            let lat = (r.latitude * 10.0).round() / 10.0;
            let lon = (r.longitude * 10.0).round() / 10.0;
            let wmo = r.current.weather_code as u8;
            let lat_str = if lat >= 0.0 { format!("{lat}°N") } else { format!("{}°S", lat.abs()) };
            let lon_str = if lon >= 0.0 { format!("{lon}°E") } else { format!("{}°W", lon.abs()) };
            all.push(WeatherObs {
                id:       format!("{lat}_{lon}"),
                name:     format!("{lat_str} {lon_str}"),
                lat, lon,
                temp_c:  r.current.temperature_2m,
                wspd_kt: r.current.wind_speed_10m,
                wdir:    r.current.wind_direction_10m as u16,
                wmo_code: wmo,
                precip:  r.current.precipitation,
            });
        }
    }

    Ok(all)
}
