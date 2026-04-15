use crate::args::AuthAction;
use melcloud_core::{MelcloudClient, MelcloudError};
use serde_json::json;

pub(crate) async fn handle(
    client: &mut MelcloudClient,
    json_output: bool,
    action: &AuthAction,
) -> Result<Option<String>, MelcloudError> {
    match action {
        AuthAction::Test => {
            let session = client.test_connection().await?;
            if json_output {
                Ok(Some(serde_json::to_string_pretty(&json!({
                    "status": "ok",
                    "session": {
                        "display": session.to_string(),
                        "user_name": session.user_name,
                        "expires_at": session.expiry.map(|value| value.to_rfc3339()),
                        "duration_minutes": session.duration_minutes,
                    }
                }))?))
            } else {
                Ok(Some(format!("auth: ok\nsession: {}", session)))
            }
        }
    }
}
