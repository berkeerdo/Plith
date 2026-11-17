use super::*;

#[test]
fn forecast_url_has_hourly_fields_and_timezone_auto() {
    let url = forecast_url(41.0138, 28.9497);
    assert!(url.starts_with("https://api.open-meteo.com/v1/forecast?"));
    assert!(url.contains("latitude=41.01&"), "{url}");
    assert!(url.contains("longitude=28.95&"), "{url}");
    for field in [
        "temperature_2m",
        "weather_code",
        "precipitation_probability",
    ] {
        assert!(url.contains(field), "missing {field}");
    }
    assert!(url.contains("timezone=auto"));
}

#[test]
fn geocode_url_escapes_city() {
    let url = geocode_url("İstanbul");
    assert!(url.starts_with("https://geocoding-api.open-meteo.com/v1/search?"));
    assert!(url.contains("name=%C4%B0stanbul"), "{url}");
    assert!(geocode_url("New York").contains("name=New%20York"));
}
