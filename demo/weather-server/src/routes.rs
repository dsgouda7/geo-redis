use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use s2::{cap::Cap, latlng::LatLng, point::Point, region::RegionCoverer, s1};
use georedis::GeoEntry;
use crate::{metar, AppState};

// ── Response types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AircraftResponse {
    count:    usize,
    aircraft: Vec<GeoEntry>,
}

#[derive(Deserialize)]
pub struct RegionParams {
    s: f64, w: f64, n: f64, e: f64,
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Maps a weather station GeoEntry to the aircraft-compatible schema
/// the Leaflet UI expects, so no UI changes are needed.
///
/// Colour scale (driven by altitude field):
///   Hot  (+35°C+) → red/orange   (ground level)
///   Warm (+20°C)  → yellow
///   Cool (  0°C)  → green
///   Cold (-20°C)  → cyan/blue
///   Very cold (-40°C+) → purple  (high altitude)
fn station_to_aircraft(mut e: GeoEntry) -> GeoEntry {
    let icao      = e.payload["icao_id"].as_str().unwrap_or(&e.id).to_string();
    let name      = e.payload["name"].as_str().unwrap_or("").to_string();
    let temp_c    = e.payload["temp_c"].as_f64();
    let wspd      = e.payload["wspd_kt"].as_f64();
    let wdir      = e.payload["wdir"].as_u64().map(|d| d as f64);
    let wx        = e.payload["wx_string"].as_str().unwrap_or("").to_string();
    let clouds    = e.payload["clouds"].as_str().unwrap_or("").to_string();
    let flt_cat   = e.payload["flt_cat"].as_str().unwrap_or("").to_string();

    // Condition label shown in origin_country field: wx code if present, else cloud cover + flight category
    let condition = if !wx.is_empty() {
        format!("{wx}  {flt_cat}")
    } else if !clouds.is_empty() {
        format!("{clouds}  {flt_cat}")
    } else {
        flt_cat.clone()
    };

    // Altitude encodes temperature for colour display
    let altitude = temp_c.map(metar::temp_to_altitude_m);

    // Callsign = station name (readable) or ICAO if name is empty
    let callsign = if !name.is_empty() { name } else { icao.clone() };

    e.payload = serde_json::json!({
        "callsign":       callsign,
        "altitude":       altitude,
        "velocity":       wspd,
        "heading":        wdir,
        "on_ground":      false,
        "origin_country": condition,
        "temp_c":         temp_c,
        "icao_id":        icao,
    });
    e
}

// ── Route handlers ─────────────────────────────────────────────────────────

pub async fn all_stations(State(st): State<Arc<AppState>>) -> Json<AircraftResponse> {
    let entries = st.trie.read().await.all_entries()
        .into_iter().map(station_to_aircraft).collect::<Vec<_>>();
    Json(AircraftResponse { count: entries.len(), aircraft: entries })
}

pub async fn region_stations(
    State(st): State<Arc<AppState>>,
    Query(p):  Query<RegionParams>,
) -> Json<AircraftResponse> {
    let tokens = viewport_tokens(p.s, p.w, p.n, p.e, st.config.s2_level);
    let entries = match st.store.query_region(&tokens).await {
        Ok(v)  => v.into_iter().map(station_to_aircraft).collect(),
        Err(e) => { tracing::error!("region query: {e}"); vec![] }
    };
    Json(AircraftResponse { count: entries.len(), aircraft: entries })
}

pub async fn station_detail(
    State(st): State<Arc<AppState>>,
    Path(id):  Path<String>,
) -> Json<serde_json::Value> {
    match st.db.get_detail(&id).await {
        Ok(Some(d)) => Json(serde_json::json!({
            "id":             d.id,
            "callsign":       if d.name.is_empty() { &d.id } else { &d.name },
            "origin_country": format!("{} {} {}", d.wx_string, d.clouds, d.flt_cat).trim().to_string(),
            "altitude":       d.temp_c.map(metar::temp_to_altitude_m),
            "velocity":       d.wspd_kt,
            "heading":        d.wdir,
            "on_ground":      false,
            "history":        d.history,
            // extra weather fields for completeness
            "temp_c":         d.temp_c,
            "dewp_c":         d.dewp_c,
        })),
        Ok(None) => Json(serde_json::json!({ "error": "not found" })),
        Err(e)   => {
            tracing::error!("detail query: {e}");
            Json(serde_json::json!({ "error": "internal error" }))
        }
    }
}

pub async fn get_metrics(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let snapshot  = st.store.metrics().snapshot();
    let trie_size = st.trie.read().await.len();
    let last_sync = *st.last_sync.read().await;
    Json(serde_json::json!({
        "source":    "aviationweather.gov (METAR)",
        "metrics":   snapshot,
        "trie_size": trie_size,
        "last_sync": last_sync,
    }))
}

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "source": "METAR" }))
}

// ── S2 viewport helper ─────────────────────────────────────────────────────

fn viewport_tokens(south: f64, west: f64, north: f64, east: f64, level: u8) -> Vec<String> {
    use std::f64::consts::PI;
    let center_lat = (south + north) / 2.0;
    let center_lon = (west  + east)  / 2.0;
    let d_lat = (north - south).abs() / 2.0;
    let d_lon = (east  - west).abs()  / 2.0;
    let radius_rad = ((d_lat * d_lat + d_lon * d_lon).sqrt() * PI / 180.0).min(PI);
    let center    = Point::from(LatLng::new(s1::Deg(center_lat).into(), s1::Deg(center_lon).into()));
    let cap_angle: s1::angle::Angle = s1::Rad(radius_rad).into();
    let cap       = Cap::from_center_angle(&center, &cap_angle);
    let coverer   = RegionCoverer { min_level: level, max_level: level, level_mod: 1, max_cells: 500 };
    coverer.covering(&cap).0.iter()
        .map(|c| { let h = format!("{:016x}", c.0); h.trim_end_matches('0').to_string() })
        .collect()
}
