use std::time::SystemTime;

use axum::{extract::State, response::IntoResponse};
use cacache::Metadata;
use http::StatusCode;
use maud::html;
use miette::IntoDiagnostic;

use crate::{get_policy_from_cache, AppState, CACHE_DIR};

#[axum_macros::debug_handler]
pub(crate) async fn route(
    State(_): State<AppState>,
) -> Result<impl IntoResponse, String> {
    let file_system_entries: Result<Vec<Metadata>, _> =
        tokio::task::spawn_blocking(move || cacache::list_sync(CACHE_DIR).collect())
            .await
            .into_diagnostic()
            .map_err(|e| e.to_string())?;
    let file_system_entries = file_system_entries.unwrap_or_default();

    let resp = html! {
        h2 { "Actions" }
        form method="post" action="/_chainedge/clear_fs" {
            input type="submit" value="Clear FS";
        }

        h2 { "Cached File" }
        ul {
            @for entry in file_system_entries {
                li { (entry.key) " TTL Seconds: " (get_policy_from_cache(&entry.key).await.unwrap().0.time_to_live(SystemTime::now()).as_secs()) }
            }
        }
    };

    Ok((StatusCode::OK, resp))
}
