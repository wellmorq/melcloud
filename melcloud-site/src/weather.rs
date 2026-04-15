use crate::error::{Result, SiteError};
use crate::models::{CliDevicesEntry, CliStatusResponse, CliWeatherObservation, WeatherCard};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const WEATHER_ICON_SOURCES: &[&str] = &[
    "https://www.worldweatheronline.com/images/wsymbols01_png_64",
    "https://cdn.worldweatheronline.com/images/wsymbols01_png_64",
    "https://cdn.worldweatheronline.net/images/wsymbols01_png_64",
];

#[derive(Debug, Clone)]
struct WeatherCardSeed {
    slot: String,
    date: Option<String>,
    period_key: String,
    icon_name: Option<String>,
    condition_name: Option<String>,
    temperature: Option<f64>,
    placeholder: bool,
    fallback_icon: String,
}

pub(crate) async fn build_weather_cards(
    status: &CliStatusResponse,
    devices: &[CliDevicesEntry],
    cache_dir: &Path,
    icon_timeout_ms: u64,
) -> Result<Vec<WeatherCard>> {
    let seeds = select_weather_cards(status, devices);
    let mut cards = Vec::with_capacity(seeds.len());
    for seed in seeds {
        let icon = resolve_weather_icon(cache_dir, &seed, icon_timeout_ms)?;
        cards.push(WeatherCard {
            slot: seed.slot,
            date: seed.date,
            period_key: seed.period_key,
            icon,
            temperature: seed.temperature,
            placeholder: seed.placeholder,
        });
    }
    Ok(cards)
}

#[cfg(test)]
pub(crate) fn build_weather_cards_without_download(
    status: &CliStatusResponse,
    devices: &[CliDevicesEntry],
) -> Vec<WeatherCard> {
    select_weather_cards(status, devices)
        .into_iter()
        .map(|seed| WeatherCard {
            slot: seed.slot,
            date: seed.date,
            period_key: seed.period_key,
            icon: seed.fallback_icon,
            temperature: seed.temperature,
            placeholder: seed.placeholder,
        })
        .collect()
}

fn select_weather_cards(
    status: &CliStatusResponse,
    devices: &[CliDevicesEntry],
) -> Vec<WeatherCardSeed> {
    let mut observations = status.weather.clone();
    if observations.is_empty() {
        observations = forecast_from_devices(devices);
    }
    observations.sort_by_key(|item| item.date.clone());
    let mut used = vec![false; observations.len()];
    let now = take_first(&observations, &mut used, |_| true);
    let night = take_first(&observations, &mut used, is_night_like)
        .or_else(|| take_first(&observations, &mut used, |_| true));
    let day = take_first(&observations, &mut used, |item| !is_night_like(item))
        .or_else(|| take_first(&observations, &mut used, |_| true));
    vec![
        card_for("now", "now", now),
        card_for("night", "night", night),
        card_for("day", "day", day),
        card_for_future(None),
    ]
}

fn forecast_from_devices(devices: &[CliDevicesEntry]) -> Vec<CliWeatherObservation> {
    devices
        .first()
        .and_then(|entry| entry.raw.get("Device"))
        .and_then(|device| device.get("WeatherForecast"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn take_first<'a, F>(
    observations: &'a [CliWeatherObservation],
    used: &mut [bool],
    predicate: F,
) -> Option<&'a CliWeatherObservation>
where
    F: Fn(&CliWeatherObservation) -> bool,
{
    for (index, observation) in observations.iter().enumerate() {
        if used[index] || !predicate(observation) {
            continue;
        }
        used[index] = true;
        return Some(observation);
    }
    None
}

fn card_for(
    slot: &str,
    period_key: &str,
    observation: Option<&CliWeatherObservation>,
) -> WeatherCardSeed {
    WeatherCardSeed {
        slot: slot.to_string(),
        date: observation.and_then(|item| item.date.clone()),
        period_key: period_key.to_string(),
        icon_name: observation.and_then(|item| item.icon.clone()),
        condition_name: observation.and_then(|item| item.condition_name.clone()),
        temperature: observation.and_then(|item| item.temperature),
        placeholder: observation.is_none(),
        fallback_icon: observation
            .map(icon_from_observation)
            .unwrap_or_else(|| "weather_cloud".to_string()),
    }
}

fn card_for_future(_observation: Option<&CliWeatherObservation>) -> WeatherCardSeed {
    WeatherCardSeed {
        slot: "future".to_string(),
        date: None,
        period_key: "noData".to_string(),
        icon_name: None,
        condition_name: None,
        temperature: None,
        placeholder: true,
        fallback_icon: "weather_cloud".to_string(),
    }
}

fn resolve_weather_icon(
    cache_dir: &Path,
    seed: &WeatherCardSeed,
    icon_timeout_ms: u64,
) -> Result<String> {
    if seed.placeholder {
        return Ok(seed.fallback_icon.clone());
    }
    let Some(icon_name) = seed.icon_name.as_deref() else {
        return Ok(seed.fallback_icon.clone());
    };
    let cache_key = cache_key(seed.condition_name.as_deref(), icon_name);
    let icon_path = cached_icon_path(cache_dir, &cache_key);
    let source_path = cached_source_path(cache_dir, &cache_key);
    if icon_path.exists() && cached_source_matches(&source_path, icon_name) {
        return Ok(public_weather_icon_path(&cache_key));
    }
    if icon_timeout_ms > 0 {
        spawn_weather_icon_cache(
            cache_dir.to_path_buf(),
            cache_key,
            icon_name.to_string(),
            icon_timeout_ms,
        );
    }
    Ok(seed.fallback_icon.clone())
}

fn spawn_weather_icon_cache(
    cache_dir: PathBuf,
    cache_key: String,
    icon_name: String,
    icon_timeout_ms: u64,
) {
    tokio::spawn(async move {
        if let Err(err) =
            ensure_weather_icon_cached(&cache_dir, &cache_key, &icon_name, icon_timeout_ms).await
        {
            eprintln!("weather icon cache skipped for `{icon_name}`: {err}");
        }
    });
}

async fn ensure_weather_icon_cached(
    cache_dir: &Path,
    cache_key: &str,
    icon_name: &str,
    icon_timeout_ms: u64,
) -> Result<String> {
    fs::create_dir_all(cache_dir)?;
    let icon_path = cached_icon_path(cache_dir, cache_key);
    let source_path = cached_source_path(cache_dir, cache_key);
    if icon_path.exists() && cached_source_matches(&source_path, icon_name) {
        return Ok(public_weather_icon_path(cache_key));
    }
    let bytes = download_weather_icon(icon_name, icon_timeout_ms).await?;
    fs::write(&icon_path, bytes)?;
    fs::write(&source_path, icon_name)?;
    Ok(public_weather_icon_path(cache_key))
}

fn cached_icon_path(cache_dir: &Path, cache_key: &str) -> PathBuf {
    cache_dir.join(format!("{cache_key}.png"))
}

fn cached_source_path(cache_dir: &Path, cache_key: &str) -> PathBuf {
    cache_dir.join(format!("{cache_key}.source.txt"))
}

fn public_weather_icon_path(cache_key: &str) -> String {
    format!("/weather-icons/{cache_key}.png")
}

fn cached_source_matches(path: &Path, icon_name: &str) -> bool {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim() == icon_name)
        .unwrap_or(false)
}

fn cache_key(condition_name: Option<&str>, icon_name: &str) -> String {
    let raw = condition_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(icon_name);
    let slug = raw
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();
    let collapsed = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "weather-icon".to_string()
    } else {
        collapsed
    }
}

async fn download_weather_icon(icon_name: &str, icon_timeout_ms: u64) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(icon_timeout_ms))
        .build()
        .map_err(|err| SiteError::Protocol(format!("failed to build icon downloader: {err}")))?;
    for base in WEATHER_ICON_SOURCES {
        let url = format!("{base}/{icon_name}.png");
        let response = match client.get(&url).send().await {
            Ok(response) => response,
            Err(_) => continue,
        };
        if !response.status().is_success() {
            continue;
        }
        let bytes = response.bytes().await.map_err(|err| {
            SiteError::Protocol(format!("failed to read weather icon `{icon_name}`: {err}"))
        })?;
        return Ok(bytes.to_vec());
    }
    Err(SiteError::Protocol(format!(
        "failed to download weather icon `{icon_name}`"
    )))
}

fn is_night_like(observation: &CliWeatherObservation) -> bool {
    matches!(observation.weather_type, Some(2))
        || haystack(observation).contains("night")
        || haystack(observation).contains("moon")
}

fn icon_from_observation(observation: &CliWeatherObservation) -> String {
    let text = haystack(observation);
    if is_night_like(observation) {
        return "weather_moon".to_string();
    }
    if is_cloud_like(&text) {
        return "weather_cloud".to_string();
    }
    "weather_sun".to_string()
}

fn haystack(observation: &CliWeatherObservation) -> String {
    format!(
        "{} {}",
        observation
            .icon
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase(),
        observation
            .condition_name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
    )
}

fn is_cloud_like(text: &str) -> bool {
    [
        "cloud", "rain", "shower", "storm", "snow", "sleet", "mist", "fog", "overcast", "patchy",
    ]
    .iter()
    .any(|keyword| text.contains(keyword))
}
