using System.Text.Json.Serialization;

namespace SatelliteServer.Models;

/// <summary>
/// Satellite position from N2YO API.
/// </summary>
public class SatellitePosition
{
    [JsonPropertyName("satlatitude")]
    public double Latitude { get; set; }

    [JsonPropertyName("satlongitude")]
    public double Longitude { get; set; }

    [JsonPropertyName("sataltitude")]
    public double Altitude { get; set; }

    [JsonPropertyName("azimuth")]
    public double Azimuth { get; set; }

    [JsonPropertyName("elevation")]
    public double Elevation { get; set; }

    [JsonPropertyName("timestamp")]
    public long Timestamp { get; set; }
}

/// <summary>
/// Response from N2YO "above" endpoint - satellites currently above a location.
/// </summary>
public class N2YOAboveResponse
{
    [JsonPropertyName("above")]
    public List<SatelliteAbove>? Above { get; set; }
}

public class SatelliteAbove
{
    [JsonPropertyName("satid")]
    public int SatId { get; set; }

    [JsonPropertyName("satname")]
    public string SatName { get; set; } = string.Empty;

    [JsonPropertyName("intDesignator")]
    public string? IntDesignator { get; set; }

    [JsonPropertyName("launchDate")]
    public string? LaunchDate { get; set; }

    [JsonPropertyName("satlat")]
    public double Latitude { get; set; }

    [JsonPropertyName("satlng")]
    public double Longitude { get; set; }

    [JsonPropertyName("satalt")]
    public double Altitude { get; set; }
}

/// <summary>
/// Simplified satellite data for API responses.
/// </summary>
public class SatelliteDto
{
    public int Id { get; set; }
    public string Name { get; set; } = string.Empty;
    public double Lat { get; set; }
    public double Lon { get; set; }
    public double Altitude { get; set; }
    public string? LaunchDate { get; set; }
    public string Category { get; set; } = "satellite";
}
