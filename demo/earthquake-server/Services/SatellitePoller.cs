using System.Text.Json;
using SatelliteServer.Models;
using Georedis.V1;
using Grpc.Net.Client;

namespace SatelliteServer.Services;

/// <summary>
/// Background service that fetches satellite positions and inserts into proxima via gRPC.
/// </summary>
public class SatellitePoller : BackgroundService
{
    private readonly N2YOClient _n2yoClient;
    private readonly IConfiguration _config;
    private readonly ILogger<SatellitePoller> _logger;
    private readonly TimeSpan _pollInterval;
    private GrpcChannel? _channel;
    private GeoRedis.GeoRedisClient? _grpcClient;

    // In-memory cache of latest satellites for the API endpoints
    private List<SatelliteDto> _latestSatellites = new();
    private readonly object _lock = new();
    private DateTime _lastUpdate = DateTime.MinValue;

    public SatellitePoller(
        N2YOClient n2yoClient,
        IConfiguration config,
        ILogger<SatellitePoller> logger)
    {
        _n2yoClient = n2yoClient;
        _config = config;
        _logger = logger;
        
        // Poll every 10 seconds for satellite positions (they move fast!)
        var intervalSeconds = config.GetValue<int>("Polling:IntervalSeconds", 10);
        _pollInterval = TimeSpan.FromSeconds(intervalSeconds);
    }

    public override async Task StartAsync(CancellationToken cancellationToken)
    {
        var geoRedisUrl = _config.GetValue<string>("GeoRedis:Url") ?? "http://localhost:3000";
        
        _logger.LogInformation("Connecting to GeoRedis at {Url}", geoRedisUrl);
        
        // Create gRPC channel
        _channel = GrpcChannel.ForAddress(geoRedisUrl, new GrpcChannelOptions
        {
            HttpHandler = new SocketsHttpHandler
            {
                PooledConnectionIdleTimeout = Timeout.InfiniteTimeSpan,
                KeepAlivePingDelay = TimeSpan.FromSeconds(60),
                KeepAlivePingTimeout = TimeSpan.FromSeconds(30),
                EnableMultipleHttp2Connections = true
            }
        });
        
        _grpcClient = new GeoRedis.GeoRedisClient(_channel);

        await base.StartAsync(cancellationToken);
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        _logger.LogInformation("SatellitePoller started. Polling every {Interval}", _pollInterval);

        // Initial fetch on startup
        await PollAndInsertAsync(stoppingToken);

        // Then poll at regular intervals
        using var timer = new PeriodicTimer(_pollInterval);
        
        while (!stoppingToken.IsCancellationRequested && await timer.WaitForNextTickAsync(stoppingToken))
        {
            await PollAndInsertAsync(stoppingToken);
        }
    }

    private async Task PollAndInsertAsync(CancellationToken cancellationToken)
    {
        try
        {
            if (_grpcClient == null)
            {
                _logger.LogError("gRPC client not initialized");
                return;
            }

            // Try to fetch from N2YO API (will use mock data if no API key)
            var useMock = _config.GetValue<bool>("N2YO:UseMock", true);
            List<SatelliteAbove> satellites;

            if (useMock || string.IsNullOrEmpty(_config.GetValue<string>("N2YO:ApiKey")))
            {
                _logger.LogInformation("Using mock satellite data (no API key configured)");
                satellites = _n2yoClient.GenerateMockSatellites();
            }
            else
            {
                satellites = await _n2yoClient.FetchSatellitesAboveAsync(cancellationToken);
            }
            
            if (satellites.Count == 0)
            {
                _logger.LogWarning("No satellites fetched");
                return;
            }

            // Build batch insert request
            var batchRequest = new InsertBatchRequest();
            var dtoList = new List<SatelliteDto>();

            foreach (var sat in satellites)
            {
                var dto = new SatelliteDto
                {
                    Id = sat.SatId,
                    Name = sat.SatName,
                    Lat = sat.Latitude,
                    Lon = sat.Longitude,
                    Altitude = sat.Altitude,
                    LaunchDate = sat.LaunchDate,
                    Category = GetSatelliteCategory(sat.SatName)
                };

                dtoList.Add(dto);

                // Create payload JSON with relevant metadata
                var payload = new
                {
                    name = dto.Name,
                    altitude = dto.Altitude,
                    launchDate = dto.LaunchDate,
                    category = dto.Category,
                    satId = dto.Id
                };

                batchRequest.Entries.Add(new GeoEntry
                {
                    Id = $"sat-{dto.Id}",
                    Lat = dto.Lat,
                    Lon = dto.Lon,
                    PayloadJson = JsonSerializer.Serialize(payload)
                });
            }

            // Update in-memory cache first (so API works even if gRPC is down)
            lock (_lock)
            {
                _latestSatellites = dtoList;
                _lastUpdate = DateTime.UtcNow;
            }

            // Try to insert batch via gRPC (will fail if geo-node is not running)
            try
            {
                var sw = System.Diagnostics.Stopwatch.StartNew();
                var response = await _grpcClient.InsertBatchAsync(batchRequest, cancellationToken: cancellationToken);
                sw.Stop();

                if (response.Success)
                {
                    _logger.LogInformation(
                        "Inserted {Count} satellites via gRPC in {Ms}ms", 
                        response.EntriesWritten,
                        sw.ElapsedMilliseconds);
                }
                else
                {
                    _logger.LogError("gRPC InsertBatch failed: {Error}", response.Error);
                }
            }
            catch (Exception grpcEx)
            {
                _logger.LogWarning(grpcEx, "gRPC unavailable (geo-node not running?), continuing with in-memory cache only");
            }
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error in satellite polling cycle");
        }
    }

    private static string GetSatelliteCategory(string name)
    {
        var upper = name.ToUpperInvariant();
        if (upper.Contains("ISS")) return "space-station";
        if (upper.Contains("STARLINK")) return "communication";
        if (upper.Contains("GPS") || upper.Contains("GLONASS") || upper.Contains("GALILEO")) return "navigation";
        if (upper.Contains("NOAA") || upper.Contains("GOES") || upper.Contains("METOP")) return "weather";
        if (upper.Contains("TERRA") || upper.Contains("AQUA") || upper.Contains("LANDSAT")) return "earth-observation";
        if (upper.Contains("HUBBLE") || upper.Contains("CHANDRA") || upper.Contains("JWST")) return "telescope";
        return "other";
    }

    public List<SatelliteDto> GetLatestSatellites()
    {
        lock (_lock)
        {
            return _latestSatellites.ToList();
        }
    }

    public DateTime GetLastUpdateTime()
    {
        lock (_lock)
        {
            return _lastUpdate;
        }
    }

    public override async Task StopAsync(CancellationToken cancellationToken)
    {
        _logger.LogInformation("SatellitePoller stopping...");
        
        if (_channel != null)
        {
            await _channel.ShutdownAsync();
            _channel.Dispose();
        }

        await base.StopAsync(cancellationToken);
    }
}
