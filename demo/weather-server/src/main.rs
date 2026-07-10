mod config;
mod db;
mod open_meteo;
mod routes;

use std::sync::Arc;
use axum::{routing::get, Router};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use georedis::{GeoEntry, GeoTrie, Metrics, RedisStore};
use config::Config;

pub struct AppState {
    pub trie:      RwLock<GeoTrie>,
    pub store:     RedisStore,
    pub config:    Config,
    pub last_sync: RwLock<Option<u64>>,
    pub db:        Arc<db::Db>,
    /// Broadcast channel — fires after every successful poll.
    /// SSE clients subscribe to be notified when fresh data is available.
    pub updates:   broadcast::Sender<()>,
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
    let (tx, _)  = broadcast::channel::<()>(16);

    tracing::info!("Source: Open-Meteo (global 10° grid — no API key required)");
    tracing::info!("Redis:  {}", cfg.redis_url);
    tracing::info!("SQLite: {}", cfg.sqlite_path);
    tracing::info!("S2 level: {}, poll interval: {}s", cfg.s2_level, cfg.poll_interval_secs);

    let state = Arc::new(AppState {
        trie:    RwLock::new(GeoTrie::new(cfg.s2_level)),
        store,
        config:  cfg.clone(),
        last_sync: RwLock::new(None),
        db:      database,
        updates: tx,
    });

    // ── Background poller ─────────────────────────────────────────────────
    let poll_state = Arc::clone(&state);
    tokio::spawn(async move {
        let http = reqwest::Client::new();
        let mut poll_count: u32 = 0;
        loop {
            tracing::info!("Fetching Open-Meteo global weather grid…");
            match open_meteo::fetch_stations(&http).await {
                Ok(stations) => {
                    let n = stations.len();

                    // 1. Persist to SQLite
                    let db_data: Vec<db::StationData> = stations.iter().map(|s| db::StationData {
                        id:        s.id.clone(),
                        lat:       s.lat,
                        lon:       s.lon,
                        name:      s.name.clone(),
                        temp_c:    Some(s.temp_c),
                        dewp_c:    None,
                        wdir:      Some(s.wdir),
                        wspd_kt:   Some(s.wspd_kt),
                        wx_string: format!("{} {}", open_meteo::wmo_emoji(s.wmo_code), open_meteo::wmo_label(s.wmo_code)),
                        clouds:    String::new(),
                        flt_cat:   String::new(),
                    }).collect();
                    if let Err(e) = poll_state.db.upsert_batch(db_data).await {
                        tracing::error!("SQLite upsert failed: {e}");
                    }

                    // 2. Rebuild trie
                    {
                        let mut trie = poll_state.trie.write().await;
                        trie.clear();
                        for s in &stations {
                            trie.insert(GeoEntry {
                                id:  s.id.clone(),
                                lat: s.lat,
                                lon: s.lon,
                                payload: serde_json::json!({
                                    "name":      s.name,
                                    "temp_c":    s.temp_c,
                                    "wspd_kt":   s.wspd_kt,
                                    "wdir":      s.wdir,
                                    "wmo_code":  s.wmo_code,
                                    "precip":    s.precip,
                                    "__is_weather": true,
                                }),
                            });
                        }
                    }

                    // 3. Persist trie to Redis
                    {
                        let trie = poll_state.trie.read().await;
                        if let Err(e) = poll_state.store.persist_trie(&trie).await {
                            tracing::error!("Redis persist failed: {e}");
                        }
                    }

                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                    *poll_state.last_sync.write().await = Some(ts);
                    tracing::info!("Synced {n} weather grid points to trie + Redis");

                    // 4. Notify all SSE clients that fresh data is ready
                    let _ = poll_state.updates.send(());

                    poll_count += 1;
                    if poll_count % 6 == 0 {
                        if let Err(e) = poll_state.db.prune_history().await {
                            tracing::warn!("History prune failed: {e}");
                        }
                    }
                }
                Err(e) => tracing::error!("Open-Meteo fetch failed: {e}"),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(
                poll_state.config.poll_interval_secs,
            )).await;
        }
    });

    // ── HTTP server ───────────────────────────────────────────────────────
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
