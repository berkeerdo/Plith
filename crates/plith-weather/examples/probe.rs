//! Reads the forecast for fixed coordinates (Istanbul, no personal location) and prints it.

use plith_weather::HttpFetch;

fn main() {
    match HttpFetch::forecast_at(41.01, 28.98) {
        Ok(forecast) => {
            println!("read_at {:?}", forecast.read_at);
            println!("{} hours", forecast.hours.len());
            for hour in forecast.hours.iter().take(6) {
                println!("{hour:?}");
            }
        }
        Err(message) => {
            eprintln!("probe failed: {message}");
            std::process::exit(1);
        }
    }
}
