use super::*;

const BODY: &str = r#"{"latitude":41.0,"longitude":29.0,"generationtime_ms":0.05,
"hourly_units":{"time":"iso8601","temperature_2m":"°C","weather_code":"wmo code","precipitation_probability":"%"},
"hourly":{"time":["2026-11-17T12:00","2026-11-17T13:00","2026-11-17T14:00","2026-11-17T15:00","2026-11-17T16:00","2026-11-17T17:00"],
"temperature_2m":[10.4,11.0,11.5,10.2,9.6,8.5],
"weather_code":[3,3,2,61,63,3],
"precipitation_probability":[5,10,20,70,85,30]}}"#;

fn stamp(day: Day, hour: u8, minute: u8) -> Stamp {
    Stamp { day, hour, minute }
}

fn at() -> Stamp {
    stamp((2026, 11, 17), 11, 30)
}

fn forecast() -> Forecast {
    parse_forecast(BODY, at()).unwrap()
}

#[test]
fn wmo_codes_map_to_four_skies() {
    for (code, sky) in [
        (0, Sky::Clear),
        (1, Sky::Clear),
        (2, Sky::Cloud),
        (3, Sky::Cloud),
        (4, Sky::Cloud),
        (44, Sky::Cloud),
        (45, Sky::Cloud),
        (48, Sky::Cloud),
        (49, Sky::Cloud),
        (50, Sky::Cloud),
        (51, Sky::Rain),
        (67, Sky::Rain),
        (68, Sky::Cloud),
        (70, Sky::Cloud),
        (71, Sky::Snow),
        (77, Sky::Snow),
        (78, Sky::Cloud),
        (79, Sky::Cloud),
        (80, Sky::Rain),
        (82, Sky::Rain),
        (83, Sky::Cloud),
        (84, Sky::Cloud),
        (85, Sky::Snow),
        (86, Sky::Snow),
        (87, Sky::Cloud),
        (94, Sky::Cloud),
        (95, Sky::Rain),
        (99, Sky::Rain),
        (100, Sky::Cloud),
        (u16::MAX, Sky::Cloud),
    ] {
        assert_eq!(sky_of(code), sky, "code {code}");
    }
}

#[test]
fn parses_a_trimmed_open_meteo_body() {
    let f = forecast();
    assert_eq!(f.hours.len(), 6);
    assert_eq!(f.read_at, at());
    let h = f.hours[3];
    assert_eq!(h.day, (2026, 11, 17));
    assert_eq!(h.hour, 15);
    assert_eq!(h.sky, Sky::Rain);
    assert!((h.temp_c - 10.2).abs() < 1e-4);
    assert_eq!(h.rain_pct, Some(70));
    assert_eq!(f.hours[0].sky, Sky::Cloud);
}

#[test]
fn mismatched_array_lengths_is_an_error() {
    let body = r#"{"hourly":{"time":["2026-11-17T12:00","2026-11-17T13:00"],
"temperature_2m":[10.4],"weather_code":[3,3],"precipitation_probability":[5,10]}}"#;
    assert!(parse_forecast(body, at()).is_err());
}

#[test]
fn missing_pieces_are_errors() {
    assert!(parse_forecast("{}", at()).is_err());
    assert!(parse_forecast("not json", at()).is_err());
    let no_codes = r#"{"hourly":{"time":[],"temperature_2m":[],"precipitation_probability":[]}}"#;
    assert!(parse_forecast(no_codes, at()).is_err());
    let bad_time = r#"{"hourly":{"time":["soon"],"temperature_2m":[1],"weather_code":[0],"precipitation_probability":[1]}}"#;
    assert!(parse_forecast(bad_time, at()).is_err());
}

#[test]
fn empty_arrays_give_no_hours() {
    let body = r#"{"hourly":{"time":[],"temperature_2m":[],"weather_code":[],"precipitation_probability":[]}}"#;
    assert!(parse_forecast(body, at()).unwrap().hours.is_empty());
}

#[test]
fn null_precipitation_is_none() {
    let body = r#"{"hourly":{"time":["2026-11-17T12:00","2026-11-17T13:00"],
"temperature_2m":[10.4,9],"weather_code":[3,0],"precipitation_probability":[null,250]}}"#;
    let f = parse_forecast(body, at()).unwrap();
    assert_eq!(f.hours[0].rain_pct, None);
    assert_eq!(f.hours[1].rain_pct, Some(100));
}

#[test]
fn next_turn_finds_rain_at_15() {
    let f = forecast();
    assert_eq!(
        next_turn(&f, ((2026, 11, 17), 12), 6),
        Some((15, Sky::Rain))
    );
    assert_eq!(
        next_turn(&f, ((2026, 11, 17), 14), 1),
        Some((15, Sky::Rain))
    );
}

#[test]
fn next_turn_none_when_sky_holds() {
    let f = forecast();
    // Cloud at 12 and 13, the turn is at 15: out of a 2 hour window.
    assert_eq!(next_turn(&f, ((2026, 11, 17), 12), 2), None);
    // A window of 0 never turns.
    assert_eq!(next_turn(&f, ((2026, 11, 17), 16), 0), None);
    // Now is not in the forecast.
    assert_eq!(next_turn(&f, ((2026, 11, 17), 9), 6), None);
    assert_eq!(next_turn(&f, ((2026, 11, 18), 12), 6), None);
}

#[test]
fn stale_after_ninety_minutes() {
    let read = stamp((2026, 11, 17), 10, 0);
    assert!(!is_stale(read, stamp((2026, 11, 17), 10, 0)));
    assert!(!is_stale(read, stamp((2026, 11, 17), 11, 30)));
    assert!(is_stale(read, stamp((2026, 11, 17), 11, 31)));
    // Clock moved backwards: not stale.
    assert!(!is_stale(read, stamp((2026, 11, 17), 9, 0)));
}

#[test]
fn stale_across_midnight_month_and_year() {
    let read = stamp((2026, 11, 17), 23, 30);
    assert!(!is_stale(read, stamp((2026, 11, 18), 1, 0)));
    assert!(is_stale(read, stamp((2026, 11, 18), 1, 1)));
    let end_of_month = stamp((2026, 11, 30), 23, 0);
    assert!(!is_stale(end_of_month, stamp((2026, 12, 1), 0, 30)));
    assert!(is_stale(end_of_month, stamp((2026, 12, 1), 0, 31)));
    let new_year = stamp((2026, 12, 31), 23, 45);
    assert!(!is_stale(new_year, stamp((2027, 1, 1), 1, 15)));
    assert!(is_stale(new_year, stamp((2027, 1, 1), 1, 16)));
    let leap = stamp((2028, 2, 28), 23, 59);
    assert!(is_stale(leap, stamp((2028, 3, 1), 1, 30)));
    assert!(!is_stale(leap, stamp((2028, 2, 29), 1, 0)));
}

#[test]
fn ip_and_geocode_bodies_parse() {
    let ip = r#"{"ip":"203.0.113.9","city":"Istanbul","region":"Istanbul","latitude":41.01,"longitude":28.95,"timezone":"Europe/Istanbul"}"#;
    assert_eq!(parse_ip_location(ip), Some((41.01, 28.95)));
    // A realistic ipapi.co/json body: root latitude and longitude among many other fields.
    let ipapi = r#"{"ip":"203.0.113.9","network":"203.0.113.0/24","version":"IPv4","city":"Istanbul","region":"Istanbul","region_code":"34","country":"TR","country_name":"Turkey","postal":"34000","latitude":41.0138,"longitude":28.9497,"timezone":"Europe/Istanbul","utc_offset":"+0300","org":"Example ISP"}"#;
    assert_eq!(parse_ip_location(ipapi), Some((41.0138, 28.9497)));
    let limited = r#"{"error":true,"reason":"RateLimited","message":"Too many requests"}"#;
    assert_eq!(parse_ip_location(limited), None);
    assert_eq!(parse_ip_location(r#"{"error":true}"#), None);
    assert_eq!(parse_ip_location("nope"), None);
    let geo = r#"{"results":[{"id":745044,"name":"İstanbul","latitude":41.01384,"longitude":28.94966,"country":"Türkiye"},{"name":"Other","latitude":1.0,"longitude":2.0}],"generationtime_ms":0.4}"#;
    assert_eq!(
        parse_geocode(geo),
        Some((41.01384, 28.94966, "\u{130}stanbul".to_string()))
    );
    assert_eq!(parse_geocode(r#"{"results":[]}"#), None);
    assert_eq!(parse_geocode(r#"{"generationtime_ms":0.4}"#), None);
}

#[test]
fn celsius_text_rounds_half_away_from_zero() {
    assert_eq!(temp_text(8.6, TempUnit::C), "9°");
    assert_eq!(temp_text(2.5, TempUnit::C), "3°");
    assert_eq!(temp_text(-2.5, TempUnit::C), "-3°");
    assert_eq!(temp_text(-0.4, TempUnit::C), "0°");
    assert_eq!(temp_text(0.0, TempUnit::C), "0°");
}

#[test]
fn fahrenheit_text() {
    assert_eq!(temp_text(0.0, TempUnit::F), "32°");
    assert_eq!(temp_text(100.0, TempUnit::F), "212°");
    assert_eq!(temp_text(-40.0, TempUnit::F), "-40°");
    assert_eq!(temp_text(9.0, TempUnit::F), "48°");
    assert_eq!(temp_text(-17.8, TempUnit::F), "0°");
}
