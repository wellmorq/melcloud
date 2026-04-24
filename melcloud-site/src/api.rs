use crate::error::SiteError;
use crate::models::{ConfigPatchRequest, FixedPresetId};
use crate::service::SiteService;
use std::convert::Infallible;
use std::sync::Arc;
use warp::http::StatusCode;
use warp::{Filter, Reply};

pub(crate) fn routes(
    service: Arc<SiteService>,
) -> impl Filter<Extract = (impl Reply,), Error = warp::Rejection> + Clone {
    let public_dir = service.config().public_dir.clone();
    let asset_dir = service.config().asset_dir.clone();
    let weather_icon_cache_dir = service.config().weather_icon_cache_dir.clone();
    let state = warp::any().map(move || service.clone());
    let get_state = warp::path!("api" / "state")
        .and(warp::get())
        .and(state.clone())
        .and_then(handle_state);
    let refresh = warp::path!("api" / "refresh")
        .and(warp::post())
        .and(state.clone())
        .and_then(handle_refresh);
    let apply_preset = warp::path!("api" / "presets" / FixedPresetId / "apply")
        .and(warp::post())
        .and(state.clone())
        .and_then(handle_apply_preset);
    let patch_preset = warp::path!("api" / "presets" / FixedPresetId / "config")
        .and(warp::patch())
        .and(state)
        .and(warp::body::content_length_limit(4096))
        .and(warp::body::json())
        .and_then(handle_patch_preset);
    let api = get_state.or(refresh).or(apply_preset).or(patch_preset);
    let weather_icons = warp::path("weather-icons").and(warp::fs::dir(weather_icon_cache_dir));
    let assets = warp::path("assets").and(warp::fs::dir(asset_dir));
    let index = warp::path::end().and(warp::fs::file(public_dir.join("index.html")));
    let files = warp::fs::dir(public_dir);
    api.or(weather_icons).or(assets).or(index).or(files)
}

async fn handle_state(service: Arc<SiteService>) -> Result<impl Reply, Infallible> {
    Ok(json_result(service.snapshot(false).await))
}

async fn handle_refresh(service: Arc<SiteService>) -> Result<impl Reply, Infallible> {
    Ok(json_result(service.refresh().await))
}

async fn handle_apply_preset(
    preset_id: FixedPresetId,
    service: Arc<SiteService>,
) -> Result<impl Reply, Infallible> {
    Ok(json_result(service.apply_preset(preset_id).await))
}

async fn handle_patch_preset(
    preset_id: FixedPresetId,
    service: Arc<SiteService>,
    patch: ConfigPatchRequest,
) -> Result<impl Reply, Infallible> {
    Ok(json_result(
        service.patch_active_preset(preset_id, &patch).await,
    ))
}

fn json_result<T>(result: Result<T, SiteError>) -> impl Reply
where
    T: serde::Serialize,
{
    match result {
        Ok(value) => warp::reply::with_status(warp::reply::json(&value), StatusCode::OK),
        Err(err) => {
            let code = match &err {
                SiteError::Protocol(_) => StatusCode::BAD_REQUEST,
                SiteError::Cli(_) | SiteError::WriteNotConfirmed(_) => StatusCode::BAD_GATEWAY,
                SiteError::CliTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
                SiteError::Io(_) | SiteError::Json(_) | SiteError::Yaml(_) => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            };
            let details = err.to_string();
            eprintln!("api error [{}]: {details}", err.kind());
            warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "error": err.user_message(),
                    "kind": err.kind(),
                    "details": details,
                })),
                code,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use warp::Reply;

    #[tokio::test]
    async fn api_error_uses_short_message_and_keeps_details() {
        let response = json_result::<Value>(Err(SiteError::Cli(
            "melcloud-cli [\"status\", \"--json\"] failed: raw stderr".to_string(),
        )))
        .into_response();

        let body = warp::hyper::body::to_bytes(response.into_body())
            .await
            .unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["error"], "MELCloud command failed.");
        assert_eq!(payload["kind"], "cli");
        assert!(payload["details"].as_str().unwrap().contains("raw stderr"));
    }
}
