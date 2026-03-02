use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteRepoQuery {
    owner: String,
    repo: String,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DeleteRepoQuery>,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    let mut repos = state.repos.write().await;
    let key = (query.owner, query.repo);
    let Some(repo) = repos.get(&key) else {
        return api_error(StatusCode::NOT_FOUND, "NotFound", "repository not found");
    };

    let Some(role) = lookup_role(repo, actor) else {
        return api_error(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "actor has no role in this repository",
        );
    };

    if !role.can_admin() {
        return api_error(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "only owners may delete repositories",
        );
    }

    repos.remove(&key);
    StatusCode::NO_CONTENT.into_response()
}
