mod config;
mod db;
mod metar_bulk;
mod open_meteo;
mod routes;

use std::collections::HashMap;
use std::sync::Arc;
use axum::{routing::get, Router};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use georedis::{GeoEntry, GeoTrie, Metrics, RedisStore};
use config::Config;

/// Payload for each SSE event dispatched during a streaming cycle.
#[derive(Clone, serde::Serialize)]
pub struct StationEvent {
    pub n:         usize,
    pub total:     usize,
    pub id:        String,
    pub lat:       f64,
    pub lon:       f64,
    pub temp_c:    f64,
    pub condition: String,
    pub wmo_code:  u8,
    pub complete:  bool,
}

pub struct AppState {
    pub trie:      RwLock<GeoTrie>,
    pub store:     RedisStore,
    pub config:    Config,
    pub last_sync: RwLock<Option<u64>>,
    pub db:        Arc<db::Db>,
    pub updates:   broadcast::Sender<StationEvent>,
    pub positions: RwLock<HashMap<String, (f64, f64)>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg      = Config::from_env();
    let metrics  = Metrics::new();
    let store    = RedisStore::with_config(&cfg.redis_url, Arc::clone(&metrics), cfg.entity_ttl_secs)?;
    let database = Arc::new(db::Db::open(&cfg.sqlite_path)?);
    let (tx, _)  = broadcast::channel::<StationEvent>(2048);

    tracing::info!("Source: aviationweather.gov bulk METAR dump (updated every 5 min)");
    tracing::info!("Redis:  {}", cfg.redis_url);
    tracing::info!("SQLite: {}", cfg.sqlite_path);
    tracing::info!("S2 level: {}, poll: {}s, stream rate: {}ms/event",
        cfg.s2_level, cfg.poll_interval_secs, cfg.stream_rate_ms);

    let state = Arc::new(AppState {
        trie:      RwLock::new(GeoTrie::new(cfg.s2_level)),
        store,
        config:    cfg.clone(),
        last_sync: RwLock::new(None),
        db:        database,
        updates:   tx,
        positions: RwLock::new(HashMap::new()),
    });

    let poll_state = Arc::clone(&state);
    tokio::spawn(async move {
        let http = reqwest::Client::new();
        loop {
            match metar_bulk::download_and_parse(&http).await {
                Ok(stations) if !stations.is_empty() => {
                    let total = stations.len();

                    let db_data: Vec<db::StationData> = stations.iter().map(|s| db::StationData {
                        id:        s.icao_id.clone(),
                        lat:       s.lat,
                        lon:       s.lon,
                        name:      s.icao_id.clone(),
                        temp_c:    s.temp_c,
                        dewp_c:    s.dewp_c,
                        wdir:      s.wind_dir,
                        wspd_kt:   s.wind_spd,
                        wx_string: format!(
                            "{} {}",
                            open_meteo::wmo_emoji(metar_bulk::wx_to_wmo(&s.wx, &s.sky)),
                            if s.wx.is_empty() { s.sky.clone() } else { s.wx.clone() }
                        ),
                        clouds:    s.sky.clone(),
                        flt_cat:   s.flt_cat.clone(),
                    }).collect();
                    if let Err(e) = poll_state.db.upsert_batch(db_data).await {
                        tracing::error!("SQLite upsert: {e}");
                    }

                    tracing::info!("Streaming {total} METAR events into trie…");
                    for (n, s) in stations.iter().enumerate() {
                        let wmo  = metar_bulk::wx_to_wmo(&s.wx, &s.sky);
                        let temp = s.temp_c.unwrap_or(0.0);

                        let entry = GeoEntry {
                            id:  s.icao_id.clone(),
                            lat: s.lat,
                            lon: s.lon,
                            payload: serde_json::json!({
                                "name":         s.icao_id,
                                "temp_c":       s.temp_c,
                                "feels_like_c": null,
                                "humidity_pct": null,
                                "wspd_kt":      s.wind_spd,
                                "gust_kt":      s.wind_gst,
                                "wdir":         s.wind_dir,
                                "wmo_code":     wmo,
                                "precip":       null,
                                "cloud_pct":    null,
                                "pressure_hpa": null,
                                "flt_cat":      s.flt_cat,
                                "__is_weather": true,
                            }),
                        };

                        {
                            let mut trie  = poll_state.trie.write().await;
                            let mut pos   = poll_state.positions.write().await;
                            if let Some(&(old_lat, old_lon)) = pos.get(&s.icao_id) {
                                trie.remove_entry(old_lat, old_lon, &s.icao_id);
                            }
                            trie.insert(entry);
                            pos.insert(s.icao_id.clone(), (s.lat, s.lon));
                        }

                        if n % 200 == 199 {
                            let trie = poll_state.trie.read().await;
                            let _ = poll_state.store.persist_trie(&trie).await;
                        }

                        let _ = poll_state.updates.send(StationEvent {
                            n, total,
                            complete:  n == total - 1,
                            id:        s.icao_id.clone(),
                            lat:       s.lat,
                            lon:       s.lon,
                            temp_c:    temp,
                            condition: if s.wx.is_empty() { s.sky.clone() } else { s.wx.clone() },
                            wmo_code:  wmo,
                        });

                        if poll_state.config.stream_rate_ms > 0 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                poll_state.config.stream_rate_ms,
                            )).await;
                        }
                    }

                    {
                        let trie = poll_state.trie.read().await;
                        let _ = poll_state.store.persist_trie(&trie).await;
                    }
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                    *poll_state.last_sync.write().await = Some(ts);
                    tracing::info!("Streaming complete — {total} stations live");
                }
                Ok(_) => tracing::warn!("Bulk METAR returned 0 stations"),
                Err(e) => tracing::error!("Bulk METAR failed: {e}"),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(
                poll_state.config.poll_interval_secs,
            )).await;
        }
    });

    let app = Router::new()
        .route("/api/aircraft",     get(routes::all_stations))
        .route("/api/aircraft/:id", get(routes::station_detail))
        .route("/api/region",       get(routes::region_stations))
        .route("/api/metrics",      get(routes::get_metrics))
        .route("/api/stream",       get(routes::sse_stream))
        .route("/health",           get(routes::health))
        .layer(CorsLayer::permissive())
        .with_state(Arc::clone(&state));

    let addr = format!("{}:{}", state.config.server_host, state.config.server_port);
    tracing::info!("Weather demo listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
