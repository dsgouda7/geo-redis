using System.Text.Json;
using SatelliteServer.Models;

namespace SatelliteServer.Services;

/// <summary>
/// Client for fetching satellite data from N2YO API.
/// API docs: https://www.n2yo.com/api/
/// Free tier: 1000 requests/hour
/// </summary>
public class N2YOClient
{
    private readonly HttpClient _httpClient;
    private readonly ILogger<N2YOClient> _logger;
    private readonly string _apiKey;

    // Popular satellites to track
    private static readonly List<int> PopularSatellites = new()
    {
        25544, // ISS
        48274, // Starlink-4682
        43013, // Hubble
        36411, // Envisat
        25994, // Iridium 33 (debris)
        20580, // NOAA 15
        28654, // NOAA 18
        33591, // NOAA 19
        37849, // Suomi NPP
        43013, // HST (Hubble)
        27424, // TERRA
        40069, // AQUA
    };

    public N2YOClient(HttpClient httpClient, ILogger<N2YOClient> logger, IConfiguration config)
    {
        _httpClient = httpClient;
        _httpClient.BaseAddress = new Uri("https://api.n2yo.com/rest/v1/satellite/");
        _logger = logger;
        _apiKey = config.GetValue<string>("N2YO:ApiKey") ?? "DEMO_KEY";
    }

    /// <summary>
    /// Get all satellites above a location (observer).
    /// Uses global coverage: 4 quadrants covering the world.
    /// </summary>
    public async Task<List<SatelliteAbove>> FetchSatellitesAboveAsync(
        CancellationToken cancellationToken = default)
    {
        var allSatellites = new List<SatelliteAbove>();

        // Query 4 points to get global coverage
        var locations = new[]
        {
            (0.0, 0.0, "Equator/Prime"),      // Atlantic
            (0.0, 90.0, "Equator/East"),      // Indian Ocean
            (0.0, -90.0, "Equator/West"),     // Pacific
            (0.0, 180.0, "Equator/Date Line") // Pacific
        };

        foreach (var (lat, lon, name) in locations)
        {
            try
            {
                var url = $"above/{lat}/{lon}/0/90/18?apiKey={_apiKey}";
                _logger.LogInformation("Fetching satellites above {Location} ({Lat}, {Lon})", name, lat, lon);

                var response = await _httpClient.GetAsync(url, cancellationToken);
                
                // N2YO free tier returns demo data without valid API key
                if (!response.IsSuccessStatusCode)
                {
                    _logger.LogWarning("N2YO API returned {StatusCode} for {Location}", 
                        response.StatusCode, name);
                    continue;
                }

                var json = await response.Content.ReadAsStringAsync(cancellationToken);
                var data = JsonSerializer.Deserialize<N2YOAboveResponse>(json, new JsonSerializerOptions
                {
                    PropertyNameCaseInsensitive = true
                });

                if (data?.Above != null)
                {
                    allSatellites.AddRange(data.Above);
                    _logger.LogInformation("Found {Count} satellites above {Location}", 
                        data.Above.Count, name);
                }
            }
            catch (HttpRequestException ex)
            {
                _logger.LogError(ex, "HTTP error fetching satellites above {Location}", name);
            }
            catch (JsonException ex)
            {
                _logger.LogError(ex, "JSON parsing error for {Location}", name);
            }
        }

        // Deduplicate by satellite ID
        var unique = allSatellites
            .GroupBy(s => s.SatId)
            .Select(g => g.First())
            .ToList();

        _logger.LogInformation("Total unique satellites: {Count}", unique.Count);
        return unique;
    }

    /// <summary>
    /// Generate mock satellite data for testing without API key.
    /// Returns ~200 satellites in various orbits.
    /// </summary>
    public List<SatelliteAbove> GenerateMockSatellites()
    {
        var satellites = new List<SatelliteAbove>();
        var random = new Random(42); // Fixed seed for consistent results

        // ISS
        satellites.Add(new SatelliteAbove
        {
            SatId = 25544,
            SatName = "ISS (ZARYA)",
            Latitude = random.NextDouble() * 100 - 50,
            Longitude = random.NextDouble() * 360 - 180,
            Altitude = 408 + random.NextDouble() * 10,
            LaunchDate = "1998-11-20"
        });

        // Starlink constellation (100 satellites)
        for (int i = 0; i < 100; i++)
        {
            satellites.Add(new SatelliteAbove
            {
                SatId = 40000 + i,
                SatName = $"STARLINK-{1000 + i}",
                Latitude = random.NextDouble() * 100 - 50,
                Longitude = random.NextDouble() * 360 - 180,
                Altitude = 540 + random.NextDouble() * 10,
                LaunchDate = "2019-05-24"
            });
        }

        // GPS satellites (30 satellites)
        for (int i = 0; i < 30; i++)
        {
            satellites.Add(new SatelliteAbove
            {
                SatId = 32000 + i,
                SatName = $"GPS-IIF-{i + 1}",
                Latitude = random.NextDouble() * 100 - 50,
                Longitude = random.NextDouble() * 360 - 180,
                Altitude = 20200 + random.NextDouble() * 200,
                LaunchDate = "2010-05-27"
            });
        }

        // Weather satellites (20 satellites)
        for (int i = 0; i < 20; i++)
        {
            satellites.Add(new SatelliteAbove
            {
                SatId = 28000 + i,
                SatName = $"NOAA-{15 + i}",
                Latitude = random.NextDouble() * 140 - 70,
                Longitude = random.NextDouble() * 360 - 180,
                Altitude = 850 + random.NextDouble() * 50,
                LaunchDate = "1998-05-13"
            });
        }

        // Earth observation (30 satellites)
        for (int i = 0; i < 30; i++)
        {
            satellites.Add(new SatelliteAbove
            {
                SatId = 39000 + i,
                SatName = $"TERRA-{i + 1}",
                Latitude = random.NextDouble() * 120 - 60,
                Longitude = random.NextDouble() * 360 - 180,
                Altitude = 705 + random.NextDouble() * 50,
                LaunchDate = "1999-12-18"
            });
        }

        // Geostationary (20 satellites) - clustered near equator
        for (int i = 0; i < 20; i++)
        {
            satellites.Add(new SatelliteAbove
            {
                SatId = 27000 + i,
                SatName = $"GOES-{i + 13}",
                Latitude = random.NextDouble() * 10 - 5, // Near equator
                Longitude = random.NextDouble() * 360 - 180,
                Altitude = 35786 + random.NextDouble() * 100,
                LaunchDate = "2006-05-24"
            });
        }

        _logger.LogInformation("Generated {Count} mock satellites", satellites.Count);
        return satellites;
    }
}
