use axum::{
    body::Body,
    extract::{Path, State},
    http::HeaderMap,
    response::Response,
};

use crate::state::AppState;

use super::service::service_post;

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((owner, repo)): Path<(String, String)>,
    body: Body,
) -> Response {
    service_post(state, headers, owner, repo, "git-receive-pack", body).await
}
