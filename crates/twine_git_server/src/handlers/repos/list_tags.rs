use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct ListTagsQuery {
    owner: String,
    repo: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ListTagsResponse {
    tags: Vec<String>,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListTagsQuery>,
) -> Response {
    let Some(actor) = actor_id(&headers) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "AuthRequired",
            "x-actor-id header is required",
        );
    };

    let repos = state.repos.read().await;
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

    if !role.can_read() {
        return api_error(StatusCode::FORBIDDEN, "Forbidden", "read access denied");
    }

    let tags = match state.git.list_tags(&repo.owner, &repo.name) {
        Ok(Some(tags)) => tags,
        Ok(None) => repo.tags.clone(),
        Err(error) => {
            tracing::warn!(owner = %repo.owner, repo = %repo.name, ?error, "failed to read tag metadata; falling back to in-memory state");
            repo.tags.clone()
        }
    };

    (StatusCode::OK, Json(ListTagsResponse { tags })).into_response()
}
