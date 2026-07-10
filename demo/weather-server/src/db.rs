use std::sync::{Arc, Mutex};
use rusqlite::{params, Connection};
use serde::Serialize;

const SCHEMA: &str = r#"
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;

CREATE TABLE IF NOT EXISTS stations (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL DEFAULT '',
    temp_c      REAL,
    dewp_c      REAL,
    wdir        INTEGER,
    wspd_kt     REAL,
    wx_string   TEXT NOT NULL DEFAULT '',
    clouds      TEXT NOT NULL DEFAULT '',
    flt_cat     TEXT NOT NULL DEFAULT '',
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS obs_history (
    rowid      INTEGER PRIMARY KEY AUTOINCREMENT,
    station_id TEXT NOT NULL,
    lat        REAL NOT NULL,
    lon        REAL NOT NULL,
    temp_c     REAL,
    recorded_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_obs_station_time
ON obs_history(station_id, recorded_at DESC);
"#;

pub struct StationData {
    pub id:        String,
    pub lat:       f64,
    pub lon:       f64,
    pub name:      String,
    pub temp_c:    Option<f64>,
    pub dewp_c:    Option<f64>,
    pub wdir:      Option<u16>,
    pub wspd_kt:   Option<f64>,
    pub wx_string: String,
    pub clouds:    String,
    pub flt_cat:   String,
}

#[derive(Serialize)]
pub struct StationDetail {
    pub id:        String,
    pub name:      String,
    pub temp_c:    Option<f64>,
    pub dewp_c:    Option<f64>,
    pub wdir:      Option<u16>,
    pub wspd_kt:   Option<f64>,
    pub wx_string: String,
    pub clouds:    String,
    pub flt_cat:   String,
    /// Last 3 observations [lat, lon], oldest first (always same point for fixed stations).
    pub history:   Vec<[f64; 2]>,
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        tracing::info!("SQLite opened at {path}");
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub async fn upsert_batch(&self, stations: Vec<StationData>) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut guard = conn.lock().map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

            let tx = guard.transaction()?;
            {
                let mut upsert = tx.prepare_cached(
                    "INSERT INTO stations
                         (id,name,temp_c,dewp_c,wdir,wspd_kt,wx_string,clouds,flt_cat,updated_at)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                     ON CONFLICT(id) DO UPDATE SET
                         name=excluded.name, temp_c=excluded.temp_c,
                         dewp_c=excluded.dewp_c, wdir=excluded.wdir,
                         wspd_kt=excluded.wspd_kt, wx_string=excluded.wx_string,
                         clouds=excluded.clouds, flt_cat=excluded.flt_cat,
                         updated_at=excluded.updated_at",
                )?;
                let mut hist = tx.prepare_cached(
                    "INSERT INTO obs_history(station_id,lat,lon,temp_c,recorded_at) VALUES(?1,?2,?3,?4,?5)",
                )?;
                for s in &stations {
                    upsert.execute(params![
                        s.id, s.name, s.temp_c, s.dewp_c,
                        s.wdir.map(|d| d as i32),
                        s.wspd_kt, s.wx_string, s.clouds, s.flt_cat, now
                    ])?;
                    hist.execute(params![s.id, s.lat, s.lon, s.temp_c, now])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await?
    }

    pub async fn prune_history(&self) -> anyhow::Result<()> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let guard = conn.lock().map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
            let cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64 - 3600;
            guard.execute("DELETE FROM obs_history WHERE recorded_at < ?1", params![cutoff])?;
            Ok(())
        })
        .await?
    }

    pub async fn get_detail(&self, id: &str) -> anyhow::Result<Option<StationDetail>> {
        let conn = Arc::clone(&self.conn);
        let id   = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<Option<StationDetail>> {
            let guard = conn.lock().map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;

            let result = guard.query_row(
                "SELECT id,name,temp_c,dewp_c,wdir,wspd_kt,wx_string,clouds,flt_cat
                 FROM stations WHERE id=?1",
                params![id],
                |row| Ok(StationDetail {
                    id:        row.get(0)?,
                    name:      row.get::<_,Option<String>>(1)?.unwrap_or_default(),
                    temp_c:    row.get(2)?,
                    dewp_c:    row.get(3)?,
                    wdir:      row.get::<_,Option<i32>>(4)?.map(|d| d as u16),
                    wspd_kt:   row.get(5)?,
                    wx_string: row.get::<_,Option<String>>(6)?.unwrap_or_default(),
                    clouds:    row.get::<_,Option<String>>(7)?.unwrap_or_default(),
                    flt_cat:   row.get::<_,Option<String>>(8)?.unwrap_or_default(),
                    history:   vec![],
                }),
            );

            let mut detail = match result {
                Ok(d)                                     => d,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e)                                    => return Err(e.into()),
            };

            let mut stmt = guard.prepare(
                "SELECT lat,lon FROM obs_history
                 WHERE station_id=?1 ORDER BY recorded_at ASC LIMIT 3",
            )?;
            detail.history = stmt
                .query_map(params![detail.id], |r| Ok([r.get::<_,f64>(0)?, r.get::<_,f64>(1)?]))?
                .collect::<rusqlite::Result<_>>()?;

            Ok(Some(detail))
        })
        .await?
    }
}
