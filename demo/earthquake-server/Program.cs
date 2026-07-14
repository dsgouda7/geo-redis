using SatelliteServer.Services;
using Georedis.V1;
using Grpc.Net.Client;
using Microsoft.AspNetCore.Mvc;

var builder = WebApplication.CreateBuilder(args);

// Add services to the container
builder.Services.AddHttpClient<N2YOClient>();

// Register SatellitePoller as singleton so we can access it from endpoints
builder.Services.AddSingleton<SatellitePoller>();
// Also register it as a hosted service (using the same instance)
builder.Services.AddHostedService(sp => sp.GetRequiredService<SatellitePoller>());

// Register TerminatorCalculator as singleton
builder.Services.AddSingleton<TerminatorCalculator>();
builder.Services.AddHostedService(sp => sp.GetRequiredService<TerminatorCalculator>());

builder.Services.AddCors(options =>
{
    options.AddDefaultPolicy(policy =>
    {
        policy.AllowAnyOrigin()
              .AllowAnyMethod()
              .AllowAnyHeader();
    });
});

// Add Swagger for API documentation
builder.Services.AddEndpointsApiExplorer();
builder.Services.AddSwaggerGen();

var app = builder.Build();

// Configure the HTTP request pipeline
if (app.Environment.IsDevelopment())
{
    app.UseSwagger();
    app.UseSwaggerUI();
}

app.UseCors();

// ── REST API Endpoints ─────────────────────────────────────────────────────

/// <summary>
/// Get all satellites from in-memory cache (fast, no gRPC call).
/// This returns the latest polled data.
/// </summary>
app.MapGet("/api/satellites", (SatellitePoller poller) =>
{
    var satellites = poller.GetLatestSatellites();
    return Results.Ok(new
    {
        count = satellites.Count,
        lastUpdate = poller.GetLastUpdateTime(),
        satellites
    });
})
.WithName("GetAllSatellites")
.WithOpenApi();

/// <summary>
/// Query satellites within a bounding box via gRPC.
/// This demonstrates the QueryRegion RPC.
/// </summary>
app.MapGet("/api/region", async (
    [FromQuery] double south,
    [FromQuery] double west,
    [FromQuery] double north,
    [FromQuery] double east,
    [FromServices] IConfiguration config) =>
{
    var geoRedisUrl = config.GetValue<string>("GeoRedis:Url") ?? "http://localhost:3000";
    
    using var channel = GrpcChannel.ForAddress(geoRedisUrl);
    var client = new GeoRedis.GeoRedisClient(channel);

    var request = new RegionRequest
    {
        South = south,
        West = west,
        North = north,
        East = east
    };

    var response = await client.QueryRegionAsync(request);

    return Results.Ok(new
    {
        count = response.Count,
        shardsQueried = response.ShardsQueried,
        satellites = response.Entries.Select(e => new
        {
            id = e.Id,
            lat = e.Lat,
            lon = e.Lon,
            payload = System.Text.Json.JsonDocument.Parse(e.PayloadJson)
        })
    });
})
.WithName("QueryRegion")
.WithOpenApi();

/// <summary>
/// Get detailed information for a specific satellite by ID via gRPC.
/// </summary>
app.MapGet("/api/satellite/{id}", async (
    string id,
    [FromServices] IConfiguration config) =>
{
    var geoRedisUrl = config.GetValue<string>("GeoRedis:Url") ?? "http://localhost:3000";
    
    using var channel = GrpcChannel.ForAddress(geoRedisUrl);
    var client = new GeoRedis.GeoRedisClient(channel);

    var request = new DetailRequest { Id = id };
    var response = await client.GetDetailAsync(request);

    if (!response.Found)
    {
        return Results.NotFound(new { error = "Satellite not found" });
    }

    return Results.Ok(new
    {
        id = response.Id,
        payload = System.Text.Json.JsonDocument.Parse(response.PayloadJson),
        history = response.History.Select(h => new { lat = h.Lat, lon = h.Lon })
    });
})
.WithName("GetSatelliteDetail")
.WithOpenApi();

/// <summary>
/// Get metrics: satellite count by category, latest update time, etc.
/// </summary>
app.MapGet("/api/metrics", (SatellitePoller poller) =>
{
    var satellites = poller.GetLatestSatellites();
    
    var metrics = new
    {
        totalCount = satellites.Count,
        lastUpdate = poller.GetLastUpdateTime(),
        categories = satellites.GroupBy(s => s.Category)
            .Select(g => new { category = g.Key, count = g.Count() })
            .OrderByDescending(x => x.count),
        altitudeRanges = new
        {
            leo = satellites.Count(s => s.Altitude < 2000),       // Low Earth Orbit
            meo = satellites.Count(s => s.Altitude >= 2000 && s.Altitude < 35786), // Medium
            geo = satellites.Count(s => s.Altitude >= 35786)      // Geostationary
        },
        topSatellites = satellites
            .OrderBy(s => s.Name)
            .Take(10)
            .Select(s => new
            {
                s.Id,
                s.Name,
                s.Altitude,
                s.Category,
                s.LaunchDate
            })
    };

    return Results.Ok(metrics);
})
.WithName("GetMetrics")
.WithOpenApi();

/// <summary>
/// Get the day/night terminator line coordinates.
/// This boundary updates every minute based on Earth's rotation.
/// </summary>
app.MapGet("/api/terminator", (TerminatorCalculator calculator) =>
{
    var coords = calculator.GetTerminatorCoordinates();
    
    return Results.Ok(new
    {
        lastUpdate = calculator.GetLastUpdateTime(),
        pointCount = coords.Count,
        nightSide = calculator.GetNightSide(),
        coordinates = coords.Select(c => new { lat = c.Lat, lon = c.Lon })
    });
})
.WithName("GetTerminator")
.WithOpenApi();

/// <summary>
/// Get cluster topology from proxima via gRPC.
/// Shows which shards are active and their key distribution.
/// </summary>
app.MapGet("/api/cluster", async ([FromServices] IConfiguration config) =>
{
    var geoRedisUrl = config.GetValue<string>("GeoRedis:Url") ?? "http://localhost:3000";
    
    using var channel = GrpcChannel.ForAddress(geoRedisUrl);
    var client = new GeoRedis.GeoRedisClient(channel);

    var response = await client.GetClusterAsync(new Empty());

    return Results.Ok(new
    {
        nodes = response.Nodes.Select(n => new
        {
            nodeId = n.NodeId,
            addr = n.Addr,
            prefixRange = $"{n.PrefixStart}-{n.PrefixEnd}",
            keyCount = n.KeyCount,
            memBytes = n.MemBytes,
            status = n.Status,
            generation = n.Generation
        })
    });
})
.WithName("GetCluster")
.WithOpenApi();

/// <summary>
/// Health check endpoint.
/// </summary>
app.MapGet("/health", (SatellitePoller poller) =>
{
    var lastUpdate = poller.GetLastUpdateTime();
    var staleness = DateTime.UtcNow - lastUpdate;
    
    return Results.Ok(new
    {
        status = staleness < TimeSpan.FromMinutes(1) ? "healthy" : "stale",
        lastUpdate,
        stalenessSeconds = staleness.TotalSeconds,
        satelliteCount = poller.GetLatestSatellites().Count
    });
})
.WithName("Health")
.WithOpenApi();

// ───────────────────────────────────────────────────────────────────────────

var port = builder.Configuration.GetValue<int>("Server:Port", 3003);
var host = builder.Configuration.GetValue<string>("Server:Host", "localhost");

app.Logger.LogInformation("Satellite Tracker starting on http://{Host}:{Port}", host, port);
app.Logger.LogInformation("Swagger UI available at http://{Host}:{Port}/swagger", host, port);

app.Run($"http://{host}:{port}");
