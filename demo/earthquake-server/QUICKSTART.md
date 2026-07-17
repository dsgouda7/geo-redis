# Earthquake Tracker Demo — Quick Start Guide

This guide walks you through running the earthquake demo standalone or with the full geo-redis stack.

## Prerequisites

- .NET 8 SDK: https://dotnet.microsoft.com/download
- Node.js 24+: https://nodejs.org
- A running geo-redis geo-node (see below)

## Option 1: Standalone (Simplest)

### Step 1: Start a geo-redis geo-node

You need a geo-node running on port 3000 for the .NET client to connect to.

```powershell
# From repo root
cd demo\server
cargo run --release
```

This starts the OpenSky aircraft demo backend, which also provides the gRPC interface.

### Step 2: Start the .NET earthquake server

```powershell
# From repo root
cd demo\earthquake-server
dotnet restore
dotnet run
```

The server will:
- Connect to geo-redis geo-node at `http://localhost:3000`
- Fetch earthquakes from USGS every 5 minutes
- Insert them via gRPC `InsertBatch`
- Expose REST API at `http://localhost:3003`

### Step 3: Start the UI

```powershell
# From repo root
cd demo\ui
npm install  # first time only
npm run dev:earthquake
```

Open **http://localhost:5175** in your browser.

---

## Option 2: With Full Demo Stack

Run all demos at once (aircraft + weather + earthquake):

```powershell
# From repo root
.\scripts\run-demo.ps1
```

Then start the earthquake server separately:

```powershell
cd demo\earthquake-server
dotnet run
```

And the earthquake UI:

```powershell
cd demo\ui
npm run dev:earthquake
```

---

## Verify It's Working

### Check the .NET server logs

You should see:
```
[12:34:56 INF] Earthquake Server starting on http://localhost:3003
[12:34:57 INF] Connecting to GeoRedis at http://localhost:3000
[12:34:58 INF] Fetching earthquakes from USGS (past 24 hours)...
[12:34:59 INF] Fetched 523 earthquakes, 342 with magnitude >= 2.5
[12:35:00 INF] Inserted 342 earthquakes via gRPC in 87ms
```

### Test the API

```powershell
# Get all earthquakes
curl http://localhost:3003/api/earthquakes

# Get metrics
curl http://localhost:3003/api/metrics

# Check health
curl http://localhost:3003/health
```

### View the map

Navigate to **http://localhost:5175** and you should see:
- Colored circles for earthquakes (size = magnitude)
- Metrics panel on the right showing magnitude distribution
- Recent large quakes (M ≥ 5.0) listed
- Click any circle for popup with USGS details

---

## Troubleshooting

### "gRPC client not initialized"

The .NET server can't connect to the geo-redis geo-node. Make sure:
- A geo-node is running on `http://localhost:3000`
- Check `appsettings.json` has the correct `GeoRedis:Url`

### "No earthquakes fetched from USGS"

- Check your internet connection
- USGS API may be temporarily down (rare)
- Check logs for HTTP error details

### UI shows "Failed to load earthquake data"

- Make sure the .NET server is running on port 3003
- Check browser console for CORS errors
- Verify `http://localhost:3003/api/earthquakes` returns JSON

---

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

---

## Docker (Optional)

Build and run in a container:

```powershell
cd demo\earthquake-server
docker build -t earthquake-server .
docker run -p 3003:3003 -e GeoRedis__Url=http://host.docker.internal:3000 earthquake-server
```

Note: Use `host.docker.internal` to reach the geo-node on the host machine.

---

## Data Source

**USGS Earthquake Hazards Program**
- API: https://earthquake.usgs.gov/earthquakes/feed/v1.0/geojson.php
- Feed: Past 24 hours, all magnitudes
- Updates: Every 5 minutes
- No API key required
- No rate limits on public feeds

---

## What's Next?

- Explore the **Swagger UI** at `http://localhost:3003/swagger`
- Try the regional query: `/api/region?s=30&w=-120&n=50&e=-100`
- Check cluster topology: `/api/cluster`
- View the [main README](../README.md) for other demos
