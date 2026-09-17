//! Weather domain state and HTTP refresh logic, relocated wholesale from
//! `apps/shell/src/weather.rs` (Phase 4a of the dock/island/panel refactor)
//! since this is weather-domain logic, not shell-engine infra.

use coconut_core::{geo, UserProfile};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;

const MET_FORECAST_URL: &str = "https://api.met.no/weatherapi/locationforecast/2.0/compact";
const NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";
const USER_AGENT: &str = "Coconut/0.1 contact@sammwy.com";

#[derive(Clone)]
pub enum WeatherState {
    Loading,
    Unavailable(String),
    Ready(Weather),
}

#[derive(Clone)]
pub struct Weather {
    pub city: String,
    pub location_codes: String,
    pub temperature_c: f64,
    pub condition: String,
    pub high_c: f64,
    pub low_c: f64,
    pub hourly: Vec<HourlyForecast>,
}

#[derive(Clone)]
pub struct HourlyForecast {
    pub time: String,
    pub temperature_c: f64,
    pub condition: String,
}

impl WeatherState {
    pub fn bar_temperature(&self) -> String {
        match self {
            Self::Ready(weather) => format_temperature(weather.temperature_c),
            Self::Loading => "…".into(),
            Self::Unavailable(_) => "--".into(),
        }
    }

    pub fn bar_condition(&self) -> String {
        match self {
            Self::Ready(weather) => condition_label(&weather.condition).to_owned(),
            Self::Loading => "Weather".into(),
            Self::Unavailable(_) => "Unavailable".into(),
        }
    }
}

/// Performs one blocking HTTP round-trip (geocoding the user's configured
/// city if no cached coordinates exist yet, then fetching the forecast) and
/// returns the resulting [`WeatherState`].
///
/// **Contract for callers (`apps/shell/src/lib.rs`, Phase 4d):** this
/// function blocks on network I/O, so it must be run on a background thread
/// (mirroring the old `apps/shell/src/lib.rs::refresh_weather`, which called
/// this via `AppHandle::spawn_background` and pushed the result into a
/// `Signal<WeatherState>` on the render thread). Call it once at startup
/// (when the `weather` island is present in `shell.toml`), and again
/// whenever `RuntimeEvent::UserProfileChanged` fires or the weather island
/// transitions from absent to present in a reloaded `shell.toml` — there is
/// no periodic re-fetch beyond that, matching the old behavior.
pub fn refresh(profile: &mut UserProfile) -> WeatherState {
    let client = match Client::builder()
        .user_agent(USER_AGENT)
        .gzip(true)
        .timeout(Duration::from_secs(12))
        .build()
    {
        Ok(client) => client,
        Err(error) => return WeatherState::Unavailable(format!("Weather client error: {error}")),
    };
    let coordinates = match coordinates(&client, profile) {
        Ok(coordinates) => coordinates,
        Err(error) => return WeatherState::Unavailable(error),
    };
    match forecast(&client, profile, coordinates) {
        Ok(weather) => WeatherState::Ready(weather),
        Err(error) => WeatherState::Unavailable(error),
    }
}

fn coordinates(client: &Client, profile: &mut UserProfile) -> Result<(f64, f64), String> {
    if let (Some(latitude), Some(longitude)) = (profile.latitude, profile.longitude) {
        return Ok((latitude, longitude));
    }
    let city = profile.city.trim();
    if city.is_empty() {
        return Err("Set a city in Users settings to show weather.".into());
    }
    let country = geo::country_name(&profile.country)
        .ok_or_else(|| "Set a valid country in Users settings to show weather.".to_owned())?;
    let mut request = client.get(NOMINATIM_URL).query(&[
        ("city", city),
        ("country", country),
        (
            "countrycodes",
            profile.country.to_ascii_lowercase().as_str(),
        ),
        ("format", "jsonv2"),
        ("limit", "1"),
    ]);
    if let Some(region) = geo::region_name(&profile.region) {
        request = request.query(&[("state", region)]);
    }
    let places: Vec<NominatimPlace> = request
        .send()
        .map_err(|_| "Could not resolve the configured location.".to_owned())?
        .error_for_status()
        .map_err(|_| "Could not resolve the configured location.".to_owned())?
        .json()
        .map_err(|_| "Location service returned invalid data.".to_owned())?;
    let place = places
        .first()
        .ok_or_else(|| "The configured city could not be found.".to_owned())?;
    let latitude = place
        .lat
        .parse::<f64>()
        .map_err(|_| "Location service returned invalid coordinates.".to_owned())?;
    let longitude = place
        .lon
        .parse::<f64>()
        .map_err(|_| "Location service returned invalid coordinates.".to_owned())?;
    profile.latitude = Some(latitude);
    profile.longitude = Some(longitude);
    if let Err(error) = profile.save() {
        eprintln!("weather: could not cache resolved location: {error}");
    }
    Ok((latitude, longitude))
}

fn forecast(
    client: &Client,
    profile: &UserProfile,
    coordinates: (f64, f64),
) -> Result<Weather, String> {
    let (latitude, longitude) = coordinates;
    let response: MetResponse = client
        .get(MET_FORECAST_URL)
        .query(&[
            ("lat", format!("{latitude:.4}")),
            ("lon", format!("{longitude:.4}")),
        ])
        .send()
        .map_err(|_| "Could not fetch weather data.".to_owned())?
        .error_for_status()
        .map_err(|_| "Weather service rejected the request.".to_owned())?
        .json()
        .map_err(|_| "Weather service returned invalid data.".to_owned())?;
    let current = response
        .properties
        .timeseries
        .first()
        .ok_or_else(|| "Weather service returned an empty forecast.".to_owned())?;
    let hourly: Vec<_> = response
        .properties
        .timeseries
        .iter()
        .take(5)
        .map(|step| HourlyForecast {
            time: step.time.clone(),
            temperature_c: step.data.instant.details.air_temperature,
            condition: symbol(&step.data),
        })
        .collect();
    let (low_c, high_c) = response.properties.timeseries.iter().take(24).fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(low, high), step| {
            let temperature = step.data.instant.details.air_temperature;
            (low.min(temperature), high.max(temperature))
        },
    );
    Ok(Weather {
        city: profile.city.trim().to_owned(),
        location_codes: [profile.country.trim(), profile.region.trim()]
            .into_iter()
            .filter(|code| !code.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
        temperature_c: current.data.instant.details.air_temperature,
        condition: symbol(&current.data),
        high_c,
        low_c,
        hourly,
    })
}

pub fn format_temperature(value: f64) -> String {
    format!("{value:.0}°")
}

pub fn condition_label(symbol: &str) -> &'static str {
    let root = symbol.split('_').next().unwrap_or(symbol);
    if root.contains("thunder") {
        "Thunderstorm"
    } else if root.contains("snow") {
        "Snow"
    } else if root.contains("sleet") {
        "Sleet"
    } else if root.contains("rain") {
        "Rain"
    } else {
        match root {
            "clearsky" => "Clear",
            "fair" => "Fair",
            "partlycloudy" => "Partly cloudy",
            "cloudy" => "Cloudy",
            "fog" => "Fog",
            _ => "Weather unavailable",
        }
    }
}

pub fn icon_for(symbol: &str) -> &'static str {
    let root = symbol.split('_').next().unwrap_or(symbol);
    if root.contains("rain") || root.contains("sleet") || root.contains("thunder") {
        "weather_rain"
    } else if root.contains("snow") || root == "cloudy" {
        "weather_cloudly"
    } else {
        match root {
            "clearsky" | "fair" => "weather_heat",
            "partlycloudy" => "weather_cloud_sun",
            "fog" => "weather_fog",
            _ => "weather_cloudly",
        }
    }
}

fn symbol(data: &MetData) -> String {
    data.next_1_hours
        .as_ref()
        .or(data.next_6_hours.as_ref())
        .or(data.next_12_hours.as_ref())
        .map(|period| period.summary.symbol_code.clone())
        .unwrap_or_else(|| "unknown".into())
}

#[derive(Deserialize)]
struct NominatimPlace {
    lat: String,
    lon: String,
}

#[derive(Deserialize)]
struct MetResponse {
    properties: MetProperties,
}

#[derive(Deserialize)]
struct MetProperties {
    timeseries: Vec<MetTimeStep>,
}

#[derive(Deserialize)]
struct MetTimeStep {
    time: String,
    data: MetData,
}

#[derive(Deserialize)]
struct MetData {
    instant: MetInstant,
    #[serde(default)]
    next_1_hours: Option<MetPeriod>,
    #[serde(default)]
    next_6_hours: Option<MetPeriod>,
    #[serde(default)]
    next_12_hours: Option<MetPeriod>,
}

#[derive(Deserialize)]
struct MetInstant {
    details: MetInstantDetails,
}

#[derive(Deserialize)]
struct MetInstantDetails {
    air_temperature: f64,
}

#[derive(Deserialize)]
struct MetPeriod {
    summary: MetSummary,
}

#[derive(Deserialize)]
struct MetSummary {
    symbol_code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_negative_and_positive_temperatures() {
        assert_eq!(format_temperature(-1.4), "-1°");
        assert_eq!(format_temperature(22.6), "23°");
    }

    #[test]
    fn maps_met_symbols_to_a_label() {
        assert_eq!(condition_label("partlycloudy_day"), "Partly cloudy");
        assert_eq!(condition_label("heavyrainshowers_night"), "Rain");
    }
}
