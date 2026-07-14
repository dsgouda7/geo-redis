using System.Text.Json;
using Georedis.V1;

namespace SatelliteServer.Services;

/// <summary>
/// Calculates and stores the day/night terminator line coordinates.
/// The terminator is the boundary between day and night on Earth.
/// </summary>
public class TerminatorCalculator : BackgroundService
{
    private readonly ILogger<TerminatorCalculator> _logger;
    private readonly IConfiguration _config;
    private GeoRedis.GeoRedisClient? _grpcClient;
    private readonly object _lock = new();
    private List<(double Lat, double Lon)> _terminatorCoords = new();
    private DateTime _lastUpdate = DateTime.MinValue;
    private string _nightSide = "south";

    public TerminatorCalculator(
        ILogger<TerminatorCalculator> logger,
        IConfiguration config)
    {
        _logger = logger;
        _config = config;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        // Initialize gRPC client
        var geoRedisUrl = _config.GetValue<string>("GeoRedis:Url") ?? "http://localhost:3000";
        _logger.LogInformation("Connecting to GeoRedis at {Url}", geoRedisUrl);

        var channel = Grpc.Net.Client.GrpcChannel.ForAddress(geoRedisUrl);
        _grpcClient = new GeoRedis.GeoRedisClient(channel);

        _logger.LogInformation("TerminatorCalculator started. Updating every 60 seconds");

        using var timer = new PeriodicTimer(TimeSpan.FromSeconds(60));

        do
        {
            await CalculateAndStoreTerminatorAsync(stoppingToken);
        }
        while (await timer.WaitForNextTickAsync(stoppingToken));
    }

    private async Task CalculateAndStoreTerminatorAsync(CancellationToken cancellationToken)
    {
        try
        {
            if (_grpcClient == null)
            {
                _logger.LogError("gRPC client not initialized");
                return;
            }

            var now = DateTime.UtcNow;
            var coords = CalculateTerminatorCoordinates(now);

            // Store each point of the terminator line in proxima
            var batchRequest = new InsertBatchRequest();
            
            for (int i = 0; i < coords.Count; i++)
            {
                var (lat, lon) = coords[i];
                var payload = new
                {
                    type = "terminator-point",
                    index = i,
                    timestamp = now.ToString("O"),
                    totalPoints = coords.Count
                };

                batchRequest.Entries.Add(new GeoEntry
                {
                    Id = $"terminator-{i}",
                    Lat = lat,
                    Lon = lon,
                    PayloadJson = JsonSerializer.Serialize(payload)
                });
            }

            // Try to insert via gRPC
            try
            {
                var response = await _grpcClient.InsertBatchAsync(batchRequest, cancellationToken: cancellationToken);
                if (response.Success)
                {
                    _logger.LogInformation(
                        "Stored terminator line with {Count} points", 
                        coords.Count);
                }
            }
            catch (Exception grpcEx)
            {
                _logger.LogWarning(grpcEx, "gRPC unavailable, storing terminator in memory only");
            }

            // Update in-memory cache
            lock (_lock)
            {
                _terminatorCoords = coords;
                _lastUpdate = now;
            }
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error calculating terminator");
        }
    }

    /// <summary>
    /// Calculate the solar terminator (day/night boundary) for the given UTC time.
    /// Sweeps longitude from -180 to 180 and computes the terminator latitude at each,
    /// producing a continuous curve suitable for rendering on a flat map (no antimeridian wrap).
    /// </summary>
    private List<(double Lat, double Lon)> CalculateTerminatorCoordinates(DateTime utc)
    {
        var coords = new List<(double, double)>();

        // Solar declination (latitude where the sun is directly overhead)
        var dayOfYear = utc.DayOfYear;
        var declination = 23.45 * Math.Sin((2 * Math.PI / 365.0) * (dayOfYear - 81));
        var decRad = declination * Math.PI / 180.0;

        // Subsolar longitude (where it is currently local solar noon)
        var hoursSinceMidnight = utc.Hour + utc.Minute / 60.0 + utc.Second / 3600.0;
        var subsolarLon = (12 - hoursSinceMidnight) * 15.0; // degrees

        var tanDec = Math.Tan(decRad);

        // The night side is the hemisphere opposite the subsolar point.
        // If the sun is north of the equator (summer), the night region lies to the south.
        _nightSide = declination >= 0 ? "south" : "north";

        for (int lonDeg = -180; lonDeg <= 180; lonDeg++)
        {
            double lat;
            if (Math.Abs(tanDec) < 1e-9)
            {
                // Equinox: terminator runs along the meridians (poles).
                lat = 0;
            }
            else
            {
                var hourAngle = (lonDeg - subsolarLon) * Math.PI / 180.0;
                lat = Math.Atan(-Math.Cos(hourAngle) / tanDec) * 180.0 / Math.PI;
            }
            coords.Add((lat, lonDeg));
        }

        return coords;
    }

    private double NormalizeLongitude(double lon)
    {
        while (lon > 180) lon -= 360;
        while (lon < -180) lon += 360;
        return lon;
    }

    public string GetNightSide()
    {
        lock (_lock)
        {
            return _nightSide;
        }
    }

    public List<(double Lat, double Lon)> GetTerminatorCoordinates()
    {
        lock (_lock)
        {
            return new List<(double, double)>(_terminatorCoords);
        }
    }

    public DateTime GetLastUpdateTime()
    {
        lock (_lock)
        {
            return _lastUpdate;
        }
    }
}
