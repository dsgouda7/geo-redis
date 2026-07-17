# Earthquake Real-Time Tracker — .NET + gRPC + USGS

A cross-platform .NET 8 demo showcasing **geo-redis's gRPC API** with real-time earthquake data from the [USGS Earthquake Hazards Program](https://earthquake.usgs.gov/).

## Features

- **gRPC batch insert** — Polls USGS every 5 minutes and inserts 100-500 earthquakes via `InsertBatch`
- **Regional queries** — Demonstrates `QueryRegion` for viewport-based filtering
- **Cluster topology** — Uses `GetCluster` to show shard distribution
- **Cross-platform** — Runs on Windows, Linux, macOS (no platform-specific dependencies)
- **No API key required** — USGS provides free, public GeoJSON feeds

## Quick Start

### Prerequisites

- .NET 8 SDK or later
- Running geo-redis geo-node (see main demo instructions)

### Run the Server

```powershell
# Windows
cd demo\earthquake-server
dotnet restore
dotnet run
```

```bash
# Linux / macOS
cd demo/earthquake-server
dotnet restore
dotnet run
```

The server starts on `http://localhost:3003` and immediately begins polling USGS.

### API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /api/earthquakes` | All recent earthquakes (from in-memory cache) |
| `GET /api/region?s=30&w=-120&n=50&e=-100` | Query earthquakes in bounding box (via gRPC) |
| `GET /api/earthquake/{id}` | Get detail for specific earthquake (via gRPC) |
| `GET /api/metrics` | Earthquake counts by magnitude range |
| `GET /api/cluster` | geo-redis cluster topology (via gRPC) |
| `GET /health` | Health check (staleness indicator) |
| `GET /swagger` | Interactive API documentation |

### Example Request

```bash
# Get all earthquakes (fast, cached)
curl http://localhost:3003/api/earthquakes

# Query California region (gRPC call)
curl "http://localhost:3003/api/region?s=32&w=-125&n=42&e=-114"

# Get metrics
curl http://localhost:3003/api/metrics
```

## Data Source

**USGS Earthquake Hazards Program** provides free, real-time earthquake data:

- **Feed**: Past 24 hours, all magnitudes ≥ 2.5
- **Update frequency**: Every 5 minutes
- **Format**: GeoJSON
- **Coverage**: Worldwide
- **No rate limits** on public feeds

The demo filters to magnitude ≥ 2.5 to keep the map focused on notable seismic activity.

## Configuration

Edit `appsettings.json`:

```json
{
  "GeoRedis": {
    "Url": "http://localhost:3000"
  },
  "Polling": {
    "IntervalMinutes": 5
  },
  "Server": {
    "Host": "localhost",
    "Port": 3003
  }
}
```

## gRPC Integration

This demo uses the **generated .NET client** from `georedis.proto`:

```csharp
using var channel = GrpcChannel.ForAddress("http://localhost:3000");
var client = new GeoRedis.GeoRedisClient(channel);

// Batch insert
var batch = new InsertBatchRequest();
batch.Entries.Add(new GeoEntry {
    Id = earthquake.Id,
    Lat = earthquake.Latitude,
    Lon = earthquake.Longitude,
    PayloadJson = JsonSerializer.Serialize(metadata)
});
var response = await client.InsertBatchAsync(batch);

// Query region
var region = new RegionRequest {
    South = 30, West = -120,
    North = 50, East = -100
};
var results = await client.QueryRegionAsync(region);
```

## Architecture

```
┌─────────────────────┐
│   USGS GeoJSON API  │  (updates every 5 min)
└──────────┬──────────┘
           │ HTTP GET
           ▼
┌─────────────────────┐
│   UsgsClient.cs     │  (fetch + parse)
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ EarthquakePoller.cs │  (background service, timer-based)
└──────────┬──────────┘
           │ gRPC InsertBatch
           ▼
┌─────────────────────┐
│  geo-redis geo-node  │  (S2 spatial trie + Redis)
└──────────┬──────────┘
           │ gRPC QueryRegion
           ▼
┌─────────────────────┐
│    Program.cs       │  (REST API for UI)
└─────────────────────┘
```

## Next Steps

- **UI Integration**: Add earthquake map to `demo/ui/src/` (see UI setup instructions)
- **Docker**: Build with `docker build -t earthquake-server .`
- **Kubernetes**: Deploy alongside geo-node cluster in `demo/k8s/`

## Why Earthquakes?

- **Real-time movement**: Events occur globally every minute (100-150 per day ≥ M4.0)
- **Geographic distribution**: Follows tectonic plate boundaries (Pacific Ring of Fire, etc.)
- **Magnitude scale**: Provides visual hierarchy (size/color by magnitude)
- **Public interest**: Everyone understands earthquakes; no domain expertise required
- **Free data**: USGS provides reliable, updated feeds with no API key

---

**Polling frequency**: 5 minutes matches USGS update cycle. Faster polling won't yield new data and would waste bandwidth. Slower polling (10-15 min) is fine for a demo but reduces "real-time" feel.
