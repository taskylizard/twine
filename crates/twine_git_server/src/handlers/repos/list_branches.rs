use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    auth::{actor_id, lookup_role},
    errors::api_error,
    state::AppState,
};

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct ListBranchesQuery {
    owner: String,
    repo: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ListBranchesResponse {
    branches: Vec<String>,
    default_branch: String,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListBranchesQuery>,
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

    let (branches, default_branch) = match state.git.list_branches(&repo.owner, &repo.name) {
        Ok(Some((branches, default_branch))) => (branches, default_branch),
        Ok(None) => (repo.branches.clone(), repo.default_branch.clone()),
        Err(error) => {
            tracing::warn!(owner = %repo.owner, repo = %repo.name, ?error, "failed to read branch metadata; falling back to in-memory state");
            (repo.branches.clone(), repo.default_branch.clone())
        }
    };

    (
        StatusCode::OK,
        Json(ListBranchesResponse {
            branches,
            default_branch,
        }),
    )
        .into_response()
}
